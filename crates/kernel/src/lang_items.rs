use core::ffi::c_void;
use core::panic::PanicInfo;

#[panic_handler]
fn panic(info: &PanicInfo<'_>) -> ! {
    println!("{}", info);
    #[cfg(feature = "unwinding")]
    {
        use unwinding::abi::*;
        struct CallbackData {
            counter: usize,
        }
        extern "C" fn callback(
            unwind_ctx: &UnwindContext<'_>,
            arg: *mut c_void,
        ) -> UnwindReasonCode {
            let data = unsafe { &mut *(arg as *mut CallbackData) };
            data.counter += 1;
            println!(
                "{:4}:{:#19x} - <unknown>",
                data.counter,
                _Unwind_GetIP(unwind_ctx)
            );
            UnwindReasonCode::NO_REASON
        }
        let mut data = CallbackData { counter: 0 };
        _Unwind_Backtrace(callback, &mut data as *mut _ as _);
    }
    sbi_rt::system_reset(sbi_rt::Shutdown, sbi_rt::SystemFailure);
    unreachable!();
}
