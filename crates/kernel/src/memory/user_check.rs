use core::{
    arch, mem,
    num::NonZeroUsize,
    ops::{Deref, Range},
    ptr::NonNull,
};

use common::config::{LOW_ADDRESS_END, MAX_PATHNAME_LEN, PAGE_SIZE, PAGE_SIZE_BITS};
use defines::error::{errno, KResult};
use riscv::{
    interrupt::Exception,
    register::stvec::{self, TrapMode},
    ExceptionNumber,
};
use riscv_guard::{AccessUserGuard, NoIrqGuard};
use scopeguard::defer;

use crate::hart::local_hart;

// 内核有时也有读写文件的需求，比如 sendfile 的实现

pub enum ReadBuffer<'a> {
    Kernel(&'a mut [u8]),
    User(UserCheck<[u8]>),
}

impl ReadBuffer<'_> {
    pub fn len(&self) -> usize {
        match self {
            ReadBuffer::Kernel(buf) => buf.len(),
            ReadBuffer::User(buf) => buf.len(),
        }
    }
}

pub enum WriteBuffer<'a> {
    Kernel(&'a [u8]),
    User(UserCheck<[u8]>),
}

impl WriteBuffer<'_> {
    pub fn len(&self) -> usize {
        match self {
            WriteBuffer::Kernel(buf) => buf.len(),
            WriteBuffer::User(buf) => buf.len(),
        }
    }
}

impl<'a> WriteBuffer<'a> {
    pub fn slice(&self, range: Range<usize>) -> Option<WriteBuffer<'a>> {
        if range.end > self.len() || range.start > range.end {
            return None;
        }
        match self {
            WriteBuffer::Kernel(buf) => Some(WriteBuffer::Kernel(buf.get(range)?)),
            WriteBuffer::User(buf) => Some(WriteBuffer::User(buf.slice(range)?)),
        }
    }
}

pub struct UserCheck<T: ?Sized> {
    ptr: NonNull<T>,
}

unsafe impl<T: ?Sized> Send for UserCheck<T> {}

impl<T> UserCheck<T> {
    pub fn new(ptr: *mut T) -> Option<Self> {
        Some(Self {
            ptr: NonNull::new(ptr)?,
        })
    }

    pub fn new_slice(ptr: *mut T, len: usize) -> Option<UserCheck<[T]>> {
        Some(UserCheck {
            ptr: NonNull::slice_from_raw_parts(NonNull::new(ptr)?, len),
        })
    }

    pub fn add(self, count: usize) -> Option<Self> {
        let offset = mem::size_of::<T>() * count;
        Some(Self {
            ptr: self.ptr.with_addr(self.ptr.addr().checked_add(offset)?),
        })
    }

    pub fn check_ptr(&self) -> KResult<UserRead<T>> {
        if self.ptr.addr().get() + mem::size_of::<T>() > LOW_ADDRESS_END {
            return Err(errno::EFAULT);
        }
        let _access_user_guard = check_read_impl(self.ptr.as_ptr(), 1)?;
        Ok(UserRead {
            ptr: self.ptr,
            _access_user_guard,
        })
    }

    pub unsafe fn check_ptr_mut(&self) -> KResult<UserWrite<T>> {
        let _access_user_guard = check_write_impl(self.ptr.as_ptr(), 1)?;
        Ok(UserWrite {
            ptr: self.ptr,
            _access_user_guard,
        })
    }
}

impl<T: ?Sized> UserCheck<T> {
    pub fn addr(&self) -> NonZeroUsize {
        self.ptr.addr()
    }
}

impl<T> UserCheck<[T]> {
    pub fn len(&self) -> usize {
        self.ptr.len()
    }

    pub fn check_slice(&self) -> KResult<UserRead<[T]>> {
        let _access_user_guard = check_read_impl(self.ptr.as_mut_ptr(), self.ptr.len())?;
        Ok(UserRead {
            ptr: self.ptr,
            _access_user_guard,
        })
    }

