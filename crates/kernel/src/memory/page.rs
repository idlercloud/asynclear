use klocks::{SpinMutex, SpinMutexGuard};

use super::frame_allocator::Frame;

pub struct Page {
    frame: SpinMutex<Frame>,
}

impl Page {
    pub fn new(frame: Frame) -> Self {
        Self {
            frame: SpinMutex::new(frame),
        }
    }

    pub fn frame(&self) -> SpinMutexGuard<'_, Frame> {
        self.frame.lock()
    }
}
