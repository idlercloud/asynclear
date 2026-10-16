//! ktest 的宿主机与 guest 之间的协议常量。

/// 整轮测试通过。
///
/// 需要非 0，因为 0 是 QEMU 的默认退出码。
pub const KTEST_OK: u8 = 0x7f;
/// 至少一个测试失败
pub const KTEST_FAILED: u8 = 0x01;
/// 至少一个期望的测试没有运行（测试二进制缺失或 exec 失败）
pub const KTEST_INCOMPLETE: u8 = 0x02;
/// 测试运行器自身出错（fork 失败、测试清单为空等）
pub const KTEST_RUNNER_ERROR: u8 = 0x03;

/// 单个测试进程的退出码：测试二进制不存在（exec 返回 ENOENT）
pub const TEST_EXIT_MISSING: i32 = 127;
/// 单个测试进程的退出码：测试二进制存在但无法执行
pub const TEST_EXIT_EXEC_ERROR: i32 = 126;

/// 测试运行器的路径
pub const KTEST_INIT_PATH: &str = "/testrunner";
