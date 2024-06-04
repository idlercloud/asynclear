# asynclear

基于 Rust 的异步操作系统内核。可运行在 riscv64imac 环境下。

## 结构说明

本项目采取 [xtask 模式](https://github.com/matklad/cargo-xtask)。可以认为是用 rust 写 make 或 bash 脚本。

这种模式只需要有 rust 环境就可以，无需其他依赖；而且更方便跨平台。

使用 `cargo xtask --help` 列出所有可用的任务，然后可以通过 `cargo <task>` 来运行（原理为在 `.cargo/config.toml` 设置 alias）。如 `cargo xbuild` 将构建内核和用户程序。具体参数可以查看每个任务的 `--help`。

### 项目模块

本项目采用 cargo workspace 维护。一个项目中包含多个 crate。crate 之间的依赖形成有向无环图

- crates/kernel
    - 内核的主模块，生成内核二进制文件
    - 包含 trap 处理、进程/线程管理、hart 管理、外设管理、文件系统、内存管理等
    - 内核的入口在 src/hart/entry.S 中
    - 加载内核栈和临时页表后，跳转到 src/hart/mod.rs::__hart_entry()
    - 主 hart 进行一些必要的初始化工作，并启动其他 hart
    - 最后进入 src/main::kernel_loop()，即内核主循环，不断运行用户任务
- crates/arch
    - 一些特定于架构的东西，比如 riscv 的 time 读取
- crates/utils
    - 一些通用组件
    - defines 包括一些内核和用户空间都会用到的定义
    - idallocator 是用于分配整数（pid、tid）的分配器实现
    - kernel_tracer 是内核的日志系统的基础
    - klocks 实现自旋锁、关中断自旋锁等原语
- deps（这个实际上不包含在 workspace 中，暂时）
    - 一些第三方库，但是需要做一些修改
    - 后期也可能基于它们扩展
- user
    - 一些用户应用，可以用做测试
    - 包括 initproc 和 shell
- xtask
    - xtask 方式管理内核的构建、运行等
    - 它是一个运行在开发环境（而非目标环境如 qemu）下的 cli

## 如何运行

1. 安装 qemu-system-riscv64，版本 7.0.x 或 7.1.x（7.2.0 有未知问题）
   - Windows: <https://qemu.weilnetz.de/w64/2022/qemu-w64-setup-20220831.exe>，这是 7.1.0 版的
   - Linux：<https://www.qemu.org/download/#linux>。找不到合适版本可能得自己从源码编译，[参考下文](#在-linux-上编译-qemu-system-riscv64)
2. 安装 rust 环境，请**务必**用[官方提供的安装方式](https://www.rust-lang.org/learn/get-started)
3. 运行 cargo env
4. 运行 cargo qemu

可以用 `cargo qemu --clog="INFO" --flog="DEBUG" --slog="DEBUG"` 来具体指定日志级别。

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

若使用 vscode + rust-analyzer，建议将以下设置加入 vscode 设置：

```json
"rust-analyzer.cargo.features": ["profiling"],
"rust-analyzer.check.overrideCommand": [
    "cargo",
    "check",
    "--message-format=json",
    /* for kernel and user apps */
    "--target",
    "riscv64imac-unknown-none-elf",
    "--package",
    "kernel",
    "--features",
    "profiling",
    "--package",
    "user",
    /* for xtask */
    // "--package",
    // "xtask",
],
```

由于一些限制，不能同时检查 kernel 和 xtask，若需开发 xtask，将上面的部分注释，再将 xtask 部分取消注释

可以通过调整添加 vscode 设置使 unsafe 块显示为血红色：

```json
"editor.semanticTokenColorCustomizations": {
    "enabled": true,
    "rules": {
        "*.unsafe:rust": "#ff4040"
    }
},
```

如非必要最好不要写 unsafe，如果一定要用，请控制使用范围，并且尽量不要从 `*const T`/`*mut T` 转换成 `&T`/`&mut T`，转换了也不要长期持有。

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

可以为 Todo Tree 添加以下配置：

```json
"todo-tree.regex.regex": "(todo!|(//|#|<!--|;|/\\*|^|^\\s*(-|\\d+.))\\s*($TAGS))",
"todo-tree.tree.labelFormat": "${tag}${after}",
"todo-tree.general.tags": ["TODO", "TAG", "NOTE", "FIXME", "[ ]"],
"todo-tree.highlights.enabled": true,
"todo-tree.highlights.customHighlight": {
    "todo!": {
        "icon": "list-unordered",
        "foreground": "#131416",
        "background": "#ffbf00",
        "rulerColour": "#ffbf00",
        "iconColour": "#ffbf00"
    },
    "TODO": {
        "icon": "list-unordered",
        "foreground": "#131416",
        "background": "#ffbf00",
        "rulerColour": "#ffbf00",
        "iconColour": "#ffbf00"
    },
    "DONE": {
        "icon": "issue-closed",
        "foreground": "#131416",
        "background": "#12cc12",
        "rulerColour": "#12cc12",
        "iconColour": "#12cc12"
    },
    "FIXME": {
        "icon": "bug",
        "foreground": "#dcdcdc",
        "background": "#e60000",
        "rulerColour": "#e60000",
        "iconColour": "#e60000",
        "rulerLane": "full"
    },
    "TAG": {
        "icon": "tag",
        "foreground": "#dcdcdc",
        "background": "#2e80f2",
        "rulerColour": "#2e80f2",
        "iconColour": "#2e80f2"
    },
    "NOTE": {
        "icon": "note",
        "foreground": "#dcdcdc",
        "background": "#8b00ff",
        "rulerColour": "#8b00ff",
        "iconColour": "#8b00ff"
    },
    "[ ]": {
        "icon": "list-unordered",
        "foreground": "#131416",
        "background": "#ffbf00",
        "rulerColour": "#ffbf00",
        "iconColour": "#ffbf00"
    }
},
```

### 调试方法

推荐使用命令行 gdb，更加靠谱。vscode 调试似乎会有一些奇怪的问题。

命令行调试过程如下：

1. 运行 `cargo qemu` 或 `cargo ktest` 时，多加一个参数 `--debug`
2. 另外开启一个终端并切换到 asynclear 目录，执行 `riscv64-unknown-elf-gdb -ex 'file target/riscv64imac-unknown-none-elf/kernel' -ex 'set arch riscv:rv64' -ex 'target remote localhost:1234'`

vscode 调试需要注意，如果 `riscv64-unknown-elf-gdb` 不在 `PATH` 中，需要在 `.vscode/launch.json` 中配置它的路径。过程如下：

1. 运行 `cargo qemu` 或 `cargo ktest` 时，多加一个参数 `--debug`
2. vscode 中按 F5，也就是启动调试
3. 可以通过图形界面控制运行，也可以在下方的调试控制台里通过 `-exec <gdb command>` 来手动输入 gdb 指令

由于 boot 时页表的变换，断点的打法是有技巧的：

1. 操作系统刚刚启动，此时起始点是在 linker 脚本里的地址，也即 0x80200000，所以先在 `*0x80200000` 处打个断点，然后 continue 过去。
2. 启动后会加载临时页表，加载后才可以直接给各种函数打断点，因此先步进大约 15 次
3. 此时高地址载入页表，已经可以用函数名或者 vscode 界面打断点了。
4. 另外，因为很快页表会再次变化，所以低地址的断点会无效，记得删掉第一个断点

其实 1、2 步可以合并，最初就直接 `break *0x8020002c`，然后直接 continue 过去，就可以进行第 3 步了。

## Todo

### 基础设施

- [x] Testing
- [ ] Benchmark
- [x] 栈回溯（基于 span）
- [x] Logging（日志事件、span 上下文）
- [x] Profiling（通过 <https://ui.perfetto.dev> 可视化）
- [ ] 探索 QEMU 的调试比如，比如暂停运行、中途连接调试器？

### 比较独立的工作

- [ ] buddy_system_allocator 增加调试信息，包括碎片率、分配耗时等等
- [ ] frame_allocator 可以试着用别的测试测试性能
- [ ] 某些堆分配可以用 Allocaotr API 试着优化
- [ ] trap 改为 vector 模式（会有优化吗？）
- [ ] per-cpu 的分配缓存
- [ ] virtio 块设备驱动采取中断方式处理
- [ ] 要定期检查下有没有无用依赖（人工，cargo-udeps，cargo-machete 等方法）
- [ ] 支持 GPU 驱动
- [ ] 支持用户的多线程之后要实现 TLB shootdown
- [ ] compact_str 或许可以改用 arcstr 或者 smol_str

### 具体任务

按优先级排列：

- [ ] 浮点数支持
- [ ] 添加文件系统的测试，包括且不限于：
    - 多次打开同一文件
    - append
    - lseek 超出文件末尾后再写
    - mmap 后读写与 read、write
    - munmap 后测试是否已经无效
- [ ] CoW、零页映射、mmap 私有映射
- [ ] mmap 和用户栈以及共享映射的区域划分要重新考虑
- [ ] 内核线程
- [ ] kernel 内容能否放入 huge page？

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
    - <https://gitlab.eduxiji.net/202310007101563/Alien>
    - [适用于 Cortex-M 的小型异步 RTOS](https://github.com/cbiffle/lilos)
