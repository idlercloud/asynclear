#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Error(core::ffi::c_int);

impl Error {
    #[inline]
    pub fn as_isize(self) -> isize {
        self.0 as isize
    }
}

pub type KResult<T = isize> = core::result::Result<T, Error>;

pub mod errno {
    macro_rules! declare_errno {
        ($($name:tt, $errno:literal, $desc:literal,)*) => {
            $(#[doc = $desc]
            pub const $name: super::Error = super::Error($errno);)*
            pub fn error_info(errno: isize) -> &'static str {
                match errno {
                    $($errno => ::core::concat!(stringify!($name), ", ", stringify!($desc)),)*
                    _ => unreachable!("{}", errno),
                }
            }
        };
    }

    #[rustfmt::skip]
    declare_errno!(
        UNSUPPORTED, -1024, "Do not support",
        BREAK,       -1023, "Thread should exit",
        
        EPERM,          -1,     "Operation not permitted.",
        ENOENT,         -2,     "No such file or directory.",
        ESRCH,          -3,     "No such process.",
        EINTR,          -4,     "Interrupted system call.",
        EIO,            -5,     "I/O error.",
        ENXIO,          -6,     "No such device or address.",
        ENOEXEC,        -8,     "Exec format error.",
        EBADF,          -9,     "Bad file number.",
        ECHILD,         -10,    "No child process",
        EAGAIN,         -11,    "Try again.",
        ENOMEM,         -12,    "Out of memory",
        EFAULT,         -14,    "Bad address.",
        EBUSY,          -16,    "Device or resource busy.",
        EEXIST,         -17,    "File exists.",
        ENOTDIR,        -20,    "Not a directory.",
        EISDIR,         -21,    "Is a directory.",
        EINVAL,         -22,    "Invalid argument.",
        EMFILE,         -24,    "Too many open files.",
        ENOTTY,         -25,    "Not a tty.",
        ESPIPE,         -29,    "Illegal seek.",
        ERANGE,         -34,    "Exceed range.",
        ENAMETOOLONG,   -78,    "Filename too long",
    );
}
