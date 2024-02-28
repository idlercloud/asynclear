#![no_std]
#![feature(naked_functions)]

#[macro_use]
extern crate kernel_tracer;

use core::{
    arch,
    marker::PhantomData,
    ops::{Deref, DerefMut},
    ptr,
};

use defines::{
    config::{MAX_PATHNAME_LEN, PAGE_SIZE, PAGE_SIZE_BITS},
    error::{errno, Result},
};
use riscv::register::stvec::{self, TrapMode};
use riscv_guard::{AccessUserGuard, NoIrqGuard};
use scopeguard::defer;

pub struct UserCheck<T> {
    addr: usize,
    _phantom: PhantomData<T>,
}

#[naked]
extern "C" fn try_read_user_byte(addr: usize) -> usize {
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
extern "C" fn try_write_user_byte(addr: usize) -> usize {
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

impl<T> UserCheck<T> {
    pub fn new(ptr: *mut T) -> Self {
        Self {
            addr: ptr as _,
            _phantom: PhantomData,
        }
    }

    // TODO: 检查用户指针 page fault 时可以采取措施挽救

    pub fn check_ptr(&self) -> Result<UserConst<T>> {
        let _access_user_guard = AccessUserGuard::new();
        if Self::check_impl(self.addr, 1, |addr| try_read_user_byte(addr) == 0) {
            Ok(UserConst {
                ptr: self.addr as _,
                _access_user_guard,
            })
        } else {
            Err(errno::EFAULT)
        }
    }

    pub fn check_ptr_mut(&self) -> Result<UserMut<T>> {
        let _access_user_guard = AccessUserGuard::new();
        if Self::check_impl(self.addr, 1, |addr| try_write_user_byte(addr) == 0) {
            Ok(UserMut {
                ptr: self.addr as _,
                _access_user_guard,
            })
        } else {
            Err(errno::EFAULT)
        }
    }

    pub fn check_slice(&self, len: usize) -> Result<UserConst<[T]>> {
        let _access_user_guard = AccessUserGuard::new();
        if Self::check_impl(self.addr, len, |addr| try_read_user_byte(addr) == 0) {
            Ok(UserConst {
                ptr: ptr::slice_from_raw_parts(self.addr as _, len),
                _access_user_guard,
            })
        } else {
            Err(errno::EFAULT)
        }
    }

    pub fn check_slice_mut(&self, len: usize) -> Result<UserMut<[T]>> {
        let _access_user_guard = AccessUserGuard::new();
        if Self::check_impl(self.addr, len, |addr| try_write_user_byte(addr) == 0) {
            Ok(UserMut {
                ptr: ptr::slice_from_raw_parts_mut(self.addr as _, len),
                _access_user_guard,
            })
        } else {
            Err(errno::EFAULT)
        }
    }

    fn check_impl(user_addr_start: usize, len: usize, access_ok: fn(usize) -> bool) -> bool {
        let _guard = NoIrqGuard::new();
        unsafe {
            stvec::write(trap_from_access_user as usize, TrapMode::Direct);
        }
        let Some(user_addr_end) = user_addr_start.checked_add(len * core::mem::size_of::<T>())
        else {
            return false;
        };
        let mut va = user_addr_start;
        while va < user_addr_end {
            if !access_ok(va) {
                return false;
            }
            va += PAGE_SIZE;
        }
        set_kernel_trap_entry();
        true
    }
}

impl UserCheck<u8> {
    /// 非 utf8 会返回 EINVAL
    pub fn check_cstr(&self) -> Result<UserConst<str>> {
        debug_span!("check_cstr");
        let _guard = NoIrqGuard::new();
        unsafe {
            stvec::write(trap_from_access_user as usize, TrapMode::Direct);
        }

        let mut va = self.addr;
        let mut end;

        let _access_user_guard = AccessUserGuard::new();
        {
            defer! {
                set_kernel_trap_entry();
            }
            loop {
                if try_read_user_byte(va) != 0 {
                    return Err(errno::EFAULT);
                }
                end = Self::check_cstr_end(va);
                if end > va && end % PAGE_SIZE == 0 {
                    // 没找到 null terminator
                    va = end;
                } else {
                    break;
                }

                if end - self.addr > MAX_PATHNAME_LEN {
                    warn!("user cstr too long, from {}", self.addr);
                    return Err(errno::ENAMETOOLONG);
                }
            }
        }

        let bytes = unsafe { core::slice::from_raw_parts(self.addr as *const u8, end - self.addr) };
        let ret = core::str::from_utf8(bytes).map_err(|_error| {
            warn!("Not utf8 in {:#x}..{:#x}", self.addr, end);
            errno::EINVAL
        })?;
        Ok(UserConst {
            ptr: ret as _,
            _access_user_guard,
        })
    }

    fn check_cstr_end(start: usize) -> usize {
        // TODO: 重构 memory 模块后，VitrAddr 相关的操作应该可以独立开来
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

// NOTE: UserConst 和 UserMut 的 Deref 和 DerefMut 不知道应不应该实现，其实很可能不是 safe 的

pub struct UserConst<T: ?Sized> {
    ptr: *const T,
    _access_user_guard: AccessUserGuard,
}

impl<T: ?Sized> Deref for UserConst<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.ptr }
    }
}

pub struct UserMut<T: ?Sized> {
    ptr: *mut T,
    _access_user_guard: AccessUserGuard,
}

impl<T: ?Sized> Deref for UserMut<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.ptr }
    }
}

impl<T: ?Sized> DerefMut for UserMut<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.ptr }
    }
}
