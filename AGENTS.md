# CLAUDE.md

## 项目简介

这是一个用 Rust 实现的类 linux 操作系统内核，目前可运行在 riscv64gc 环境下。

项目的构建、运行和测试等采用 xtask 模式，相关逻辑在 xtask 目录中。

## 工作约定

- Python 相关操作统一使用 `uv`：`uv sync`、`uv run ...`
- 不要添加显而易见的注释
- 不要使用写 git 的操作
- `EcoString` 是有小字符串优化、写时复制的第三方库字符串实现，合适时尽量用它

## 项目结构

- `crates/kernel`：内核二进制 crate，组装其它部分得到最终产物
- `crates/libkernel`：内核主要逻辑（内存、虚拟文件系统、进程管理等）
- `crates/fs/*`：文件系统实现
- `crates/arch/*`：架构相关实现（RISC-V 等）
- `crates/driver/*`：设备驱动实现
- `crates/utils/*`：通用组件（锁、日志、分配器、公共定义）
- `user`：用户态程序（initproc、shell、测试 app）
- `xtask`：构建/运行/测试 CLI（cargo 别名入口）
- `res/rootfs`：rootfs 与测试程序资源
- `docs`：设计文档

## 常用指令

- `just dev`：在开发模式下运行内核
- `just lint`：编译检查、代码风格检查。优先使用这个而非 `cargo check`
- `just dbg`：调试启动内核并等待调试器连接
- `just ktest`：运行内核测试
- `just cargo_test`：运行 cargo 测试，即不依赖于内核环境，可以直接在宿主机上运行的测试
