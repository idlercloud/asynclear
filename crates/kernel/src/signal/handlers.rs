use defines::signal::{KSignalAction, SIGSET_SIZE};

use super::Signal;

pub enum DefaultHandler {
    Terminate,
    Ignore,
    CoreDump,
    Stop,
    Continue,
}

impl DefaultHandler {
    pub fn new(signal: Signal) -> Self {
        #[allow(clippy::enum_glob_use)]
        use Signal::*;
        match signal {
            SIGABRT | SIGBUS | SIGILL | SIGQUIT | SIGSEGV | SIGSYS | SIGTRAP | SIGXCPU | SIGXFSZ => {
                DefaultHandler::CoreDump
            }
            SIGCHLD | SIGURG | SIGWINCH => DefaultHandler::Ignore,
            SIGSTOP | SIGTSTP | SIGTTIN | SIGTTOU => DefaultHandler::Stop,
            SIGCONT => DefaultHandler::Continue,
            _ => DefaultHandler::Terminate,
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

    pub fn action(&self, signal: Signal) -> &KSignalAction {
        &self.actions[signal as usize]
    }

    pub fn action_mut(&mut self, signal: Signal) -> &mut KSignalAction {
        &mut self.actions[signal as usize]
    }
}
