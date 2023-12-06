#[macro_export]
macro_rules! error {
    ($($arg:tt)+) => ($crate::log($crate::Level::Error, ::core::format_args!($($arg)+)))
}

#[macro_export]
macro_rules! warn {
    ($($arg:tt)+) => ($crate::log($crate::Level::Warn, ::core::format_args!($($arg)+)))
}

#[macro_export]
macro_rules! info {
    ($($arg:tt)+) => ($crate::log($crate::Level::Info, ::core::format_args!($($arg)+)))
}

#[macro_export]
macro_rules! debug {
    ($($arg:tt)+) => ($crate::log($crate::Level::Debug, ::core::format_args!($($arg)+)))
}

#[macro_export]
macro_rules! trace {
    ($($arg:tt)+) => ($crate::log($crate::Level::Trace, ::core::format_args!($($arg)+)))
}

#[macro_export]
macro_rules! span {
    // span!(Level::Info, "sys_clone");
    ($lvl:expr, $name:expr) => {
        if ($lvl <= $crate::CLOG || $lvl <= $crate::FLOG) && $lvl <= $crate::SLOG {
            $crate::Span::new(
                $lvl,
                $name,
                ::core::option::Option::None,
            )
        } else {
            $crate::Span::disabled()
        }
    };
    // span!(Level::Info, "sys_clone", key1 = 42, key2 = true);
    ($lvl:expr, $name:expr, $($key:tt = $value:expr),+) => {
        if ($lvl <= $crate::CLOG || $lvl <= $crate::FLOG) && $lvl <= $crate::SLOG {
            $crate::Span::new(
                $lvl,
                $name,
                core::option::Option::Some(&[$((::core::stringify!($key), &$value)),+]),
            )
        } else {
            $crate::Span::disabled()
        }
    };
}

#[macro_export]
macro_rules! trace_span {
    ($name:expr) => { $crate::span!($crate::Level::Trace, $name) };
    ($name:expr, $($args:tt)+) => ($crate::span!($crate::Level::Trace, $name, $($args)+));
}

#[macro_export]
macro_rules! debug_span {
    ($name:expr) => { $crate::span!($crate::Level::Debug, $name) };
    ($name:expr, $($args:tt)+) => ($crate::span!($crate::Level::Debug, $name, $($args)+));
}

#[macro_export]
macro_rules! info_span {
    ($name:expr) => { $crate::span!($crate::Level::Info, $name) };
    ($name:expr, $($args:tt)+) => ($crate::span!($crate::Level::Info, $name, $($args)+));
}

#[macro_export]
macro_rules! warn_span {
    ($name:expr) => { $crate::span!($crate::Level::Warn, $name) };
    ($name:expr, $($args:tt)+) => ($crate::span!($crate::Level::Warn, $name, $($args)+));
}

#[macro_export]
macro_rules! error_span {
    ($name:expr) => { $crate::span!($crate::Level::Error, $name) };
    ($name:expr, $($args:tt)+) => ($crate::span!($crate::Level::Error, $name, $($args)+));
}
