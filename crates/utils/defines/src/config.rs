pub const PTR_SIZE: usize = core::mem::size_of::<usize>();
const KB: usize = 1024;
const MB: usize = 1024 * KB;

/// 物理内存的末端
pub const MEMORY_END: usize = 0x8800_0000;
/// 物理内存的估算大小，只大不小
pub const MEMORY_SIZE: usize = 0x800_0000;

/// 内核地址空间中，虚拟地址相对于物理地址的偏移量
pub const PA_TO_VA: usize = 0xFFFF_FFFF_0000_0000;

/// 内核堆大小
pub const KERNEL_HEAP_SIZE: usize = 32 * MB;

/// 一个页大小的 bit 数
pub const PAGE_SIZE_BITS: usize = 12;
/// 页大小
pub const PAGE_SIZE: usize = 1 << PAGE_SIZE_BITS;

/// 每个页中 PTE 的数量
pub const PTE_PER_PAGE: usize = PAGE_SIZE / PTR_SIZE;

/// 地址空间的最后一个字节
pub const ADDR_END: usize = usize::MAX;

/// 用户栈的大小
pub const USER_STACK_SIZE: usize = 8 * MB;

/// mmap 开始寻找可映射段的起点，即低地址的 128GiB 处
pub const MMAP_START: usize = 0x20_0000_0000;
/// 低地址的末端，即 256GiB 处
pub const LOW_ADDRESS_END: usize = 0x40_0000_0000;

/// 时钟频率
///
/// TODO: 后续可能需要根据实际情况修改
pub const CLOCK_FREQ: usize = 12_500_000;

/// I/O 映射的起始地址和长度
pub const MMIO: &[(usize, usize)] = &[(0x1000_1000, 0x1000)];

/// 信号机制所需的 bitset 大小
pub const SIGSET_SIZE: usize = 64;
pub const SIGSET_SIZE_BYTES: usize = SIGSET_SIZE / 8;

/// 内核线程的数量（核心数）
pub const HART_NUM: usize = 8;
/// Hart 启动时的地址
pub const HART_START_ADDR: usize = 0x8020_0000;

pub const TASK_LIMIT: usize = 256;
