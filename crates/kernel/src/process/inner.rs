use alloc::{collections::BTreeMap, vec::Vec};
use core::ops::Range;

use compact_str::CompactString;
use defines::signal::{KSignalSet, Signal};
use idallocator::RecycleAllocator;
use memory::{MemorySpace, VirtAddr};
use triomphe::Arc;

use super::Process;
use crate::{
    fs::{DEntryDir, FdTable},
    memory,
    signal::SignalHandlers,
    thread::Thread,
};

pub struct ProcessInner {
    // 这里添加的资源都需要考虑在 `exit_thread` 和 `sys_wait4` 时候释放 */
    // 以及在 `Process:from_path()`、`Process::clone()`、`Process::exec()` 时初始化
    pub name: CompactString,

    // 地址空间
    pub memory_space: MemorySpace,
    /// 用户堆的范围。
    ///
    /// `heap_range.start` 一般紧邻进程 elf 数据之后，并且创建之后不会改变
    ///
    /// `heap_range.end` 即 brk，由 `sys_brk` 系统调用控制
    pub heap_range: Range<VirtAddr>,

    // 进程
    pub parent: Option<Arc<Process>>,
    pub children: Vec<Arc<Process>>,
    /// cwd 应当永远有着 `/xxx/yyy/` 的形式（包括 `/`）
    pub cwd: Arc<DEntryDir>,

    // 文件
    pub fd_table: FdTable,

    // 信号
    pub signal_handlers: SignalHandlers,

    // 线程
    pub tid_allocator: RecycleAllocator,
    pub threads: BTreeMap<usize, Arc<Thread>>,
}

impl ProcessInner {
    // pub fn alloc_fd(&mut self) -> usize {
    //     self.alloc_fd_from(0)
    // }

    // /// 分配出来的 fd 必然不小于 `min`
    // pub fn alloc_fd_from(&mut self, min: usize) -> usize {
    //     if min > self.fd_table.len() {
    //         self.fd_table
    //             .extend(core::iter::repeat(None).take(min -
    // self.fd_table.len()));     }
    //     if let Some(fd) = (min..self.fd_table.len()).find(|fd|
    // self.fd_table[*fd].is_none()) {         fd
    //     } else {
    //         self.fd_table.push(None);
    //         self.fd_table.len() - 1
    //     }
    // }

    pub fn main_thread(&self) -> Arc<Thread> {
        Arc::clone(self.threads.get(&0).unwrap())
    }

    // /// 设置用户堆顶。失败返回原来的 brk，成功则返回新的 brk
    // pub fn set_user_brk(&mut self, new_brk: usize) -> usize {
    //     if new_end <= self.heap_range.end {
    //         return self.brk;
    //     }
    //     // TODO: 注，这里是假定地址空间和物理内存都够用
    //     self.memory_space.set_user_brk(new_end, self.heap_range);
    //     self.brk = new_brk;
    //     new_brk
    // }

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
