mod init_stack;
mod inner;
mod user_ptr;

use alloc::{
    sync::{Arc, Weak},
    vec,
    vec::Vec,
};
use compact_str::CompactString;
use defines::{config::PAGE_SIZE, error::Result, trap_context::TrapContext};
use goblin::elf::Elf;
use idallocator::RecycleAllocator;
use memory::{MemorySet, KERNEL_SPACE};
use spin::{Lazy, Mutex};

use crate::thread::{self, Thread};

use self::{
    init_stack::{UserAppInfo, UserStackInit, AT_PAGESZ},
    inner::ProcessInner,
};

pub use self::user_ptr::*;

// FIXME: 暂时而言应用是嵌入内核的。之后需要修复

static INITPROC_ELF: &[u8] =
    include_bytes!("../../../../user/target/riscv64imac-unknown-none-elf/release/initproc");

static SHELL_ELF: &[u8] =
    include_bytes!("../../../../user/target/riscv64imac-unknown-none-elf/release/shell");

pub static INITPROC: Lazy<Arc<Process>> = Lazy::new(|| {
    Process::from_path(
        CompactString::from_static_str("initproc"),
        vec![CompactString::from_static_str("initproc")],
    )
    .expect("INITPROC Failed.")
});

pub struct Process {
    pid: usize,
    pub inner: Mutex<ProcessInner>,
}

impl Process {
    pub fn from_path(path: CompactString, args: Vec<CompactString>) -> Result<Arc<Self>> {
        let mut process_name = path.clone();
        for arg in args.iter().skip(1) {
            process_name.push(' ');
            process_name.push_str(arg);
        }

        let mut memory_set = MemorySet::new_bare();
        memory_set.map_kernel_areas(KERNEL_SPACE.page_table());
        let elf_data = if path == "initproc" {
            INITPROC_ELF
        } else if path == "shell" {
            SHELL_ELF
        } else {
            panic!("Unsupported app");
        };
        let elf = Elf::parse(elf_data).expect("Should be valid elf");
        let elf_end = memory_set.load_sections(&elf, elf_data);
        let mut user_sp = Thread::alloc_user_stack(0, &mut memory_set);

        // 在用户栈上推入参数、环境变量、辅助向量等
        let mut stack_init = UserStackInit::new(user_sp, memory_set.page_table());
        let argc = args.len();
        let argv_base = stack_init.init_stack(UserAppInfo {
            args,
            envs: Vec::new(),
            auxv: vec![(AT_PAGESZ, PAGE_SIZE)],
        });
        user_sp = stack_init.user_sp();

        let process = Arc::new_cyclic(|process| {
            let brk = elf_end.vpn_ceil().page_start();
            let mut tid_allocator = RecycleAllocator::new();
            let tid = tid_allocator.alloc();
            // 第一个线程，主线程，tid 为 0
            assert_eq!(tid, 0);
            let mut trap_context = TrapContext::app_init_context(elf.entry as usize, user_sp);
            trap_context.user_regs[9] = argc;
            trap_context.user_regs[10] = argv_base;
            Process {
                pid: PID_ALLOCATOR.lock().alloc(),
                inner: Mutex::new(ProcessInner {
                    name: process_name,
                    memory_set,
                    heap_range: brk..brk,
                    parent: Weak::new(),
                    children: Vec::new(),
                    zombie_exit_code: None,
                    cwd: CompactString::from_static_str("/"),
                    tid_allocator,
                    threads: vec![Some(Arc::new(Thread::new(
                        Weak::clone(process),
                        tid,
                        trap_context,
                    )))],
                }),
            }
        });

        Ok(process)
    }

