#[macro_export]
macro_rules! log {
    // log!(Level::Info, "a {} event", "log");
    ($level:expr, $($arg:tt)+) => {{
        $crate::log_impl($level, ::core::format_args!($($arg)+))
    }};
}

#[macro_export]
macro_rules! error {
    // error!("a {} event", "log")
    ($($arg:tt)+) => ($crate::log!($crate::Level::Error, $($arg)+))
}

#[macro_export]
macro_rules! warn {
    // warn!("a {} event", "log")
    ($($arg:tt)+) => ($crate::log!($crate::Level::Warn, $($arg)+))
}

#[macro_export]
macro_rules! info {
    // info!("a {} event", "log")
    ($($arg:tt)+) => ($crate::log!($crate::Level::Info, $($arg)+))
}

#[macro_export]
macro_rules! debug {
    // debug!("a {} event", "log")
    ($($arg:tt)+) => ($crate::log!($crate::Level::Debug, $($arg)+))
}

#[macro_export]
macro_rules! trace {
    // trace!("a {} event", "log")
    ($($arg:tt)+) => ($crate::log!($crate::Level::Trace, $($arg)+))
}
