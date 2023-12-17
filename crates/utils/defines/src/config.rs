use crate::constant::MiB;

pub const PTR_SIZE: usize = core::mem::size_of::<usize>();

/// 物理内存的末端
pub const MEMORY_END: usize = 0x8800_0000;
/// 物理内存的估算大小，只大不小
pub const MEMORY_SIZE: usize = 0x800_0000;

/// 内核地址空间中，虚拟地址相对于物理地址的偏移量
pub const PA_TO_VA: usize = 0xFFFF_FFFF_0000_0000;

/// 内核堆大小
pub const KERNEL_HEAP_SIZE: usize = 32 * MiB;

/// 一个页大小的 bit 数
pub const PAGE_SIZE_BITS: usize = 12;
/// 页大小
pub const PAGE_SIZE: usize = 1 << PAGE_SIZE_BITS;

/// 每个页中 PTE 的数量
pub const PTE_PER_PAGE: usize = PAGE_SIZE / PTR_SIZE;

/// 地址空间的最后一个字节
pub const ADDR_END: usize = usize::MAX;

/// 用户栈的大小
pub const USER_STACK_SIZE: usize = 8 * MiB;

/// mmap 开始寻找可映射段的起点，即低地址的 128GiB 处
pub const MMAP_START: usize = 0x20_0000_0000;
/// 低地址的末端，即 256GiB 处
pub const LOW_ADDRESS_END: usize = 0x40_0000_0000;

/// 时钟频率。似乎由 qemu 中的 `RISCV_ACLINT_DEFAULT_TIMEBASE_FREQ` 宏定义
///
/// TODO: 后续可能需要根据实际情况修改
pub const CLOCK_FREQ: usize = 10_000_000;

/// 每秒的 Tick 数。即理想状况下每秒触发定时器中断的次数
pub const TICKS_PER_SEC: usize = 20;

/// I/O 映射的起始地址和长度
pub const MMIO: &[(usize, usize)] = &[
    (QEMU_UART_ADDR, 0x1000),   // UART
    (0x1000_1000, 0x1000),      // VIRTIO
    (0x0200_0000, 0x10000),     // CLINT
    (QEMU_PLIC_ADDR, 0x400000), // PLIC
];

pub const QEMU_UART_ADDR: usize = 0x1000_0000;
pub const QEMU_PLIC_ADDR: usize = 0xc00_0000;

/// 信号机制所需的 bitset 大小
pub const SIGSET_SIZE: usize = 64;
pub const SIGSET_SIZE_BYTES: usize = SIGSET_SIZE / 8;

/// 内核线程的数量（核心数）
pub const HART_NUM: usize = 8;
/// Hart 启动时的地址
pub const HART_START_ADDR: usize = 0x8020_0000;

/// 内核并发任务上限
pub const TASK_LIMIT: usize = 128;
