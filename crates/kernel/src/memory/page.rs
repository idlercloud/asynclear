use super::frame_allocator::Frame;

#[derive(Debug)]
pub struct Page {
    frame: Frame,
}

impl Page {
    pub fn new(frame: Frame) -> Self {
        Self { frame }
    }
}
