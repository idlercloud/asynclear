#![no_std]
#![feature(decl_macro)]

use core::fmt::{Arguments, Result, Write};

use klocks::SpinNoIrqMutex;

/// 标准输出
pub struct Stdout(());

impl Write for Stdout {
    fn write_str(&mut self, s: &str) -> Result {
        qemu_uart::UART0.print(s);
        Ok(())
    }
}

pub static STDOUT: SpinNoIrqMutex<Stdout> = SpinNoIrqMutex::new(Stdout(()));

#[inline]
pub fn print(args: Arguments<'_>) {
    STDOUT.lock().write_fmt(args).unwrap();
}

#[inline]
pub fn eprint(args: Arguments<'_>) {
    struct Stderr;
    impl Write for Stderr {
        fn write_str(&mut self, s: &str) -> Result {
            for byte in s.bytes() {
                sbi_rt::console_write_byte(byte);
            }
            Ok(())
        }
    }
    let _ = Stderr.write_fmt(args);
}

/// 打印格式字符串，无换行
pub macro print {
    ($($arg:tt)*) => {
        $crate::print(core::format_args!($($arg)*));
    }
}

/// 打印格式字符串，有换行
pub macro println {
    () => (print!("\n")),
    ($($arg:tt)*) => {
        $crate::print(core::format_args_nl!($($arg)*));
    }
}

/// 强制打印格式字符串，无换行
pub macro eprint {
    ($($arg:tt)*) => {
        $crate::eprint(core::format_args!($($arg)*));
    }
}

/// 强制打印格式字符串，有换行
pub macro eprintln {
    () => (eprint!("\n")),
    ($($arg:tt)*) => {
        $crate::eprint(core::format_args_nl!($($arg)*));
    }
}
