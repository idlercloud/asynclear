use alloc::vec::Vec;
use core::ops::Range;

use common::config::LOW_ADDRESS_END;
use hashbrown::HashMap;
use idallocator::RecycleAllocator;
use memory::{MemorySpace, VirtAddr};
use triomphe::Arc;

use super::Process;
use crate::{
    fs::{DEntryDir, FdTable},
    memory,
    signal::{KSignalSet, Signal, SignalHandlers},
    thread::Thread,
};

pub struct ProcessInner {
    // 这里添加的资源都需要考虑在 `exit_thread` 和 `sys_wait4` 时候释放 */
    // 以及在 `Process:from_path()`、`Process::clone()`、`Process::exec()` 时初始化
    /// 地址空间
    pub memory_space: MemorySpace,
    /// 用户堆的范围。
    ///
    /// `heap_range.start` 一般紧邻进程 elf 数据之后，并且创建之后不会改变
    ///
    /// `heap_range.end` 即 brk，由 `sys_brk` 系统调用控制
    pub heap_range: Range<VirtAddr>,

    // 进程
    /// 父进程引用
    pub parent: Option<Arc<Process>>,
    /// 子进程引用列表
    pub children: Vec<Arc<Process>>,
    /// 当前工作目录
    pub cwd: Arc<DEntryDir>,

    // 文件
    /// 文件描述符表
    pub fd_table: FdTable,

    // 信号
    /// 信号处理函数
    pub signal_handlers: SignalHandlers,

    // 线程
    /// 线程 tid 分配器
    pub tid_allocator: RecycleAllocator,
    /// 线程引用列表
    pub threads: HashMap<usize, Arc<Thread>>,
}

impl ProcessInner {
    pub fn main_thread(&self) -> Arc<Thread> {
        Arc::clone(self.threads.get(&0).unwrap())
    }

    /// 设置用户堆顶。失败返回原来的 brk，成功则返回新的 brk
    ///
    /// 失败的情况包括：
    ///
    /// - `new_brk` 不大于堆的开头
    /// - `new_brk` 超过低地址空间末端
    pub fn set_user_brk(&mut self, new_brk: VirtAddr) -> VirtAddr {
        if new_brk <= self.heap_range.start || new_brk.0 > LOW_ADDRESS_END {
            return self.heap_range.end;
        }
        // 由于上面的条件语句，下面一定有 `heap_start < new_end`
        let heap_start = self.heap_range.start.vpn_floor();
        let new_end = new_brk.vpn_ceil();
        self.memory_space.set_user_brk(heap_start, new_end);
        self.heap_range.end = new_brk;
        new_brk
    }

    /// 挑选一个合适的线程让其处理信号
    pub fn receive_signal(&mut self, signal: Signal) {
        for thread in self.threads.values() {
            let mut inner = thread.lock_inner();
            let signal = KSignalSet::from(signal);
            if !inner.signal_mask.contains(signal) && !inner.pending_signal.contains(signal) {
                debug!("thread {} receive signal {signal:?}", thread.tid());
                inner.pending_signal.insert(signal);
                break;
            }
        }
    }
}
