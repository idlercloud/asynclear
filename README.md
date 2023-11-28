# asynclear

基于 Rust 的异步操作系统内核。

## Todo

按优先级排列：

- [ ] rCore-Tutorial I/O 设备管理（中断）
- [ ] 内核线程
- [ ] async-task 和 embassy 的原理
- [ ] 统一的睡眠锁和唤醒方式
- [ ] 用户指针检查通过内核异常来做
- [ ] 信号机制
- [ ] 虚拟文件系统和页缓存

## 参考资料

- [riscv sbi 规范](https://github.com/riscv-non-isa/riscv-sbi-doc)
    - binary-encoding 是调用约定
    - ext-debug-console 是一种更好的输入和输出控制台的方式
    - ext-legacy 是一些旧版的功能
