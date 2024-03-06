use super::SignalFlag;

pub struct SignalReceiver {
    pub mask: SignalFlag,
    received: SignalFlag,
}

impl SignalReceiver {
    #[inline]
    pub const fn new() -> Self {
        Self {
            mask: SignalFlag::empty(),
            received: SignalFlag::empty(),
        }
    }
    pub fn clear(&mut self) {
        self.mask = SignalFlag::empty();
        self.received = SignalFlag::empty();
    }
}
