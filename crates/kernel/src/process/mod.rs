mod inner;
mod manager;

use alloc::{vec, vec::Vec};
use core::num::NonZeroUsize;

use atomic::{Atomic, Ordering};
use defines::error::{errno, KResult};
use ecow::EcoString;
use elf::Elf;
use event_listener::Event;
use hashbrown::HashMap;
use idallocator::RecycleAllocator;
use klocks::{SpinMutex, SpinMutexGuard};
use manager::ProcessManager;
use memory::MemorySpace;
use triomphe::Arc;

use self::inner::ProcessInner;
use crate::{
    executor,
    fs::{self, DEntry, FdTable, VFS},
    memory,
    signal::{KSignalSet, Signal, SignalHandlers},
    thread::{self, Thread},
    trap::TrapContext,
};

pub static PROCESS_MANAGER: ProcessManager = ProcessManager::new();
pub const INITPROC_PID: usize = 1;

pub fn init() {
    let init_proc = Process::from_path("/initproc", vec![EcoString::from("/initproc")]).expect("INITPROC Failed.");
    PROCESS_MANAGER.add(INITPROC_PID, init_proc);
}

pub struct Process {
    pid: usize,
    /// 用于 `sys_wait4` 唤醒
    pub wait4_event: Event,
    /// 进程状态，指示是否成为僵尸或已退出。如已退出，则其中还包含了退出码
    pub status: Atomic<ProcessStatus>,
    /// 退出时向父进程发送的信号
    pub exit_signal: Option<Signal>,
    inner: SpinMutex<ProcessInner>,
}

impl Process {
    // TODO: 整理这些函数，抽出共同部分

