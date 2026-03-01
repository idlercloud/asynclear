# CLAUDE.md

## 工作约定
- Python 相关操作统一使用 `uv`：`uv sync`、`uv run ...`

## 项目结构
- `crates/kernel`：内核主体（入口、调度、内存、文件系统等）
- `crates/arch/*`：架构相关实现（RISC-V 等）
- `crates/utils/*`：通用组件（锁、日志、分配器、公共定义）
- `user`：用户态程序（initproc、shell、测试 app）
- `xtask`：构建/运行/测试 CLI（cargo 别名入口）
- `res/rootfs`：rootfs 与测试程序资源
- `docs`：设计文档

## 常用指令
- `just dev`：在开发模式下运行内核
- `just lint`：代码检查
- `just dbg`：调试启动内核并等待调试器连接
- `just test`：运行内核测试
