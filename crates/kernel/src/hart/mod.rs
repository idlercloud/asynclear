use alloc::vec::Vec;
use core::{
    arch::asm,
    cell::{Ref, RefCell, SyncUnsafeCell},
    sync::atomic::{AtomicBool, Ordering},
};

use common::config::HART_NUM;
use crossbeam_utils::CachePadded;
use kernel_tracer::SpanId;
use memory::KERNEL_SPACE;
use triomphe::Arc;

use crate::{
    drivers::{self, qemu_block::BLOCK_SIZE},
    fs, memory,
    process::{Process, INITPROC},
    thread::{self, Thread},
};

core::arch::global_asm!(include_str!("entry.S"));

// `CachePadded` 可以保证 per-cpu 的结构位于不同的 cache line 中
// 因此避免 false sharing
static HARTS: [SyncUnsafeCell<CachePadded<Hart>>; HART_NUM] =
    [const { SyncUnsafeCell::new(CachePadded::new(Hart::new())) }; HART_NUM];

/// # SAFETY
/// Hart 结构实际上只会被对应的 hart 访问
unsafe impl Sync for Hart {}

/// 可以认为代表一个处理器。存放一些 per-hart 的数据
///
/// 因此，一般可以假定不会被并行访问
pub struct Hart {
    hart_id: usize,
    // TODO: 内核线程是不是会不太一样？
    /// 当前 hart 上正在运行的线程。
    thread: RefCell<Option<Arc<Thread>>>,
    pub span_stack: RefCell<Vec<SpanId>>,
    /// 用于读磁盘的缓冲区，避免在栈上反复开辟空间
    pub block_buffer: RefCell<[u8; BLOCK_SIZE]>,
}

impl Hart {
    pub const fn new() -> Hart {
        Hart {
            hart_id: 0,
            thread: RefCell::new(None),
            span_stack: RefCell::new(Vec::new()),
            block_buffer: RefCell::new([0; BLOCK_SIZE]),
        }
    }

    pub fn hart_id(&self) -> usize {
        self.hart_id
    }

    pub fn replace_thread(&self, new_thread: Option<Arc<Thread>>) -> Option<Arc<Thread>> {
        core::mem::replace(&mut self.thread.borrow_mut(), new_thread)
    }

    pub fn curr_thread(&self) -> Ref<'_, Thread> {
        Ref::map(self.thread.borrow(), |t| t.as_ref().unwrap().as_ref())
    }

    pub fn curr_process(&self) -> Ref<'_, Process> {
        Ref::map(self.curr_thread(), |t| t.process.as_ref())
    }

    pub fn curr_process_arc(&self) -> Ref<'_, Arc<Process>> {
        Ref::map(self.curr_thread(), |t| &t.process)
    }
}

#[no_mangle]
pub extern "C" fn __hart_entry(hart_id: usize) -> ! {
    static INIF_HART: AtomicBool = AtomicBool::new(true);
    static INIT_FINISHED: AtomicBool = AtomicBool::new(false);

    // 主核启动
    if INIF_HART
        .compare_exchange(true, false, Ordering::Acquire, Ordering::Relaxed)
        .is_ok()
    {
        clear_bss();
        unsafe {
            set_local_hart(hart_id);
            memory::init();
        }
        KERNEL_SPACE.activate();
        // drivers 依赖于 mmio 映射（其实也许可以放在 boot page table 里？）
        drivers::init();
        // log 实现依赖于 uart 和 virtio_block
        crate::tracer::init();
        memory::log_kernel_sections();

        fs::init();

        thread::spawn_user_thread(INITPROC.lock_inner_with(|inner| inner.main_thread()));
        info!("Init hart {hart_id} started");
        INIT_FINISHED.store(true, Ordering::SeqCst);

        // 将下面的代码取消注释即可启动多核
        // for i in 0..HART_NUM {
        //     if i == hart_id {
        //         continue;
        //     }
        //     sbi_rt::hart_start(i, HART_START_ADDR, 0);
        // }
    } else {
        while !INIT_FINISHED.load(Ordering::SeqCst) {
            core::hint::spin_loop();
        }
        unsafe {
            set_local_hart(hart_id);
        }
        KERNEL_SPACE.activate();
        info!("Hart {hart_id} started");
    }

    let _enter = info_span!("hart", id = hart_id).entered();

    // 允许在内核态下访问用户数据
    crate::trap::init();

    crate::kernel_loop();
}

fn clear_bss() {
    extern "C" {
        fn sbss();
        fn ebss();
    }
    let len = ebss as usize - sbss as usize;
    // 为 debug 模式做的优化，减少启动时间
    #[cfg(debug_assertions)]
    {
        // 似乎 `BATCH_SIZE` 为 4096 时效果最好
        // 是因为恰好为一个 `PAGE_SIZE` 吗
        const BATCH_SIZE: usize = 4096;
        let batch_end = sbss as usize + len / BATCH_SIZE * BATCH_SIZE;
        unsafe {
            core::slice::from_raw_parts_mut(
                sbss as usize as *mut [u8; BATCH_SIZE],
                len / BATCH_SIZE,
            )
            .fill([0; BATCH_SIZE]);
            core::slice::from_raw_parts_mut(batch_end as *mut u8, ebss as usize - batch_end)
                .fill(0);
        }
    }
    #[cfg(not(debug_assertions))]
    unsafe {
        core::slice::from_raw_parts_mut(sbss as *mut u8, len).fill(0);
    }
}

/// 设置当前 hart 的 `Hart` 结构，将 `tp` 设置为其地址
///
/// # Safety
///
/// 需保证由不同 hart 调用
unsafe fn set_local_hart(hart_id: usize) {
    unsafe {
        let hart_ptr = HARTS[hart_id].get();
        (*hart_ptr).hart_id = hart_id;
        asm!("mv tp, {}", in(reg) hart_ptr as usize);
    }
}

pub fn local_hart<'a>() -> &'a Hart {
    let tp: usize;
    unsafe {
        asm!("mv {}, tp", out(reg) tp);
        &*(tp as *const Hart)
    }
}