    /// `path` 需要是绝对路径
    fn from_path(path: &str, args: Vec<EcoString>) -> KResult<Arc<Self>> {
        let _enter = info_span!("spawn process", path = path, args = args).entered();

        let mut memory_space;
        let (elf_end, auxv, elf_entry) = {
            let DEntry::Bytes(bytes) = fs::find_file(path)? else {
                return Err(errno::EISDIR);
            };
            let elf_data = executor::block_on(fs::read_file(bytes.inode()))?;
            let elf = Elf::parse(&elf_data).map_err(|e| {
                warn!("parse elf error {e}");
                errno::ENOEXEC
            })?;

            memory_space = MemorySpace::empty_user();
            memory_space.load_elf_sections(&elf, &elf_data)?
        };

        // 在用户栈上推入参数、环境变量、辅助向量等
        let argc = args.len();
        let (user_sp, argv_base) = memory_space.init_stack(0, args, Vec::new(), auxv);

        let brk = elf_end.vpn_ceil().page_start();
        let mut tid_allocator = RecycleAllocator::new();
        let tid = tid_allocator.alloc();
        // 第一个线程，主线程，tid 为 0
        assert_eq!(tid, 0);
        let mut trap_context = TrapContext::app_init_context(elf_entry, user_sp);
        *trap_context.a0_mut() = argc;
        *trap_context.a1_mut() = argv_base;
        let process = Arc::new(Process {
            pid: PID_ALLOCATOR.lock().alloc(),
            wait4_event: Event::new(),
            status: Atomic::new(ProcessStatus::normal()),
            exit_signal: None,
            inner: SpinMutex::new(ProcessInner {
                memory_space,
                heap_range: brk..brk,
                parent: None,
                children: Vec::new(),
                cwd: Arc::clone(VFS.root_dir()),
                fd_table: FdTable::with_stdio(),
                signal_handlers: SignalHandlers::new(),
                tid_allocator,
                threads: HashMap::new(),
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
    pub fn fork(self: &Arc<Self>, stack: Option<NonZeroUsize>, exit_signal: Option<Signal>) -> Arc<Self> {
        let child = self.lock_inner_with(|inner| {
            assert_eq!(inner.threads.len(), 1);
            let parent_main_thread = inner.main_thread();
            let signal_mask = parent_main_thread.lock_inner_with(|inner| inner.signal_mask);
            let mut trap_context = unsafe { parent_main_thread.get_owned().as_mut().trap_context.clone() };
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
                    memory_space: MemorySpace::from_other(&inner.memory_space),
                    heap_range: inner.heap_range.clone(),
                    parent: Some(Arc::clone(self)),
                    children: Vec::new(),
                    cwd: Arc::clone(&inner.cwd),
                    fd_table: inner.fd_table.clone(),
                    signal_handlers: inner.signal_handlers.clone(),
                    tid_allocator: inner.tid_allocator.clone(),
                    threads: HashMap::new(),
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
            PROCESS_MANAGER.add(child.pid(), Arc::clone(&child));
            // 新进程添入原进程的子进程表
            inner.children.push(Arc::clone(&child));
            child
        });
        // 子进程的主线程可以加入调度队列中了
        child.lock_inner_with(|inner| thread::spawn_user_thread(inner.main_thread()));
        child
    }

    /// 根据 `elf_data` 加载一个新的 ELF 文件并执行。
    ///
    /// 目前要求原进程仅有一个线程并且没有子进程
    pub fn exec(&self, elf_data: &[u8], args: Vec<EcoString>, envs: Vec<EcoString>) -> KResult<()> {
        // let bytes = {
        //     let DEntry::Bytes(bytes) =
        //         fs::find_file(self.lock_inner_with(|inner| Arc::clone(&inner.cwd)), &path)?
        //     else {
        //         return Err(errno::EISDIR);
        //     };
        //     bytes.into_inode()
        // };
        // let header_buf = {
        //     let mut buf = MaybeUninit::uninit_array::<{ elf64::header::SIZEOF_EHDR }>();
        //     let n_read = bytes.inner.read_at(bytes.meta(), buf.as_out(), 0)?;
        //     if n_read != elf64::header::SIZEOF_EHDR {
        //         return Err(errno::ENOEXEC);
        //     }
        //     // SAFETY: 如上已保证全部读取
        //     unsafe { MaybeUninit::array_assume_init(buf) }
        // };
        // let elf: goblin::error::Result<Elf<'_>> = try {
        //     let header = Elf::parse_header(&header_buf)?;
        //     Elf::lazy_parse(header)?
        // };
        // let mut elf = elf.map_err(|e| {
        //     warn!("parse elf header error {e}");
        //     errno::ENOEXEC
        // })?;
        // {
        //     let header = &elf.header;
        //     let ph_size = header.e_phnum as usize * header.e_phentsize as usize;
        //     let mut buf = Vec::new();
        //     let out = buf.reserve_uninit(ph_size).as_out();
        //     let n_read = bytes
        //         .inner
        //         .read_at(bytes.meta(), out, header.e_phoff as usize)?;
        //     if n_read != ph_size {
        //         return Err(errno::ENOEXEC);
        //     }
        //     // SAFETY: 如上已保证全部读取
        //     unsafe {
        //         buf.set_len(ph_size);
        //     }

        //     let ctx = Ctx::new(
        //         if elf.is_64 {
        //             Container::Big
        //         } else {
        //             Container::Little
        //         },
        //         if elf.little_endian {
        //             Endian::Little
        //         } else {
        //             Endian::Big
        //         },
        //     );
        //     elf.program_headers = ProgramHeader::parse(&buf, 0, elf.header.e_phnum as usize, ctx)
        //         .map_err(|e| {
        //         warn!("parse program header error {e}");
        //         errno::ENOEXEC
        //     })?;
        // }

        let elf = Elf::parse(elf_data).map_err(|e| {
            warn!("parse elf error {e}");
            errno::ENOEXEC
        })?;
        let ret = self.lock_inner_with(|inner| {
            // TODO: 如果是多线程情况下，应该需要先终结其它线程？有子进程可能也类似？
            assert_eq!(inner.threads.len(), 1);
            assert_eq!(inner.children.len(), 0);
            inner.memory_space.recycle_user_pages();
            // TODO: 执行新进程过程中发生错误，该退出还是恢复？
            let (elf_end, auxv, elf_entry) = inner.memory_space.load_elf_sections(&elf, elf_data)?;
            inner.heap_range = {
                let brk = elf_end.vpn_ceil().page_start();
                brk..brk
            };
            inner.fd_table.close_on_exec();
            inner.signal_handlers = SignalHandlers::new();

            let argc = args.len();
            let (user_sp, argv_base) = inner.memory_space.init_stack(0, args, envs, auxv);
            memory::flush_tlb(None);

            // TODO: [low] 也许可以直接原地修改？
            let trap_context = unsafe { &mut inner.main_thread().get_owned().as_mut().trap_context };

            *trap_context = TrapContext::app_init_context(elf_entry, user_sp);
            *trap_context.a0_mut() = argc;
            *trap_context.a1_mut() = argv_base;
            Ok(())
        });
        if ret.is_err() {
            exit_process(self, -10);
        }
        ret
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
