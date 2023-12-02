use core::{cmp::Ordering, fmt};

static LOG_LEVEL_NAMES: [&str; 6] = ["OFF", "ERROR", "WARN", "INFO", "DEBUG", "TRACE"];

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub enum Level {
    Error = 1,
    Warn,
    Info,
    Debug,
    Trace,
}

impl PartialEq<LevelFilter> for Level {
    #[inline]
    fn eq(&self, other: &LevelFilter) -> bool {
        *self as u8 == *other as u8
    }
}

impl PartialOrd<LevelFilter> for Level {
    #[inline]
    fn partial_cmp(&self, other: &LevelFilter) -> Option<Ordering> {
        (*self as u8).partial_cmp(&(*other as u8))
    }
}

impl fmt::Display for Level {
    #[inline]
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.pad(self.as_str())
    }
}

impl Level {
    pub fn as_str(&self) -> &'static str {
        LOG_LEVEL_NAMES[*self as usize]
    }
}

#[repr(usize)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub enum LevelFilter {
    Off,
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl PartialEq<Level> for LevelFilter {
    #[inline]
    fn eq(&self, other: &Level) -> bool {
        other.eq(self)
    }
}

impl PartialOrd<Level> for LevelFilter {
    #[inline]
    fn partial_cmp(&self, other: &Level) -> Option<Ordering> {
        (*self as u8).partial_cmp(&(*other as u8))
    }
}

impl fmt::Display for LevelFilter {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.pad(self.as_str())
    }
}

impl LevelFilter {
    pub fn as_str(&self) -> &'static str {
        LOG_LEVEL_NAMES[*self as usize]
    }
}

pub const CLOG: LevelFilter = decide_log_level(option_env!("KERNEL_CLOG"));
pub const FLOG: LevelFilter = decide_log_level(option_env!("KERNEL_FLOG"));

const fn decide_log_level(level_str: Option<&str>) -> LevelFilter {
    const fn str_eq(lhs: &str, rhs: &str) -> bool {
        let lhs = lhs.as_bytes();
        let rhs = rhs.as_bytes();

        if lhs.len() != rhs.len() {
            return false;
        }
        let mut i = 0;
        while i < lhs.len() {
            if lhs[i] != rhs[i] {
                return false;
            }
            i += 1;
        }
        true
    }

    if let Some(level_str) = level_str {
        if str_eq(level_str, "TRACE") {
            LevelFilter::Trace
        } else if str_eq(level_str, "DEBUG") {
            LevelFilter::Debug
        } else if str_eq(level_str, "INFO") {
            LevelFilter::Info
        } else if str_eq(level_str, "WARN") {
            LevelFilter::Warn
        } else if str_eq(level_str, "ERROR") {
            LevelFilter::Error
        } else {
            LevelFilter::Off
        }
    } else {
        LevelFilter::Off
    }
}
