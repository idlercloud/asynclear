//! 参考 Rust For Linux 的 <https://rust.docs.kernel.org/kernel/uaccess/index.html>

use core::{
    arch,
    mem::{self, MaybeUninit},
    ptr::NonNull,
};

use bytemuck::Pod;
use common::config::{LOW_ADDRESS_END, MAX_PATH_LEN, PAGE_SIZE, PAGE_SIZE_BITS};
use defines::error::{errno, KResult};
use ecow::EcoString;
use riscv::{
    interrupt::Exception,
    register::stvec::{self, Stvec, TrapMode},
    ExceptionNumber,
};
use riscv_guard::{AccessUserGuard, NoIrqGuard};
use scopeguard::defer;

use crate::{hart, trap};

pub struct UserPtr<T: ?Sized> {
    ptr: NonNull<T>,
}

impl<T: ?Sized> Clone for UserPtr<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: ?Sized> Copy for UserPtr<T> {}

unsafe impl<T: ?Sized> Send for UserPtr<T> {}

impl<T> UserPtr<T> {
    pub fn new(ptr: *mut T) -> KResult<Self> {
        let begin = ptr as usize;
        if begin >= LOW_ADDRESS_END || begin.saturating_add(mem::size_of::<T>()) > LOW_ADDRESS_END {
            return Err(errno::EFAULT);
        }
        NonNull::new(ptr).map(|ptr| Self { ptr }).ok_or(errno::EFAULT)
    }
}

impl<T> UserPtr<[T]> {
    pub fn new_slice(ptr: *mut T, len: usize) -> KResult<Self> {
        let begin = ptr as usize;
        let byte_len = mem::size_of::<T>().checked_mul(len).ok_or(errno::EFAULT)?;
        let end = begin.checked_add(byte_len).ok_or(errno::EFAULT)?;
        if (byte_len > 0 && begin >= LOW_ADDRESS_END) || end > LOW_ADDRESS_END {
            return Err(errno::EFAULT);
        }
        NonNull::new(ptr)
            .map(|ptr| Self {
                ptr: NonNull::slice_from_raw_parts(ptr, len),
            })
            .ok_or(errno::EFAULT)
    }

    pub fn len(&self) -> usize {
        self.ptr.len()
    }

    pub fn is_empty(&self) -> bool {
        self.ptr.len() == 0
    }
}

impl<T: Pod> UserPtr<T> {
    /// # Safety
    /// 需要用户自行确保并发性
    pub unsafe fn read(self) -> KResult<T> {
        let mut ret = MaybeUninit::<T>::uninit();
        let bytes = ret.as_bytes_mut();
        unsafe {
            copy_from_user(
                self.ptr.as_ptr() as *const u8,
                bytes.as_mut_ptr().cast::<u8>(),
                mem::size_of::<T>(),
            )?;
            Ok(ret.assume_init())
        }
    }
}

impl<T: Pod> UserPtr<T> {
    /// # Safety
    /// 需要用户自行确保并发性
    pub unsafe fn write(self, value: T) -> KResult<()> {
        let bytes = bytemuck::bytes_of(&value);
        unsafe {
            copy_to_user(bytes.as_ptr(), self.ptr.as_ptr().cast::<u8>(), bytes.len())?;
        }
        Ok(())
    }
}

impl UserPtr<[u8]> {
    /// # Safety
    /// 需要用户自行确保并发性
    pub unsafe fn read_bytes(self, dst: &mut [u8]) -> KResult<()> {
        if dst.len() > self.len() {
            return Err(errno::EINVAL);
        }
        unsafe {
            copy_from_user(self.ptr.as_mut_ptr(), dst.as_mut_ptr(), dst.len())?;
        }
        Ok(())
    }

    /// # Safety
    /// 需要用户自行确保并发性
    pub unsafe fn write_bytes(self, src: &[u8]) -> KResult<()> {
        if src.len() > self.len() {
            return Err(errno::EINVAL);
        }
        unsafe {
            copy_to_user(src.as_ptr(), self.ptr.as_mut_ptr(), src.len())?;
        }
        Ok(())
    }