    /// clone 一个新进程，目前仅支持只有一个主线程的进程。
    ///
    /// `stack` 若不为 0 则指定新进程的栈顶
    pub fn clone(self: &Arc<Self>, stack: usize) -> Arc<Self> {
        let child = self.lock_inner(|inner| {
            assert_eq!(inner.thread_count(), 1);
            let pid = PID_ALLOCATOR.lock().alloc();
            let child = Arc::new_cyclic(|weak_child| {
                // 复制父进程的地址空间
                let memory_set = MemorySet::from_existed_user(&inner.memory_set);
                // // 复制文件描述符表
                // let new_fd_table = parent_inner.fd_table.clone();
                let parent_main_thread = inner.main_thread();
                let mut trap_context =
                    parent_main_thread.lock_inner(|inner| inner.trap_context.clone());
                if stack != 0 {
                    trap_context.user_regs[1] = stack;
                }
                let child_main_thread = Arc::new(Thread::new(
                    Weak::clone(weak_child),
                    parent_main_thread.tid,
                    trap_context,
                ));
                Self {
                    pid,
                    inner: Mutex::new(ProcessInner {
                        name: inner.name.clone(),
                        memory_set,
                        heap_range: inner.heap_range.clone(),
                        parent: Arc::downgrade(self),
                        children: Vec::new(),
                        threads: vec![Some(Arc::clone(&child_main_thread))],
                        zombie_exit_code: None,
                        cwd: inner.cwd.clone(),
                        tid_allocator: inner.tid_allocator.clone(),
                    }),
                }
            });
            // 新进程添入原进程的子进程表
            inner.children.push(Arc::clone(&child));
            child
        });
        // 子进程的主线程可以加入调度队列中了
        child.lock_inner(|inner| thread::spawn_user_thread(inner.main_thread()));
        child
    }

    /// 根据 `path` 加载一个新的 ELF 文件并执行。目前要求原进程仅有一个线程
    pub fn exec(&self, path: CompactString, args: Vec<CompactString>) -> Result<()> {
        let mut process_name = path.clone();
        for arg in args.iter().skip(1) {
            process_name.push(' ');
            process_name.push_str(arg);
        }

        let elf_data = if path == "initproc" {
            INITPROC_ELF
        } else if path == "shell" {
            SHELL_ELF
        } else {
            panic!("Unsupported app: {path}");
        };
        let elf = Elf::parse(elf_data).expect("Should be valid elf");
        self.lock_inner(|inner| {
            assert_eq!(inner.thread_count(), 1);
            assert_eq!(inner.children.len(), 0);
            inner.name = process_name;
            inner.memory_set.recycle_user_pages();
            let elf_end = inner.memory_set.load_sections(&elf, elf_data);
            inner.heap_range = {
                let brk = elf_end.vpn_ceil().page_start();
                brk..brk
            };
            let mut user_sp = Thread::alloc_user_stack(0, &mut inner.memory_set);
            inner.memory_set.flush_tlb(None);

            // TODO: 这边其实不需要这样。因为此时的页表就是该进程的页表
            let mut stack_init = UserStackInit::new(user_sp, inner.memory_set.page_table());
            let argc = args.len();
            let argv_base = stack_init.init_stack(UserAppInfo {
                args,
                envs: Vec::new(),
                auxv: vec![(AT_PAGESZ, PAGE_SIZE)],
            });
            user_sp = stack_init.user_sp();

            inner.main_thread().lock_inner(|inner| {
                inner.trap_context = TrapContext::app_init_context(elf.entry as usize, user_sp);
                inner.trap_context.user_regs[9] = argc;
                inner.trap_context.user_regs[10] = argv_base;
            });
        });

        Ok(())
    }

    /// 锁 inner 然后进行操作。这应该是访问 inner 的唯一方式
    pub fn lock_inner<T>(&self, f: impl FnOnce(&mut ProcessInner) -> T) -> T {
        f(&mut self.inner.lock())
    }

    pub fn pid(&self) -> usize {
        self.pid
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        PID_ALLOCATOR.lock().dealloc(self.pid)
    }
}

static PID_ALLOCATOR: Mutex<RecycleAllocator> = Mutex::new(RecycleAllocator::begin_with(1));

/// 退出进程，终止其所有线程。
///
/// 但注意，其他线程此时可能正在运行，因此终止不是立刻发生的，仅仅只是标记该进程为 zombie
///
/// 其他线程在进入内核时会检查对应的进程是否为 zombie 从而决定是否退出
pub fn exit_process(process: Arc<Process>, exit_code: i8) {
    info!("[Pid {}] Process exits with code {exit_code}", process.pid);
    process.lock_inner(|inner| inner.mark_exit(exit_code));
}
