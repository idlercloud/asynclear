use core::{arch, mem, num::NonZeroUsize, ops::Deref, ptr::NonNull};

use common::config::{LOW_ADDRESS_END, MAX_PATHNAME_LEN, PAGE_SIZE, PAGE_SIZE_BITS};
use defines::error::{errno, KResult};
use riscv::register::{
    scause::Exception,
    stvec::{self, TrapMode},
};
use riscv_guard::{AccessUserGuard, NoIrqGuard};
use scopeguard::defer;
use uninit::out_ref::Out;

use crate::hart::local_hart;

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

impl<T> UserRead<[T]> {
    pub fn len(&self) -> usize {
        self.ptr.len()
    }

    pub fn read_at(&self, index: usize) -> Option<T> {
        if index >= self.len() {
            return None;
        }
        let ptr = unsafe { self.ptr.as_non_null_ptr().add(index) };
        Some(if ptr.is_aligned() {
            unsafe { ptr.read() }
        } else {
            unsafe { ptr.read_unaligned() }
        })
    }
}

impl<T> Deref for UserRead<[T]> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        const {
            assert!(mem::align_of::<T>() == 1);
        }
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
    pub fn write(self, val: T) {
        if self.ptr.is_aligned() {
            unsafe { self.ptr.write(val) }
        } else {
            unsafe { self.ptr.write_unaligned(val) }
        }
    }
}

impl<T> UserWrite<[T]> {
    pub fn out(&self) -> Out<'_, [T]> {
        const {
            assert!(mem::align_of::<T>() == 1);
        }
        unsafe { Out::from_raw(self.ptr.as_ptr()) }
    }
}

unsafe impl<T: ?Sized> Send for UserRead<T> {}
unsafe impl<T: ?Sized> Send for UserWrite<T> {}

fn try_read_user_byte(addr: usize) -> KResult<()> {
    let ret = try_read_user_byte_impl(addr);
    if ret.is_err {
        // 因为关中断，发生的必然是 `Exception`
        debug_assert!(ret.scause & (1 << (usize::BITS as usize - 1)) == 0);
        let e = Exception::from(ret.scause & !(1 << (usize::BITS as usize - 1)));
        handle_memory_exception(addr, e)?;
    }
    Ok(())
}

fn try_write_user_byte(addr: usize) -> KResult<()> {
    let ret = try_write_user_byte_impl(addr);
    if ret.is_err {
        // 因为关中断，发生的必然是 `Exception`
        debug_assert!(ret.scause & (1 << (usize::BITS as usize - 1)) == 0);
        let e = Exception::from(ret.scause & !(1 << (usize::BITS as usize - 1)));
        handle_memory_exception(addr, e)?;
    }
    Ok(())
}

#[repr(C)]
struct TryOpRet {
    is_err: bool,
    scause: usize,
}

#[naked]
extern "C" fn try_read_user_byte_impl(addr: usize) -> TryOpRet {
    unsafe {
        arch::asm!(
            "mv a1, a0",
            "mv a0, zero",
            "lb a1, 0(a1)",
            "ret",
            options(noreturn)
        );
    }
}

#[naked]
extern "C" fn try_write_user_byte_impl(addr: usize) -> TryOpRet {
    unsafe {
        arch::asm!(
            "mv a1, a0",
            "mv a0, zero",
            "sb a1, 0(a1)",
            "ret",
            options(noreturn)
        );
    }
}

#[naked]
extern "C" fn trap_from_access_user(addr: usize) {
    unsafe {
        arch::asm!(
            ".align 2",
            "csrw sepc, ra",
            "li a0, 1",
            "csrr a1, scause",
            "sret",
            options(noreturn)
        );
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
        Exception::StoreFault
            | Exception::StorePageFault
            | Exception::InstructionPageFault
            | Exception::LoadPageFault
    ) {
        warn!("Unexpected exception {e:?} when checking user ptr {addr:#x}");
        return Err(errno::EFAULT);
    };
    let ok = local_hart().curr_process().lock_inner_with(|inner| {
        inner
            .memory_space
            .handle_memory_exception(addr, e == Exception::StoreFault)
    });
    if !ok { Err(errno::EFAULT) } else { Ok(()) }
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
