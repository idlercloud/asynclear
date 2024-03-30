mod init_stack;
mod inner;

use core::num::NonZeroUsize;

use alloc::{collections::BTreeMap, vec, vec::Vec};
use atomic::{Atomic, Ordering};
use common::config::PAGE_SIZE;
use compact_str::CompactString;
use defines::{
    error::{errno, Result},
    signal::{KSignalSet, Signal},
};
use event_listener::Event;
use goblin::elf::Elf;
use idallocator::RecycleAllocator;
use klocks::{Lazy, SpinMutex, SpinMutexGuard};
use memory::{MemorySet, KERNEL_SPACE};
use triomphe::Arc;

use crate::{
    fs::FdTable,
    memory,
    signal::SignalHandlers,
    thread::{self, Thread},
    trap::TrapContext,
    uart_console::println,
};

use self::{
    init_stack::{UserAppInfo, UserStackInit, AT_PAGESZ},
    inner::ProcessInner,
};

// FIXME: 暂时而言应用是嵌入内核的。之后需要修复
static TMP_FS: &[(&str, &[u8])] = macros::gen_tmp_fs!();

fn find_file(name: &str) -> Option<&'static [u8]> {
    for (n, bytes) in TMP_FS {
        if *n == name {
            return Some(bytes);
        }
    }
    None
}

pub static INITPROC: Lazy<Arc<Process>> = Lazy::new(|| {
    println!("----All Apps----");
    for (name, _) in TMP_FS {
        println!("{name}");
    }
    println!("----------------");
    Process::from_path(
        CompactString::from_static_str("initproc"),
        vec![CompactString::from_static_str("initproc")],
    )
    .expect("INITPROC Failed.")
});

pub struct Process {
    pid: usize,
    pub wait4_event: Event,
    pub status: Atomic<ProcessStatus>,
    pub exit_signal: Option<Signal>,
    inner: SpinMutex<ProcessInner>,
}

impl Process {
    // TODO: 整理这些函数，抽出共同部分

    fn from_path(path: CompactString, args: Vec<CompactString>) -> Result<Arc<Self>> {
        let _enter = info_span!("spawn process", path = path, args = args).entered();
        let mut process_name = path.clone();
        for arg in args.iter().skip(1) {
            process_name.push(' ');
            process_name.push_str(arg);
        }

        let elf_data = find_file(&path).ok_or(errno::ENOENT)?;
        let elf = Elf::parse(elf_data).expect("Should be valid elf");
        let mut memory_set = MemorySet::new_bare();
        memory_set.map_kernel_areas(KERNEL_SPACE.page_table());
        let elf_end = memory_set.load_sections(&elf, elf_data);
        let mut user_sp = Thread::alloc_user_stack(0, &mut memory_set);

        // 在用户栈上推入参数、环境变量、辅助向量等
        let mut stack_init = UserStackInit::new(user_sp, Some(memory_set.page_table()));
        let argc = args.len();
        let argv_base = stack_init.init_stack(UserAppInfo {
            args,
            envs: Vec::new(),
            auxv: vec![(AT_PAGESZ, PAGE_SIZE)],
        });

        user_sp = stack_init.user_sp();
        let brk = elf_end.vpn_ceil().page_start();
        let mut tid_allocator = RecycleAllocator::new();
        let tid = tid_allocator.alloc();
        // 第一个线程，主线程，tid 为 0
        assert_eq!(tid, 0);
        let mut trap_context = TrapContext::app_init_context(elf.entry as usize, user_sp);
        *trap_context.a0_mut() = argc;
        *trap_context.a1_mut() = argv_base;
        let process = Arc::new(Process {
            pid: PID_ALLOCATOR.lock().alloc(),
            wait4_event: Event::new(),
            status: Atomic::new(ProcessStatus::normal()),
            exit_signal: None,
            inner: SpinMutex::new(ProcessInner {
                name: process_name,
                memory_set,
                heap_range: brk..brk,
                parent: None,
                children: Vec::new(),
                cwd: CompactString::from_static_str("/"),
                fd_table: FdTable::with_stdio(),
                signal_handlers: SignalHandlers::new(),
                tid_allocator,
                threads: BTreeMap::new(),
            }),
        });
        process.lock_inner_with(|inner| {
            inner.threads.insert(
                tid,
                Arc::new(Thread::new(
                    Arc::clone(&process),
                    tid,
                    trap_context,
                    KSignalSet::empty(),
                )),
            );
        });

        Ok(process)
    }

