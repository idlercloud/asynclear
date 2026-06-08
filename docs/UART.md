# UART

UART 是 tty 体系的一部分，用于标准输入、标准输出。

一般而言，UART 处理的速度远低于 CPU 运行速度，因此在处理其输入输出时不应当忙等待。

## 输入

指的是从 UART 读入数据。

中断处理中，从 UART 硬件上读取数据，写入一个快速的环形缓冲区（记为 `UART_read_buffer`，Linux 中的 tty flip buffer，应当是固定大小如 4k）。

然后在通常的内核环境下，从 `UART_read_buffer` 中读数据，并经过 tty 处理（Linux 中的 line discipline），发送到 tty 读缓冲区（Linux 中的 line discipline buffer），并唤醒等待数据的用户线程。

### QA

Q: 如果有多个用户线程同时 read tty，如何处理？
A: tty 应当有一个读缓冲区锁，用户线程首先争抢锁，没拿到锁的自然沉睡等待锁唤醒，而拿到锁的则尝试从 tty 读缓冲区中读取数据，如果没有，则向 tty 注册 waker，并沉睡等待 tty 唤醒，注意这时仍然持有着 tty 读缓冲区锁，所以其他尝试 read 的进程仍然会被阻拦。

TODO: tty 读缓冲区该是固定大小吗？似乎应当是可配置的？如果数据量超过其大小怎么办？

## 输出

指的是向 UART 写入数据。

对于用户进程请求的 write，以及内核直接输出（如日志）是需要区分的。

### 用户 write

从用户缓冲区将数据拷贝到 tty 写缓冲区，此时 `write()` 系统调用就可以直接返回了。

TODO: 如果 tty 写缓冲区不够大呢？

TODO: 同理也得有个 tty 写缓冲区锁是吗？

然后内核会将 tty 写缓冲区中的数据写入到 UART 的 TX FIFO 中，并启动发送，然后 CPU 就可以去执行其他任务了。

UART 将 TX FIFO 的数据全部写完后，触发一个 TX Empty 中断，内核于是可以再将 tty 写缓冲区的数据写入到 UART TX FIFO 中。周而复始，直到 tty 写缓冲区再无数据。