    /// # Safety
    /// 需要用户自行确保并发性
    pub unsafe fn write_bytes_at(self, src: &[u8], offset: usize) -> KResult<()> {
        if let Some(end) = offset.checked_add(src.len())
            && end > self.len()
        {
            return Err(errno::EINVAL);
        }
        unsafe {
            copy_to_user(src.as_ptr(), self.ptr.as_mut_ptr().add(offset), src.len())?;
        }
        Ok(())
    }
}

impl UserPtr<u8> {
    /// 从用户空间读入一个 C 式的 `\0` 结尾字符串，包括末尾的 `\0` 一并写入 `buf`
    ///
    /// 返回字符串长度（不包括末尾 `\0`）。
    /// `buf` 容纳不下（包括末尾的 `\0`）时返回 [`errno::ENAMETOOLONG`]
    ///
    /// # Safety
    /// 需要用户自行确保并发性
    pub unsafe fn read_cstr(self, buf: &mut [u8]) -> KResult {
        unsafe { copy_cstr_from_user(self.ptr.as_ptr(), buf.as_mut_ptr(), buf.len()) }
    }

    /// 从用户空间读入一个路径
    ///
    /// 长度超过 [`MAX_PATH_LEN`] 时返回 [`errno::ENAMETOOLONG`]，非 utf8 则返回 [`errno::EINVAL`]
    ///
    /// # Safety
    /// 需要用户自行确保并发性
    pub unsafe fn read_path(self) -> KResult<EcoString> {
        let mut buf = [0; MAX_PATH_LEN];
        let len = unsafe { self.read_cstr(&mut buf) }.inspect_err(|&error| {
            if error == errno::ENAMETOOLONG {
                warn!("user path too long, from {:p}", self.ptr);
            }
        })?;
        let path = core::str::from_utf8(&buf[..len]).map_err(|_error| {
            warn!("Not utf8 path from {:p}", self.ptr);
            errno::EINVAL
        })?;
        Ok(EcoString::from(path))
    }
}

/// Safety:
/// - `src` 是用户缓冲区
/// - `dst` 是合法的内核缓冲区，长度不小于 `len`
unsafe fn copy_from_user(src: *const u8, dst: *mut u8, len: usize) -> KResult<()> {
    let src_start = src as usize;
    let src_end = src_start + len;

    let _access_user_guard = AccessUserGuard::new();
    {
        let _guard = NoIrqGuard::new();
        set_access_user_trap_entry();
        defer! {
            trap::set_kernel_trap_entry();
        }
        let mut va = src_start;
        // TODO: 这里的实现是有问题的，以后要换成基于异常表的读取
        while va < src_end {
            try_read_user_byte(va)?;
            va = ((va >> PAGE_SIZE_BITS) << PAGE_SIZE_BITS) + PAGE_SIZE;
        }
    }
    unsafe {
        dst.copy_from(src, len);
    }
    Ok(())
}

/// 读出的内容包括末尾的 `\0`，返回字符串长度（不包括末尾 `\0`）
///
/// `dst` 容纳不下（包括末尾的 `\0`）时返回 [`errno::ENAMETOOLONG`]
///
/// Safety:
/// - `src` 是用户缓冲区，代表一个 C 式的 `\0` 结尾字符串
/// - `dst` 是合法的内核缓冲区，长度不小于 `len`
unsafe fn copy_cstr_from_user(mut src: *const u8, mut dst: *mut u8, len: usize) -> KResult {
    if len == 0 {
        return Err(errno::ENAMETOOLONG);
    }
    let src_start = src as usize;

    let _guard = NoIrqGuard::new();
    set_access_user_trap_entry();
    defer! {
        trap::set_kernel_trap_entry();
    }
    let _access_user_guard = AccessUserGuard::new();
    let mut va = src_start;
    let mut page_end = ((src_start >> PAGE_SIZE_BITS) << PAGE_SIZE_BITS) + PAGE_SIZE;
    // TODO: 这里的实现是有问题的，以后要换成基于异常表的读取
    'ok: loop {
        if va >= LOW_ADDRESS_END {
            return Err(errno::EFAULT);
        }
        try_read_user_byte(va)?;
        while va < page_end {
            unsafe {
                let byte = src.read();
                dst.write(byte);
                if byte == 0 {
                    break 'ok;
                }
                va += 1;
                if va - src_start >= len {
                    return Err(errno::ENAMETOOLONG);
                }
                src = src.add(1);
                dst = dst.add(1);
            }
        }
        page_end += PAGE_SIZE;
    }
    Ok(va - src_start)
}