    pub unsafe fn check_slice_mut(&self) -> KResult<UserWrite<[T]>> {
        let _access_user_guard = check_write_impl(self.ptr.as_mut_ptr(), self.ptr.len())?;
        Ok(UserWrite {
            ptr: self.ptr,
            _access_user_guard,
        })
    }

    pub fn as_user_check(&self) -> UserCheck<T> {
        UserCheck {
            ptr: self.ptr.as_non_null_ptr(),
        }
    }

    pub fn slice(&self, range: Range<usize>) -> Option<UserCheck<[T]>> {
        if range.end > self.len() || range.start > range.end {
            return None;
        }
        let first = self.as_user_check().add(range.start)?;
        Some(UserCheck {
            ptr: NonNull::slice_from_raw_parts(first.ptr, range.end - range.start),
        })
    }
}

impl UserCheck<u8> {
    /// 非 utf8 会返回 EINVAL
    pub fn check_cstr(&self) -> KResult<UserRead<str>> {
        let start = self.ptr.addr().get();
        if self.ptr.addr().get() >= LOW_ADDRESS_END {
            return Err(errno::EFAULT);
        }

        let mut va = start;
        let mut end;

        let _access_user_guard = AccessUserGuard::new();
        {
            let _guard = NoIrqGuard::new();
            unsafe {
                stvec::write(trap_from_access_user as usize, TrapMode::Direct);
            }
            defer! {
                set_kernel_trap_entry();
            }
            loop {
                try_read_user_byte(va)?;
                end = Self::check_cstr_end(va);
                if end > va && end % PAGE_SIZE == 0 {
                    // 没找到 null terminator
                    va = end;
                } else {
                    break;
                }

                if va >= LOW_ADDRESS_END {
                    return Err(errno::EFAULT);
                }

                if end - start > MAX_PATHNAME_LEN {
                    warn!("user cstr too long, from {:p}", self.ptr);
                    return Err(errno::ENAMETOOLONG);
                }
            }
        }

        let bytes = unsafe { core::slice::from_raw_parts(self.ptr.as_ptr(), end - start) };
        let ret = core::str::from_utf8(bytes).map_err(|_error| {
            warn!("Not utf8 in {:#x}..{:#x}", start, end);
            errno::EINVAL
        })?;
        Ok(UserRead {
            ptr: NonNull::from(ret),
            _access_user_guard,
        })
    }

    fn check_cstr_end(start: usize) -> usize {
        let next_page = ((start >> PAGE_SIZE_BITS) << PAGE_SIZE_BITS) + PAGE_SIZE;
        let mut va = start;
        while va < next_page {
            if unsafe { *(va as *const u8) } == 0 {
                return va;
            }
            va += 1;
        }
        va
    }
}

/// `UserRead` 直接 deref 到对应的 T。
///
/// 其来源是用户的指针，没有任何方式约束，只能假设它指向的内容是合适地初始化的
pub struct UserRead<T: ?Sized> {
    ptr: NonNull<T>,
    _access_user_guard: AccessUserGuard,
}

impl<T> UserRead<T> {
    pub fn read(self) -> T {
        if self.ptr.is_aligned() {
            unsafe { self.ptr.read() }
        } else {
            unsafe { self.ptr.read_unaligned() }
        }
    }
}

impl Deref for UserRead<[u8]> {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        unsafe { self.ptr.as_ref() }
    }
}

impl Deref for UserRead<str> {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        unsafe { self.ptr.as_ref() }
    }
}

pub struct UserWrite<T: ?Sized> {
    ptr: NonNull<T>,
    _access_user_guard: AccessUserGuard,
}

impl<T> UserWrite<T> {
    pub fn read(&self) -> T {
        if self.ptr.is_aligned() {
            unsafe { self.ptr.read() }
        } else {
            unsafe { self.ptr.read_unaligned() }
        }
    }

    pub fn write(self, val: T) {
        if self.ptr.is_aligned() {
            unsafe { self.ptr.write(val) }
        } else {
            unsafe { self.ptr.write_unaligned(val) }
        }
    }
}

