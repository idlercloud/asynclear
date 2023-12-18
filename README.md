# asynclear

基于 Rust 的异步操作系统内核。可运行在 riscv64imac 环境下。

## 结构说明

本项目采取 [xtask 模式](https://github.com/matklad/cargo-xtask)。可以认为是用 rust 写 make 或 bash 脚本。

这种模式只需要有 rust 环境就可以，无需其他依赖；而且更方便跨平台。

使用 `cargo xtask --help` 列出所有可用的任务，然后可以通过 `cargo <task>` 来运行（原理为在 `.cargo/config.toml` 设置 alias）。如 `cargo xbuild` 将构建内核和用户程序。具体参数可以查看每个任务的 `--help`。

TODO: 添加项目模块说明

## 如何运行

1. 安装 qemu-system-riscv64，版本 7.0.x 或 7.1.x（7.2.0 有未知问题）
   - Windows: <https://qemu.weilnetz.de/w64/2022/qemu-w64-setup-20220831.exe>，这是 7.1.0 版的
   - Linux：<https://www.qemu.org/download/#linux>。找不到合适版本可能得自己从源码编译，[参考下文](#在-linux-上编译-qemu-system-riscv64)
2. 安装 rust 环境，请**务必**用[官方提供的安装方式](https://www.rust-lang.org/learn/get-started)
3. 运行 cargo env
4. 运行 cargo qemu

可以用 `cargo qemu --clog="DEBUG" --flog="TRACE" --slog="TRACE"` 来具体指定日志级别。

### 在 Linux 上编译 qemu-system-riscv64

```sh
wget https://download.qemu.org/qemu-7.0.0.tar.xz
tar xvJf qemu-7.0.0.tar.xz
cd qemu-7.0.0
./configure --target-list=riscv64-softmmu --prefix=/opt/qemu-7.0.0 --enable-virtfs
make -j12
sudo make install
# 然后将 qemu-system-riscv64 添加到 PATH 里
```

## 开发指南

### vscode 配置

若使用 vscode + rust-analyzer，建议将以下设置加入 vscode 设置：`"rust-analyzer.check.overrideCommand": ["cargo", "check", "--workspace", "--message-format=json", "--bins", "--target", "riscv64imac-unknown-none-elf", "--exclude", "xtask"],`

注意，在这种情况下，由于 xtask 目录被排除，vscode 中只会为 xtask 提供基本的补全、跳转，错误信息不会显示。

推荐扩展：

- rust-analyzer
- Even Better TOML
- crates
- Error Lens
- C/C++（调试用）
- RISC-V Support
- todo tree（用于查看项目中的 TODO/FIXME/NOTE）
- ANSI Colors（用于查看日志文件）
- AutoCorrect（中英文之间自动加空格隔开）

### 调试方法

如果 `riscv64-unknown-elf-gdb` 不在 `PATH` 中，需要在 `.vscode/launch.json` 中配置它的路径。

1. 运行 `cargo qemu` 时，多加一个参数 `--debug`
2. vscode 中按 F5，也就是启动调试
3. 可以通过图形界面控制运行，也可以在下方的调试控制台里通过 `-exec <gdb command>` 来手动输入 gdb 指令

由于 boot 时页表的变换，断点的打法是有技巧的：

1. 操作系统刚刚启动，此时起始点是在 linker 脚本里的地址，也即 0x80200000，所以先在 `*0x80200000` 处打个断点，然后 continue 过去。
2. 启动后会加载临时页表，加载后才可以直接给各种函数打断点，因此先步进大约 15 次
3. 此时高地址载入页表，已经可以用函数名或者 vscode 界面打断点了。
4. 另外，因为很快页表会再次变化，所以低地址的断点会无效，记得删掉第一个断点

其实 1、2 步可以合并，最初就直接 `break *0x8020001a`，然后直接 continue 过去，就可以进行第 3 步了。

## Todo

### 基础设施

- [ ] Testing
- [ ] 栈回溯（基于 span）
- [x] Logging（日志事件、span 上下文）
- [ ] Profiling（可视化）

### 比较独立的工作

- [ ] buddy_system_allocator 增加调试信息，包括碎片率、分配耗时等等
- [ ] frame_allocator 可以试着用别的测试测试性能
- [ ] 某些堆分配可以用 Allocaotr API 试着优化
- [ ] trap 改为 vector 模式（会有优化吗？）
- [ ] 要定期检查下有没有无用依赖（人工，cargo-udeps，cargo-machete 等方法）

### 具体任务

按优先级排列：

- [ ] rCore-Tutorial I/O 设备管理（中断）
- [ ] 内核线程
- [ ] Testing
- [ ] kernel_tracer（Profiling 可视化）
- [ ] 用户指针检查通过内核异常来做
- [ ] CoW、Lazy Page，顺便重构 memory 模块
- [ ] RCU
- [ ] 信号机制
- [ ] async-task 和 embassy 的原理
- [ ] kernel 内容能否放入 huge page？
- [ ] 虚拟文件系统和页缓存
- [ ] 思考 `Future` 和 `Send`

## 参考资料

- [riscv sbi 规范](https://github.com/riscv-non-isa/riscv-sbi-doc)
    - binary-encoding 是调用约定
    - ext-debug-console 是一种更好的输入和输出控制台的方式
    - ext-legacy 是一些旧版的功能
- 其他 OS 实现
    - <https://github.com/greenhandzpx/Titanix.git>
    - <https://gitlab.eduxiji.net/DarkAngelEX/oskernel2022-ftlos>
    - <https://gitlab.eduxiji.net/scPointer/maturin>
    - <https://gitlab.eduxiji.net/dh2zz/oskernel2022>
    - <https://gitee.com/LoanCold/ultraos_backup>
    - <https://github.com/xiaoyang-sde/rust-kernel-riscv>
    - <https://github.com/equation314/nimbos>