/// Safety:
/// - `src` 是合法的内核缓冲区，长度不小于 `len`
/// - `dst` 是用户缓冲区
unsafe fn copy_to_user(src: *const u8, dst: *mut u8, len: usize) -> KResult<()> {
    let dst_start = dst as usize;
    let dst_end = dst_start + len;

    let _access_user_guard = AccessUserGuard::new();
    {
        let _guard = NoIrqGuard::new();
        set_access_user_trap_entry();
        defer! {
            trap::set_kernel_trap_entry();
        }
        let mut va = dst_start;
        // TODO: 这里的实现是有问题的，以后要换成基于异常表的写入
        while va < dst_end {
            try_write_user_byte(va)?;
            va = ((va >> PAGE_SIZE_BITS) << PAGE_SIZE_BITS) + PAGE_SIZE;
        }
    }
    unsafe {
        dst.copy_from(src, len);
    }
    Ok(())
}

#[repr(C)]
struct TryOpRet {
    scause: usize,
    is_err: bool,
}

#[unsafe(naked)]
extern "C" fn try_read_user_byte_impl(addr: usize) -> TryOpRet {
    arch::naked_asm!("mv a1, zero", "lb a0, 0(a0)", "ret");
}

/// NOTE: 这里其实有一个隐式的假设：不存在只写页，也就是只要可写就可读
///
/// 一些资料表示没有支持只写页的处理器：
/// - <https://devblogs.microsoft.com/oldnewthing/20230306-00/?p=107902>
/// - <https://stackoverflow.com/questions/49421125/what-is-the-use-of-a-page-table-entry-being-write-only>
#[unsafe(naked)]
extern "C" fn try_write_user_byte_impl(addr: usize) -> TryOpRet {
    arch::naked_asm!("mv a1, zero", "lb a2, 0(a0)", "sb a2, 0(a0)", "ret",);
}

#[unsafe(naked)]
extern "C" fn trap_from_access_user() {
    arch::naked_asm!(".align 2", "csrw sepc, ra", "li a1, 1", "csrr a0, scause", "sret",);
}

fn set_access_user_trap_entry() {
    let mut stvec_value = Stvec::from_bits(trap_from_access_user as *const () as usize);
    stvec_value.set_trap_mode(TrapMode::Direct);
    unsafe {
        stvec::write(stvec_value);
    }
}

fn handle_memory_exception(addr: usize, e: Exception) -> KResult<()> {
    if !matches!(
        e,
        Exception::StoreFault | Exception::StorePageFault | Exception::InstructionPageFault | Exception::LoadPageFault
    ) {
        warn!("Unexpected exception {e:?} when checking user ptr {addr:#x}");
        return Err(errno::EFAULT);
    };
    let ok = hart::local_hart().curr_process().lock_inner_with(|inner| {
        inner
            .memory_space
            .handle_memory_exception(addr, e == Exception::StoreFault)
    });
    if !ok {
        Err(errno::EFAULT)
    } else {
        Ok(())
    }
}

fn try_read_user_byte(addr: usize) -> KResult<()> {
    let ret = try_read_user_byte_impl(addr);
    if ret.is_err {
        // 因为关中断，发生的必然是 `Exception`
        debug_assert!(ret.scause & (1 << (usize::BITS as usize - 1)) == 0);
        let e = Exception::from_number(ret.scause & !(1 << (usize::BITS as usize - 1))).map_err(|err| {
            error!("Unknown riscv error in try write: {err}");
            errno::EFAULT
        })?;
        handle_memory_exception(addr, e)?;
    }
    Ok(())
}

fn try_write_user_byte(addr: usize) -> KResult<()> {
    let ret = try_write_user_byte_impl(addr);
    if ret.is_err {
        // 因为关中断，发生的必然是 `Exception`
        debug_assert!(ret.scause & (1 << (usize::BITS as usize - 1)) == 0);
        let e = Exception::from_number(ret.scause & !(1 << (usize::BITS as usize - 1))).map_err(|err| {
            error!("Unknown riscv error in try write: {err}");
            errno::EFAULT
        })?;
        handle_memory_exception(addr, e)?;
    }
    Ok(())
}
