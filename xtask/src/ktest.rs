use std::{
    process::{ExitCode, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::Duration,
};

use clap::Parser;
use defines::ktest::{KTEST_FAILED, KTEST_INCOMPLETE, KTEST_INIT_PATH, KTEST_OK, KTEST_RUNNER_ERROR};

use crate::{build::BuildArgs, qemu::QemuArgs, tool};

/// 有测试没通过
const EXIT_TESTS_FAILED: u8 = 1;
/// 没拿到可信结果（超时、缺失、运行器错误、QEMU 异常退出等）
const EXIT_HARNESS_ERROR: u8 = 2;
/// 内核 panic / guest 请求重启 / SBI 报失败时的 QEMU 退出码
const QEMU_EXIT_SBI_FAILURE: i32 = 0xffff;

/// 运行内核集成测试
#[derive(Parser)]
pub struct KtestArgs {
    #[clap(flatten)]
    build: BuildArgs,
    /// Hart 数量（SMP 代表 Symmetrical Multiple Processor）.
    #[clap(long, default_value_t = 2)]
    smp: u8,
    #[clap(long)]
    skip_build: bool,
    /// 如果开启，QEMU 会阻塞并等待 GDB 连接，此时超时参数不会生效
    #[clap(long)]
    debug: bool,
    /// 整轮测试的超时时间（秒），超时后杀掉 QEMU 并以失败结束
    #[clap(long, default_value_t = 300)]
    timeout: u64,
}

impl KtestArgs {
    pub fn run_test(self) -> ExitCode {
        if !self.skip_build {
            self.build.build();
            tool::prepare_os();
        }

        // 测试运行由特殊的 testrunner 作 PID 1，它运行所有测试，最终写 `/dev/ktest` 关闭内核
        let bootargs = format!("init={KTEST_INIT_PATH} ktest");
        println!("Running qemu with args \"{bootargs}\"...");

        let spawned = QemuArgs::base_qemu()
            .args(["-smp", &self.smp.to_string()])
            .args(["-append", bootargs.as_str()])
            .optional_args(self.debug.then_some(["-s", "-S"]))
            .stdin(Stdio::null())
            .spawn();
        let child = Arc::new(Mutex::new(spawned));
        // 提前返回或 panic 时也不能把仍在运行的 QEMU 留在后台占用文件镜像
        let _guard = scopeguard::guard(Arc::clone(&child), |child| {
            let mut child = child.lock().unwrap();
            if child.try_wait().is_ok_and(|status| status.is_none()) {
                let _ = child.kill();
                let _ = child.wait();
            }
        });
        let timed_out = Arc::new(AtomicBool::new(false));

        if self.debug {
            println!("debug 模式：不启用超时，等待 gdb 连接");
        } else {
            // 挂死的测试只能由宿主机兜底，超时 kill，QEMU 不会自行退出
            let child = Arc::clone(&child);
            let timed_out = Arc::clone(&timed_out);
            let timeout = Duration::from_secs(self.timeout);
            thread::spawn(move || {
                thread::sleep(timeout);
                let mut child = child.lock().unwrap();
                if child.try_wait().is_ok_and(|status| status.is_none()) {
                    timed_out.store(true, Ordering::SeqCst);
                    let _ = child.kill();
                }
            });
        }

        let wait_result = {
            let mut child = child.lock().unwrap();
            child.wait()
        };
        let status = match wait_result {
            Ok(status) => status,
            Err(e) => {
                return harness_error(format!("failed to wait for QEMU: {e}"));
            }
        };

        if timed_out.load(Ordering::SeqCst) {
            return harness_error(format!("timed out after {}s", self.timeout));
        }

        // 判定只看 QEMU 的退出码：guest 的结果码经 `/dev/ktest` → sifive_test 变成它
        match status.code() {
            Some(code) if code == i32::from(KTEST_OK) => ExitCode::SUCCESS,
            Some(code) if code == i32::from(KTEST_FAILED) => {
                // TODO: 现在只知道"有测试失败"，不知道是哪些，得自己往上翻串口日志。
                // 计划改成结构化上报：由 testrunner 把逐测试结论写到专门的地方，宿主机读出后列出失败的测试
                println!("ktest: some tests failed");
                ExitCode::from(EXIT_TESTS_FAILED)
            }
            Some(code) if code == i32::from(KTEST_INCOMPLETE) => harness_error("some tests did not run"),
            Some(code) if code == i32::from(KTEST_RUNNER_ERROR) => harness_error("the test runner reported an error"),
            Some(code) if code == QEMU_EXIT_SBI_FAILURE => harness_error(
                "the kernel panicked, requested a reboot, or the SBI reported a failure (QEMU exit code 0xffff)",
            ),
            Some(code) => harness_error(format!("no result (QEMU exit code {code})")),
            None => harness_error(format!("QEMU was terminated: {status}")),
        }
    }
}

/// 打印没拿到可信结果的原因
fn harness_error(message: impl std::fmt::Display) -> ExitCode {
    println!("ERROR: {message}");
    ExitCode::from(EXIT_HARNESS_ERROR)
}
