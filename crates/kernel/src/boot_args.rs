//! 内核启动参数，由 QEMU 的 `-append` 写在设备树 `/chosen/bootargs` 里
//!
//! 识别：
//! - `ktest`：打开测试模式，注册 `/dev/ktest`
//! - `init=<path>`：指定 PID 1 的程序（默认 /initproc）

use core::sync::atomic::{AtomicBool, Ordering};

use ecow::EcoString;
use fdt::Fdt;
use klocks::Once;

pub const DEFAULT_INIT_PATH: &str = "/initproc";

static KTEST_MODE: AtomicBool = AtomicBool::new(false);
static INIT_PATH: Once<EcoString> = Once::new();

/// 解析设备树里的启动参数
pub fn init(fdt: &Fdt<'_>) {
    let Some(bootargs) = fdt.chosen().bootargs() else {
        return;
    };
    info!("bootargs: {bootargs}");
    for arg in bootargs.split_whitespace() {
        if arg == "ktest" {
            KTEST_MODE.store(true, Ordering::SeqCst);
        } else if let Some(path) = arg.strip_prefix("init=") {
            if path.is_empty() {
                warn!("ignoring empty init path");
            } else {
                info!("init path from bootargs: {path}");
                INIT_PATH.call_once(|| EcoString::from(path));
            }
        } else {
            warn!("unknown boot arg: {arg}");
        }
    }

    if ktest_mode() && INIT_PATH.get().is_none() {
        warn!("ktest mode without `init=`: the default init will not report a result");
    }
}

/// PID 1 要执行的程序：`init=` 指定的（缺省 [`DEFAULT_INIT_PATH`]）
pub fn init_path() -> &'static str {
    INIT_PATH.get().map_or(DEFAULT_INIT_PATH, |path| path.as_str())
}

/// 是否处于 ktest 模式
pub fn ktest_mode() -> bool {
    KTEST_MODE.load(Ordering::SeqCst)
}
