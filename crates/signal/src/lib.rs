#![no_std]

mod action;
mod handlers;
mod receiver;

pub use action::SignalAction;
pub use handlers::SignalHandlers;
pub use receiver::SignalReceiver;

use bitflags::bitflags;
use num_enum::TryFromPrimitive;

bitflags! {
    #[derive(Clone, Copy, Debug)]
    pub struct SignalSet: u64 {
        const SIGDEF = 1 << 0;
        const SIGHUP = 1 << 1;
        const SIGINT = 1 << 2;
        const SIGQUIT = 1 << 3;
        const SIGILL = 1 << 4;
        const SIGTRAP = 1 << 5;
        const SIGABRT = 1 << 6;
        const SIGBUS = 1 << 7;
        const SIGFPE = 1 << 8;
        const SIGKILL = 1 << 9;
        const SIGUSR1 = 1 << 10;
        const SIGSEGV = 1 << 11;
        const SIGUSR2 = 1 << 12;
        const SIGPIPE = 1 << 13;
        const SIGALRM = 1 << 14;
        const SIGTERM = 1 << 15;
        const SIGSTKFLT = 1 << 16;
        const SIGCHLD = 1 << 17;
        const SIGCONT = 1 << 18;
        const SIGSTOP = 1 << 19;
        const SIGTSTP = 1 << 20;
        const SIGTTIN = 1 << 21;
        const SIGTTOU = 1 << 22;
        const SIGURG = 1 << 23;
        const SIGXCPU = 1 << 24;
        const SIGXFSZ = 1 << 25;
        const SIGVTALRM = 1 << 26;
        const SIGPROF = 1 << 27;
        const SIGWINCH = 1 << 28;
        const SIGIO = 1 << 29;
        const SIGPWR = 1 << 30;
        const SIGSYS = 1 << 31;
        const SIGRTMIN = 1 << 32;
        const SIGRT1 = 1 << 33;
        const SIGRT2 = 1 << 34;
        const SIGRT3 = 1 << 35;
        const SIGRT4 = 1 << 36;
        const SIGRT5 = 1 << 37;
        const SIGRT6 = 1 << 38;
        const SIGRT7 = 1 << 39;
        const SIGRT8 = 1 << 40;
        const SIGRT9 = 1 << 41;
        const SIGRT10 = 1 << 42;
        const SIGRT11 = 1 << 43;
        const SIGRT12 = 1 << 44;
        const SIGRT13 = 1 << 45;
        const SIGRT14 = 1 << 46;
        const SIGRT15 = 1 << 47;
        const SIGRT16 = 1 << 48;
        const SIGRT17 = 1 << 49;
        const SIGRT18 = 1 << 50;
        const SIGRT19 = 1 << 51;
        const SIGRT20 = 1 << 52;
        const SIGRT21 = 1 << 53;
        const SIGRT22 = 1 << 54;
        const SIGRT23 = 1 << 55;
        const SIGRT24 = 1 << 56;
        const SIGRT25 = 1 << 57;
        const SIGRT26 = 1 << 58;
        const SIGRT27 = 1 << 59;
        const SIGRT28 = 1 << 60;
        const SIGRT29 = 1 << 61;
        const SIGRT30 = 1 << 62;
        const SIGRT31 = 1 << 63;
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, TryFromPrimitive)]
#[repr(u8)]
#[allow(clippy::upper_case_acronyms)]
pub enum Signal {
    ERR = 0,
    SIGHUP = 1,
    SIGINT = 2,
    SIGQUIT = 3,
    SIGILL = 4,
    SIGTRAP = 5,
    SIGABRT = 6,
    SIGBUS = 7,
    SIGFPE = 8,
    SIGKILL = 9,
    SIGUSR1 = 10,
    SIGSEGV = 11,
    SIGUSR2 = 12,
    SIGPIPE = 13,
    SIGALRM = 14,
    SIGTERM = 15,
    SIGSTKFLT = 16,
    SIGCHLD = 17,
    SIGCONT = 18,
    SIGSTOP = 19,
    SIGTSTP = 20,
    SIGTTIN = 21,
    SIGTTOU = 22,
    SIGURG = 23,
    SIGXCPU = 24,
    SIGXFSZ = 25,
    SIGVTALRM = 26,
    SIGPROF = 27,
    SIGWINCH = 28,
    SIGIO = 29,
    SIGPWR = 30,
    SIGSYS = 31,
    SIGRTMIN = 32,
    SIGRT1 = 33,
    SIGRT2 = 34,
    SIGRT3 = 35,
    SIGRT4 = 36,
    SIGRT5 = 37,
    SIGRT6 = 38,
    SIGRT7 = 39,
    SIGRT8 = 40,
    SIGRT9 = 41,
    SIGRT10 = 42,
    SIGRT11 = 43,
    SIGRT12 = 44,
    SIGRT13 = 45,
    SIGRT14 = 46,
    SIGRT15 = 47,
    SIGRT16 = 48,
    SIGRT17 = 49,
    SIGRT18 = 50,
    SIGRT19 = 51,
    SIGRT20 = 52,
    SIGRT21 = 53,
    SIGRT22 = 54,
    SIGRT23 = 55,
    SIGRT24 = 56,
    SIGRT25 = 57,
    SIGRT26 = 58,
    SIGRT27 = 59,
    SIGRT28 = 60,
    SIGRT29 = 61,
    SIGRT30 = 62,
    SIGRT31 = 63,
}
