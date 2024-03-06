use core::ops::Range;

use alloc::{collections::BTreeMap, vec::Vec};
use compact_str::CompactString;
use idallocator::RecycleAllocator;
use memory::{MemorySet, VirtAddr};
use signal::SignalHandlers;
use triomphe::Arc;

use crate::thread::Thread;

use super::Process;

pub struct ProcessInner {
    /* 这里添加的资源都需要考虑在 `exit_thread` 和 `sys_wait4` 时候释放 */
    /* 以及在 `Process:from_path()`、`Process::clone()`、`Process::exec()` 时初始化 */
    pub name: CompactString,

    /* 地址空间 */
    pub memory_set: MemorySet,
    /// 用户堆的范围。
    ///
    /// `heap_range.start` 一般紧邻进程 elf 数据之后，并且创建之后不会改变
    ///
    /// `heap_range.end` 即 brk，由 `sys_brk` 系统调用控制
    pub heap_range: Range<VirtAddr>,

    /* 进程 */
    pub parent: Option<Arc<Process>>,
    pub children: Vec<Arc<Process>>,
    /// 若为 Some，则代表进程已经退出，但不一定回收了资源变为僵尸
    pub exit_code: Option<i8>,
    /// cwd 应当永远有着 `/xxx/yyy/` 的形式（包括 `/`）
    pub cwd: CompactString,

    /* 文件 */
    // pub fd_table: Vec<Option<Arc<File>>>,

    /* 信号 */
    pub signal_handlers: SignalHandlers,

    /* 线程 */
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
    //             .extend(core::iter::repeat(None).take(min - self.fd_table.len()));
    //     }
    //     if let Some(fd) = (min..self.fd_table.len()).find(|fd| self.fd_table[*fd].is_none()) {
    //         fd
    //     } else {
    //         self.fd_table.push(None);
    //         self.fd_table.len() - 1
    //     }
    // }

    pub fn main_thread(&self) -> Arc<Thread> {
        Arc::clone(self.threads.get(&0).unwrap())
    }

    /// 标记进程已退出。但是不会回收资源。
    ///
    /// 一般而言，所有线程都退出后，会真正清理资源，变为僵尸进程
    pub fn mark_exit(&mut self, exit_code: i8) {
        assert_eq!(self.exit_code, None);
        self.exit_code = Some(exit_code);
    }

    // /// 设置用户堆顶。失败返回原来的 brk，成功则返回新的 brk
    // pub fn set_user_brk(&mut self, new_brk: usize) -> usize {
    //     if new_end <= self.heap_range.end {
    //         return self.brk;
    //     }
    //     // TODO: 注，这里是假定地址空间和物理内存都够用
    //     self.memory_set.set_user_brk(new_end, self.heap_range);
    //     self.brk = new_brk;
    //     new_brk
    // }
}
