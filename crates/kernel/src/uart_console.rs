use klocks::SpinNoIrqMutex;

use core::fmt::{Arguments, Result, Write};

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

/// 输出到 stdout
#[inline]
fn stdout_puts(fmt: Arguments<'_>) {
    STDOUT.lock().write_fmt(fmt).unwrap();
}

#[inline]
pub fn print(args: Arguments<'_>) {
    stdout_puts(args);
}

/// 打印格式字符串，无换行
pub macro print {
    ($($arg:tt)*) => {
        $crate::uart_console::print(core::format_args!($($arg)*));
    }
}

/// 打印格式字符串，有换行
pub macro println {
    () => ($crate::print!("\n")),
    ($($arg:tt)*) => {
        $crate::uart_console::print(core::format_args_nl!($($arg)*));
    }
}
