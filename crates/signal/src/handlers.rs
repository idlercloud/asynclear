use super::{Signal, SignalAction};
use utils::config::SIGSET_SIZE;

/// 由进程持有
#[derive(Clone)]
pub struct SignalHandlers {
    actions: [SignalAction; SIGSET_SIZE],
}

impl SignalHandlers {
    pub const fn new() -> Self {
        const DEFAULT_ACTION: SignalAction = SignalAction::new();
        Self {
            actions: [DEFAULT_ACTION; SIGSET_SIZE],
        }
    }
    pub fn clear(&mut self) {
        self.actions.fill(SignalAction::new());
    }
    pub fn action(&self, signal: Signal) -> SignalAction {
        // signal < SIGSET_SIZE 必然成立，所以不会 panic
        self.actions[signal as usize]
    }
    pub fn set_action(&mut self, signal: Signal, new_action: SignalAction) {
        // signal < SIGSET_SIZE 必然成立，所以不会 panic
        self.actions[signal as usize] = new_action;
    }
}
