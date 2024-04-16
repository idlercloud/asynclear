use super::{frame_allocator::Frame, PhysPageNum};

#[derive(Debug)]
pub struct Page {
    frame: Frame,
}

impl Page {
    pub fn new(frame: Frame) -> Self {
        Self { frame }
    }

    pub fn ppn(&self) -> PhysPageNum {
        self.frame.ppn()
    }
}
