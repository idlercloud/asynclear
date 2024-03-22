use core::task::Waker;

use heapless::Deque;

const TTY_BUFFER_LEN: usize = 128;

pub struct Tty {
    pub(crate) queue: Deque<u8, TTY_BUFFER_LEN>,
    // 这里似乎可以用 futures::AtomicWaker 代替？
    pub(crate) waker: Option<Waker>,
}

impl Tty {
    pub fn get_byte(&mut self) -> Option<u8> {
        self.queue.pop_front()
    }

    /// 注册一个任务。如果原来已经注册过，就将旧的任务唤醒。
    ///
    /// 这是为了防止多个进程读时永久阻塞不被唤醒
    pub fn register_waker(&mut self, waker: Waker) {
        if let Some(old_waker) = self.waker.replace(waker) {
            old_waker.wake();
        }
    }
}
