use core::ops::Range;

use alloc::{
    sync::{Arc, Weak},
    vec::Vec,
};
use compact_str::CompactString;
use idallocator::RecycleAllocator;
use memory::{MemorySet, VirtAddr};

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
    pub parent: Weak<Process>,
    pub children: Vec<Arc<Process>>,
    /// 若为 Some，则代表进程已经是僵尸
    pub zombie_exit_code: Option<i8>,
    /// cwd 应当永远有着 `/xxx/yyy/` 的形式（包括 `/`)
    pub cwd: CompactString,

    /* 文件 */
    // pub fd_table: Vec<Option<Arc<File>>>,

    /* 线程 */
    pub tid_allocator: RecycleAllocator,
    pub threads: Vec<Option<Arc<Thread>>>,
}

impl ProcessInner {
    // pub fn new() -> Self {
    //     Self {
    //         is_zombie: false,
    //         memory_set: MemorySet::new_bare(),
    //         parent: Weak::new(),
    //         children: Vec::new(),
    //         exit_code: 0,
    //         heap_range: VirtPageNum(0),
    //         brk: 0,
    //         fd_table: vec![
    //             // 0 -> stdin
    //             Some(Arc::new(File::new(FileEntity::Stdin(Stdin)))),
    //             // 1 -> stdout
    //             Some(Arc::new(File::new(FileEntity::Stdout(Stdout)))),
    //             // 2 -> stderr
    //             Some(Arc::new(File::new(FileEntity::Stdout(Stdout)))),
    //         ],
    //         threads: Vec::new(),
    //         thread_res_allocator: RecycleAllocator::new(),
    //         cwd: "/".to_string(),
    //         sig_handlers: SignalHandlers::new(),
    //     }
    // }

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

    pub fn thread_count(&self) -> usize {
        let mut count = 0;
        for t in &self.threads {
            if t.is_some() {
                count += 1;
            }
        }
        count
    }

    pub fn main_thread(&self) -> Arc<Thread> {
        self.threads[0].as_ref().cloned().unwrap()
    }

    /// 标记进程已退出。但是不会回收资源。
    ///
    /// 一般而言，所有线程都退出后，会调用 become_zombie 真正变为僵尸进程
    pub fn mark_exit(&mut self, exit_code: i8) {
        assert_eq!(self.zombie_exit_code, None);
        self.zombie_exit_code = Some(exit_code);
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