    /// fork 一个新进程，目前仅支持只有一个主线程的进程。
    ///
    /// `stack` 若不为 0 则指定新进程的栈顶
    pub fn fork(
        self: &Arc<Self>,
        stack: Option<NonZeroUsize>,
        exit_signal: Option<Signal>,
    ) -> Arc<Self> {
        let child = self.lock_inner_with(|inner| {
            assert_eq!(inner.threads.len(), 1);
            // // 复制文件描述符表
            // let new_fd_table = parent_inner.fd_table.clone();
            let parent_main_thread = inner.main_thread();
            let (mut trap_context, signal_mask) = parent_main_thread
                .lock_inner_with(|inner| (inner.trap_context.clone(), inner.signal_mask));
            if let Some(stack) = stack {
                *trap_context.sp_mut() = stack.get();
            }
            // 子进程 fork 后返回值为 0
            *trap_context.a0_mut() = 0;
            let child = Arc::new(Self {
                pid: PID_ALLOCATOR.lock().alloc(),
                wait4_event: Event::new(),
                status: Atomic::new(self.status.load(Ordering::SeqCst)),
                exit_signal,
                inner: SpinMutex::new(ProcessInner {
                    name: inner.name.clone(),
                    memory_set: MemorySet::from_existed_user(&inner.memory_set),
                    heap_range: inner.heap_range.clone(),
                    parent: Some(Arc::clone(self)),
                    children: Vec::new(),
                    cwd: inner.cwd.clone(),
                    fd_table: inner.fd_table.clone(),
                    signal_handlers: inner.signal_handlers.clone(),
                    tid_allocator: inner.tid_allocator.clone(),
                    threads: BTreeMap::new(),
                }),
            });
            child.lock_inner_with(|inner| {
                inner.threads.insert(
                    parent_main_thread.tid(),
                    Arc::new(Thread::new(
                        Arc::clone(&child),
                        parent_main_thread.tid(),
                        trap_context,
                        signal_mask,
                    )),
                )
            });
            // 新进程添入原进程的子进程表
            inner.children.push(Arc::clone(&child));
            child
        });
        // 子进程的主线程可以加入调度队列中了
        child.lock_inner_with(|inner| thread::spawn_user_thread(inner.main_thread()));
        child
    }

    /// 根据 `path` 加载一个新的 ELF 文件并执行。目前要求原进程仅有一个线程并且没有子进程
    pub fn exec(&self, path: CompactString, args: Vec<CompactString>) -> Result<()> {
        let mut process_name = path.clone();
        for arg in args.iter().skip(1) {
            process_name.push(' ');
            process_name.push_str(arg);
        }

        let elf_data = find_file(&path).ok_or_else(|| {
            info!("executable does not exist");
            errno::ENOENT
        })?;
        // TODO: [low] 处理 elf 文件无效的情况
        let elf = Elf::parse(elf_data).expect("Should be valid elf");
        // TODO: [mid]: 处理 close_on_exec
        self.lock_inner_with(|inner| {
            // TODO: 如果是多线程情况下，应该需要先终结其它线程？有子进程可能也类似？
            assert_eq!(inner.threads.len(), 1);
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
            inner.signal_handlers = SignalHandlers::new();

            let mut stack_init = UserStackInit::new(user_sp, None);
            let argc = args.len();
            let argv_base = stack_init.init_stack(UserAppInfo {
                args,
                envs: Vec::new(),
                auxv: vec![(AT_PAGESZ, PAGE_SIZE)],
            });
            user_sp = stack_init.user_sp();

            inner.main_thread().lock_inner_with(|inner| {
                inner.trap_context = TrapContext::app_init_context(elf.entry as usize, user_sp);
                *inner.trap_context.a0_mut() = argc;
                *inner.trap_context.a1_mut() = argv_base;
            });
        });

        Ok(())
    }

    pub fn lock_inner(&self) -> SpinMutexGuard<'_, ProcessInner> {
        self.inner.lock()
    }

    /// 锁 inner 然后进行操作，算是个快捷方法。尽量避免同时拿多个锁
    pub fn lock_inner_with<T>(&self, f: impl FnOnce(&mut ProcessInner) -> T) -> T {
        f(&mut self.inner.lock())
    }

    pub fn pid(&self) -> usize {
        self.pid
    }

    // pub fn is_normal(&self) -> bool {
    //     self.status.load(Ordering::SeqCst).0 & (0b1111_1111 << 8) == (0 << 8)
    // }

    pub fn is_exited(&self) -> bool {
        self.status.load(Ordering::SeqCst).0 & (0b1111_1111 << 8) == (1 << 8)
    }

    pub fn is_zombie(&self) -> bool {
        self.status.load(Ordering::SeqCst).0 & (0b1111_1111 << 8) == (2 << 8)
    }

    pub fn exit_code(&self) -> Option<i8> {
        let status = self.status.load(Ordering::SeqCst);
        if status == ProcessStatus::normal() {
            return None;
        }
        Some((status.0 & 0b1111_1111) as i8)
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        PID_ALLOCATOR.lock().dealloc(self.pid);
    }
}

static PID_ALLOCATOR: SpinMutex<RecycleAllocator> = SpinMutex::new(RecycleAllocator::begin_with(1));

/// 退出进程，终止其所有线程。
///
/// 但注意，其他线程此时可能正在运行，因此终止不是立刻发生的，仅仅只是标记该进程为退出，而不回收资源
///
/// 其他线程在进入内核时会检查对应的进程是否已标记为退出从而决定是否退出
pub fn exit_process(process: &Process, exit_code: i8) {
    info!("Process exits with code {exit_code}");
    let new_status = ProcessStatus::exited(exit_code);
    let old_status = process.status.swap(new_status, Ordering::SeqCst);
    assert_eq!(old_status, ProcessStatus::normal());
}

/// 标记一个进程的状态，其中低 8 位记录 exit code
///
/// 高 8 位的可能有如下几种：
/// - 0: 进程处于正常状态下
/// - 1: 进程标记为退出，但资源尚未回收
/// - 2: 进程资源已回收，成为僵尸等待父进程 wait
#[derive(bytemuck::NoUninit, Copy, Clone, Debug, PartialEq, Eq)]
#[repr(transparent)]
pub struct ProcessStatus(u16);

impl ProcessStatus {
    pub fn normal() -> Self {
        Self(0)
    }

    pub fn exited(exit_code: i8) -> Self {
        Self((1 << 8) | (exit_code as u8 as u16))
    }

    pub fn zombie(exit_code: i8) -> Self {
        Self((2 << 8) | (exit_code as u8 as u16))
    }
}
