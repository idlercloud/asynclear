//! QEMU `virt` 机器上的 `sifive_test` 设备（地址 0x100000）
//!
//! 向它的 offset 0 写一个 32 位值即可让整个模拟结束：低 16 位是 finisher 取值，
//! 高 16 位会成为 QEMU 进程的退出码。

#![no_std]

use core::arch::asm;

use common::config::{PA_TO_VA, QEMU_TEST_ADDR};

/// 表示"正常结束"的 finisher，退出码由写入值的高 16 位携带
const FINISHER_PASS: u32 = 0x5555;

/// 以 `code` 作为 QEMU 进程的退出码结束整个模拟
pub fn exit_with(code: u8) -> ! {
    const TEST_VA: usize = QEMU_TEST_ADDR + PA_TO_VA;
    #[allow(clippy::assertions_on_constants)]
    const {
        // 设备寄存器必须对齐访问
        assert!(TEST_VA.is_multiple_of(4));
    }

    let value = u32::from(code) << 16 | FINISHER_PASS;
    unsafe {
        (TEST_VA as *mut u32).write_volatile(value);
    }

    // 写入没有生效时（例如设备不存在）不至于跑飞
    loop {
        unsafe { asm!("wfi") }
    }
}
