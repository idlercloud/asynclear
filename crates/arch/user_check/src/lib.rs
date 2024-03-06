#![no_std]
#![feature(naked_functions)]

#[macro_use]
extern crate kernel_tracer;

use core::{
    arch,
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
    ptr: *const T,
}

pub struct UserCheckMut<T> {
    ptr: *mut T,
}

unsafe impl<T> Send for UserCheck<T> {}
unsafe impl<T> Send for UserCheckMut<T> {}

// TODO: 检查用户指针 page fault 时可以采取措施挽救

impl<T> UserCheck<T> {
    pub fn new(ptr: *const T) -> Self {
        Self { ptr }
    }

    pub fn is_null(&self) -> bool {
        self.ptr.is_null()
    }

    pub fn check_ptr(&self) -> Result<UserConst<T>> {
        let _access_user_guard = AccessUserGuard::new();
        if check::check_const_impl(self.ptr, 1) {
            Ok(UserConst {
                ptr: self.ptr,
                _access_user_guard,
            })
        } else {
            Err(errno::EFAULT)
        }
    }

    pub fn check_slice(&self, len: usize) -> Result<UserConst<[T]>> {
        let _access_user_guard = AccessUserGuard::new();
        if check::check_const_impl(self.ptr, len) {
            Ok(UserConst {
                ptr: ptr::slice_from_raw_parts(self.ptr.cast(), len),
                _access_user_guard,
            })
        } else {
            Err(errno::EFAULT)
        }
    }
}

impl<T> UserCheckMut<T> {
    pub fn new(ptr: *mut T) -> Self {
        Self { ptr }
    }

    pub fn is_null(&self) -> bool {
        self.ptr.is_null()
    }

    pub fn check_ptr(&self) -> Result<UserConst<T>> {
        UserCheck::new(self.ptr as *const T).check_ptr()
    }

    pub fn check_ptr_mut(&self) -> Result<UserMut<T>> {
        let _access_user_guard = AccessUserGuard::new();
        if check::check_mut_impl(self.ptr, 1) {
            Ok(UserMut {
                ptr: self.ptr,
                _access_user_guard,
            })
        } else {
            Err(errno::EFAULT)
        }
    }

    pub fn check_slice(&self, len: usize) -> Result<UserConst<[T]>> {
        UserCheck::new(self.ptr as *const T).check_slice(len)
    }

    pub fn check_slice_mut(&self, len: usize) -> Result<UserMut<[T]>> {
        let _access_user_guard = AccessUserGuard::new();
        if check::check_mut_impl(self.ptr, len) {
            Ok(UserMut {
                ptr: ptr::slice_from_raw_parts_mut(self.ptr.cast(), len),
                _access_user_guard,
            })
        } else {
            Err(errno::EFAULT)
        }
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

        let mut va = self.ptr as usize;
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

                if end - self.ptr as usize > MAX_PATHNAME_LEN {
                    warn!("user cstr too long, from {:p}", self.ptr);
                    return Err(errno::ENAMETOOLONG);
                }
            }
        }

        let bytes =
            unsafe { core::slice::from_raw_parts(self.ptr.cast::<u8>(), end - self.ptr as usize) };
        let ret = core::str::from_utf8(bytes).map_err(|_error| {
            warn!("Not utf8 in {:#x}..{:#x}", self.ptr as usize, end);
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

mod check {
    use defines::config::PAGE_SIZE;
    use riscv::register::stvec::{self, TrapMode};
    use riscv_guard::NoIrqGuard;

    use crate::{
        set_kernel_trap_entry, trap_from_access_user, try_read_user_byte, try_write_user_byte,
    };

    fn check_impl<T>(user_addr_start: usize, len: usize, access_ok: fn(usize) -> bool) -> bool {
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

    pub fn check_const_impl<T>(user_ptr: *const T, len: usize) -> bool {
        let user_addr_start = user_ptr as usize;
        check_impl::<T>(user_addr_start, len, |addr| try_read_user_byte(addr) == 0)
    }

    pub fn check_mut_impl<T>(user_ptr: *mut T, len: usize) -> bool {
        let user_addr_start = user_ptr as usize;
        check_impl::<T>(user_addr_start, len, |addr| try_write_user_byte(addr) == 0)
    }
}
