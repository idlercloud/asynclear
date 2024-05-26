use core::fmt::{Arguments, Result, Write};

use klocks::SpinNoIrqMutex;

use crate::drivers::qemu_uart::UART0;

/// 标准输出
pub struct Stdout(());

impl Write for Stdout {
    fn write_str(&mut self, s: &str) -> Result {
        UART0.print(s);
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
                #[allow(deprecated)]
                sbi_rt::legacy::console_putchar(byte as usize);
            }
            Ok(())
        }
    }
    let _ = Stderr.write_fmt(args);
}

/// 打印格式字符串，无换行
pub macro print {
    ($($arg:tt)*) => {
        $crate::uart_console::print(core::format_args!($($arg)*));
    }
}

/// 打印格式字符串，有换行
pub macro println {
    () => (print!("\n")),
    ($($arg:tt)*) => {
        $crate::uart_console::print(core::format_args_nl!($($arg)*));
    }
}

/// 强制打印格式字符串，无换行
pub macro eprint {
    ($($arg:tt)*) => {
        $crate::uart_console::eprint(core::format_args!($($arg)*));
    }
}

/// 强制打印格式字符串，有换行
pub macro eprintln {
    () => (eprint!("\n")),
    ($($arg:tt)*) => {
        $crate::uart_console::eprint(core::format_args_nl!($($arg)*));
    }
}
