use crate::Signal;

use defines::{config::SIGSET_SIZE, structs::KSignalAction};

pub enum DefaultHandler {
    Terminate,
    Ignore,
    CoreDump,
    Stop,
    Continue,
}

impl DefaultHandler {
    #[allow(clippy::enum_glob_use)]
    pub fn new(signal: Signal) -> Self {
        use DefaultHandler::*;
        use Signal::*;
        match signal {
            SIGABRT | SIGBUS | SIGILL | SIGQUIT | SIGSEGV | SIGSYS | SIGTRAP | SIGXCPU
            | SIGXFSZ => CoreDump,
            SIGCHLD | SIGURG | SIGWINCH => Ignore,
            SIGSTOP | SIGTSTP | SIGTTIN | SIGTTOU => Stop,
            SIGCONT => Continue,
            _ => Terminate,
        }
    }
}

/// 由进程持有
#[derive(Clone)]
pub struct SignalHandlers {
    actions: [KSignalAction; SIGSET_SIZE],
}

impl SignalHandlers {
    pub const fn new() -> Self {
        const DEFAULT_ACTION: KSignalAction = KSignalAction::new();
        Self {
            actions: [DEFAULT_ACTION; SIGSET_SIZE],
        }
    }

    pub fn clear(&mut self) {
        self.actions.fill(KSignalAction::new());
    }

    pub fn action(&self, signal: Signal) -> &KSignalAction {
        &self.actions[signal as usize]
    }

    pub fn action_mut(&mut self, signal: Signal) -> &mut KSignalAction {
        &mut self.actions[signal as usize]
    }
}
