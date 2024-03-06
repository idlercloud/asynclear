use crate::Signal;

use super::SignalAction;
use defines::config::SIGSET_SIZE;

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

    pub fn action(&self, signal: Signal) -> &SignalAction {
        &self.actions[signal as usize]
    }

    pub fn action_mut(&mut self, signal: Signal) -> &mut SignalAction {
        &mut self.actions[signal as usize]
    }
}