impl<T> UserWrite<[T]> {
    pub fn iter_mut(&mut self) -> impl Iterator<Item = UserWrite<T>> + '_ {
        core::iter::from_coroutine(
            #[coroutine]
            || {
                for i in 0..self.ptr.len() {
                    let ptr = unsafe { self.ptr.as_non_null_ptr().add(i) };
                    yield UserWrite {
                        ptr,
                        _access_user_guard: AccessUserGuard::new(),
                    }
                }
            },
        )
    }
}

impl UserWrite<[u8]> {
    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        unsafe { self.ptr.as_mut() }
    }
}

impl<T: ?Sized> !Send for UserRead<T> {}
impl<T: ?Sized> !Send for UserWrite<T> {}

// unsafe impl<T: ?Sized> Send for UserRead<T> {}
// unsafe impl<T: ?Sized> Send for UserWrite<T> {}

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

#[repr(C)]
struct TryOpRet {
    scause: usize,
    is_err: bool,
}

#[naked]
extern "C" fn try_read_user_byte_impl(addr: usize) -> TryOpRet {
    unsafe {
        arch::naked_asm!("mv a1, zero", "lb a0, 0(a0)", "ret");
    }
}

/// NOTE: 这里其实有一个隐式的假设：不存在只写页，也就是只要可写就可读
///
/// 一些资料表示没有支持只写页的处理器：
/// - <https://devblogs.microsoft.com/oldnewthing/20230306-00/?p=107902>
/// - <https://stackoverflow.com/questions/49421125/what-is-the-use-of-a-page-table-entry-being-write-only>
#[naked]
extern "C" fn try_write_user_byte_impl(addr: usize) -> TryOpRet {
    unsafe {
        arch::naked_asm!("mv a1, zero", "lb a2, 0(a0)", "sb a2, 0(a0)", "ret",);
    }
}

#[naked]
extern "C" fn trap_from_access_user() {
    unsafe {
        arch::naked_asm!(".align 2", "csrw sepc, ra", "li a1, 1", "csrr a0, scause", "sret",);
    }
}

pub fn set_kernel_trap_entry() {
    extern "C" {
        fn __trap_from_kernel();
    }
    unsafe {
        stvec::write(__trap_from_kernel as usize, TrapMode::Direct);
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
    let ok = local_hart().curr_process().lock_inner_with(|inner| {
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

fn check_read_impl<T>(user_ptr: *const T, len: usize) -> KResult<AccessUserGuard> {
    let user_addr_start = user_ptr as usize;
    let user_addr_end = user_addr_start + len * core::mem::size_of::<T>();
    if user_addr_end > LOW_ADDRESS_END {
        return Err(errno::EFAULT);
    }
    let _guard = NoIrqGuard::new();
    unsafe {
        stvec::write(trap_from_access_user as usize, TrapMode::Direct);
    }
    let access_user_guard = AccessUserGuard::new();
    let mut va = user_addr_start;
    while va < user_addr_end {
        try_read_user_byte(va)?;
        va += PAGE_SIZE;
    }
    set_kernel_trap_entry();
    Ok(access_user_guard)
}

fn check_write_impl<T>(user_ptr: *mut T, len: usize) -> KResult<AccessUserGuard> {
    let user_addr_start = user_ptr as usize;
    let user_addr_end = user_addr_start + len * core::mem::size_of::<T>();
    if user_addr_end > LOW_ADDRESS_END {
        return Err(errno::EFAULT);
    }
    let _guard = NoIrqGuard::new();
    unsafe {
        stvec::write(trap_from_access_user as usize, TrapMode::Direct);
    }
    let access_user_guard = AccessUserGuard::new();
    let mut va = user_addr_start;
    while va < user_addr_end {
        try_write_user_byte(va)?;
        va += PAGE_SIZE;
    }
    set_kernel_trap_entry();
    Ok(access_user_guard)
}
