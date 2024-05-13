use common::config::PAGE_OFFSET_MASK;
use defines::{
    error::{errno, KResult},
    misc::{MmapFlags, MmapProt},
};

use crate::{
    hart::local_hart,
    memory::{MapPermission, VirtAddr},
};

/// 映射虚拟内存。返回实际映射的地址。
///
/// `addr` 若是 NULL，那么内核会自动选择一个按页对齐的地址进行映射，这也是比较可移植的方式。
///
/// `addr` 若有指定地址，那么内核会尝试在最近的页边界上映射，但如果已经被映射过了，就挑选一个新的地址。
/// 该新地址可能参考也可能不参考 `addr`。
///
/// 如果映射文件，那么会以该文件 (`fd`) `offset` 开始处的 `len` 个字节初始化映射内容。
///
/// `mmap()` 返回之后，就算 `fd` 指向的文件被立刻关闭，也不会影响映射的结果。
///
/// `prot` 要么是 `PROT_NONE`，要么是多个标志位的或。
///
/// `flags` 决定该映射是否对其他映射到同一区域的进程可见，以及更新是否会同步到底层文件上。
///
/// 参数：
/// - `addr` 映射的目标地址。
/// - `len` 映射的目标长度
/// - `prot` 描述该映射的内存保护方式，不能与文件打开模式冲突
/// - `flags` 描述映射的特征，详细参考 [`MmapFlags`]
/// - `fd` 被映射的文件描述符
/// - `offset` 映射的起始偏移，必须是 `PAGE_SIZE` 的整数倍
pub fn sys_mmap(addr: usize, len: usize, prot: u32, flags: u32, fd: i32, offset: usize) -> KResult {
    let prot = MmapProt::from_bits(prot).ok_or(errno::EINVAL)?;
    let Some(flags) = MmapFlags::from_bits(flags) else {
        // flags 出现了意料之外的标志位
        error!("unsupported flags: {flags:#b}");
        return Err(errno::UNSUPPORTED);
    };
    debug!("prot: {prot:?}, flags: {flags:?}");
    if flags.contains(MmapFlags::MAP_SHARED) {
        // 共享映射
        todo!("[mid] impl shared mapping");
    } else {
        // 私有映射
        // `MAP_SHARED`、`MAP_PRIVATE` 至少有其一
        if !flags.contains(MmapFlags::MAP_PRIVATE) {
            return Err(errno::EINVAL);
        }

        if flags.contains(MmapFlags::MAP_ANONYMOUS) {
            // 私有匿名映射
            if fd != -1 || offset != 0 {
                warn!("fd must be -1 and offset must be 0 for anonyous mapping");
                return Err(errno::EINVAL);
            }
            private_anonymous_map(prot, flags, addr, len)
        } else {
            todo!("[mid] impl private file mapping");
        }
    }
}

/// 私有匿名映射，没有底层文件。内容全部初始化为 0
///
/// 如果 addr 没有对齐到页边界或者 len 为 0
fn private_anonymous_map(prot: MmapProt, flags: MmapFlags, addr: usize, len: usize) -> KResult {
    debug!("private anonymous map, addr: {addr:#}, len: {len}");
    if addr & PAGE_OFFSET_MASK != 0 || len == 0 {
        return Err(errno::EINVAL);
    }
    let process = local_hart().curr_process();
    let va_start = VirtAddr(addr);
    process.lock_inner_with(|inner| {
        inner
            .memory_space
            .try_map(va_start..va_start + len, MapPermission::from(prot), flags)
    })
}

/// 将一块区域取消映射。
///
/// （未实现）有可能产生多个新的区域，比如 unmap 一个大区域的中间，左右两遍会变成两个单独的小区域
///
/// 在目前的实现中应该只会在参数不正确（`addr` 未对齐、`len` 为 0）时返回 `EINVAL` 一种错误
pub fn sys_munmap(addr: usize, len: usize) -> KResult {
    debug!("unmap {addr}..{}", addr + len);
    if addr & PAGE_OFFSET_MASK != 0 || len == 0 {
        return Err(errno::EINVAL);
    }
    let va_start = VirtAddr(addr);
    local_hart()
        .curr_process()
        .lock_inner_with(|inner| inner.memory_space.unmap(va_start..va_start + len));
    Ok(0)
}

/// 将 program break 设置为 `brk`。高于当前堆顶会分配空间，低于则会释放空间。
///
/// `brk` 为 0 时返回当前堆顶地址。设置成功时返回新的 brk，设置失败返回原来的 brk
///
/// 参数：
/// - `brk` 希望设置的 program break 值
pub fn sys_brk(brk: usize) -> KResult {
    let process = local_hart().curr_process();
    let mut inner = process.lock_inner();
    Ok(inner.set_user_brk(VirtAddr(brk)).0 as isize)
}
