//! 内存页的抽象，类似于 linux 中的 `struct page`
//!
//! 可以有文件作为后备
//!
//! 总体而言在几个地方用到：
//!
//! 1. 送入中断处理器，作为磁盘读写的缓冲区
//! 2. 用户的 ELF 信息、非匿名 mmap 映射、页缓存是有文件后备的
//! 3. （不考虑交换的情况下）用户栈、堆、匿名 mmap 映射是无文件后备的
//!
//! 注意用作页表的帧和 DMA 的帧不是以这种形式管理的。
//!
//! 这些页的读写可能发生在多种地方：
//!
//! 1. 用户态 mmap 后直接读写
//! 2. 读写文件操作时，如有未过时的页缓存，则直接读取；否则会进行磁盘请求
//! 3. 待补充
//!
//! 页的写操作会导致页被设置为
//! Dirty。注意这是整个页的属性，尽管可能只有其中一个块被写入了。Dirty
//! 的页最终会被写回磁盘中
//!
//! # Race Condition
//!
//! 读写可能会发生 race。首先，用户态的读写是无法侦测的，最多只能采取类似于 COW
//! 之类的手段拦截一次。
//!
//! 也就是说，比如用户以一个文件为后备 mmap
//! 之后，有可能直接内存读写该文件，并同时 `read()` 或 `write()` 该文件。
//!
//! 这种 race 是无法从内核态阻止的，用户应当自行保证不创造这样的 race
//! condition。
//!
//! 排除用户态通过 mmap 直接读写后。主要的 race 就来自于 `read()`、`write()`
//! 系统调用，需要有锁来保护。

use klocks::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use super::frame_allocator::Frame;

pub struct Page {
    frame: RwLock<Frame>,
}

impl Page {
    pub fn with_frame(frame: Frame) -> Self {
        Self {
            frame: RwLock::new(frame),
        }
    }

    pub fn frame(&self) -> RwLockReadGuard<'_, Frame> {
        self.frame.read()
    }

    pub fn frame_mut(&self) -> RwLockWriteGuard<Frame> {
        self.frame.write()
    }
}
