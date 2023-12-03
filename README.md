# asynclear

基于 Rust 的异步操作系统内核。

## Todo

### 基础设施

- [ ] Testing
- [ ] Logging
- [ ] Profiling

### 比较独立的工作

- [ ] buddy_system_allocator 增加调试信息，包括碎片率、分配耗时等等
- [ ] frame_allocator 可以试着用别的测试测试性能

### 具体任务

按优先级排列：

- [ ] 修复 Frame allocate 的问题
- [ ] kernel_tracer
- [ ] rCore-Tutorial I/O 设备管理（中断）
- [ ] 统一的睡眠锁和唤醒方式
- [ ] 内核线程
- [ ] CoW、Lazy Page，顺便重构 memory 模块
- [ ] async-task 和 embassy 的原理
- [ ] 思考 Future 和 Send
- [ ] 用户指针检查通过内核异常来做
- [ ] 信号机制
- [ ] trap 改为 vector 模式
- [ ] kernel 内容能否放入 huge page？
- [ ] 虚拟文件系统和页缓存

## 参考资料

- [riscv sbi 规范](https://github.com/riscv-non-isa/riscv-sbi-doc)
    - binary-encoding 是调用约定
    - ext-debug-console 是一种更好的输入和输出控制台的方式
    - ext-legacy 是一些旧版的功能
