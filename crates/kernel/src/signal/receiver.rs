use super::KSignalSet;

pub struct SignalReceiver {
    mask: KSignalSet,
    received: KSignalSet,
}

impl SignalReceiver {
    #[inline]
    pub const fn new() -> Self {
        Self {
            mask: KSignalSet::empty(),
            received: KSignalSet::empty(),
        }
    }
    pub fn clear(&mut self) {
        self.mask = KSignalSet::empty();
        self.received = KSignalSet::empty();
    }
}
