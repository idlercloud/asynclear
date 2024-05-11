use core::{arch, mem, ops::Deref};

use common::config::{LOW_ADDRESS_END, MAX_PATHNAME_LEN, PAGE_SIZE, PAGE_SIZE_BITS};
use defines::error::{errno, KResult};
use riscv::register::stvec::{self, TrapMode};
use riscv_guard::{AccessUserGuard, NoIrqGuard};
use scopeguard::defer;
use uninit::out_ref::Out;

pub struct UserCheck<T: ?Sized> {
    ptr: *mut T,
}

unsafe impl<T: ?Sized> Send for UserCheck<T> {}

// TODO: 检查用户指针 page fault 时可以采取措施挽救

impl<T> UserCheck<T> {
    pub fn new(ptr: *mut T) -> Self {
        Self { ptr }
    }

    pub fn new_slice(ptr: *mut [T]) -> UserCheck<[T]> {
        UserCheck { ptr }
    }

    pub fn is_null(&self) -> bool {
        self.ptr.is_null()
    }

    pub fn add(self, count: usize) -> Self {
        Self {
            ptr: (self.ptr as usize + mem::size_of::<T>() * count) as _,
        }
    }
}

impl<T> UserCheck<T> {
    pub fn check_ptr(&self) -> KResult<UserRead<T>> {
        if self.ptr as usize + mem::size_of::<T>() > LOW_ADDRESS_END {
            return Err(errno::EFAULT);
        }
        let _access_user_guard = AccessUserGuard::new();
        if check::check_const_impl(self.ptr, 1) {
            Ok(UserRead {
                ptr: self.ptr,
                _access_user_guard,
            })
        } else {
            Err(errno::EFAULT)
        }
    }

    pub unsafe fn check_ptr_mut(&self) -> KResult<UserWrite<T>> {
        if self.ptr as usize + mem::size_of::<T>() > LOW_ADDRESS_END {
            return Err(errno::EFAULT);
        }
        let _access_user_guard = AccessUserGuard::new();
        if check::check_mut_impl(self.ptr, 1) {
            Ok(UserWrite {
                ptr: self.ptr,
                _access_user_guard,
            })
        } else {
            Err(errno::EFAULT)
        }
    }
}

impl<T> UserCheck<[T]> {
    pub fn len(&self) -> usize {
        self.ptr.len()
    }

    pub fn check_slice(&self) -> KResult<UserRead<[T]>> {
        if self.ptr.cast::<T>() as usize + mem::size_of::<T>() * self.ptr.len() > LOW_ADDRESS_END {
            return Err(errno::EFAULT);
        }
        let _access_user_guard = AccessUserGuard::new();
        if check::check_const_impl(self.ptr.cast_const().as_ptr(), self.ptr.len()) {
            Ok(UserRead {
                ptr: self.ptr,
                _access_user_guard,
            })
        } else {
            Err(errno::EFAULT)
        }
    }

    pub unsafe fn check_slice_mut(&self) -> KResult<UserWrite<[T]>> {
        if self.ptr.cast::<T>() as usize + mem::size_of::<T>() * self.ptr.len() > LOW_ADDRESS_END {
            return Err(errno::EFAULT);
        }
        let _access_user_guard = AccessUserGuard::new();
        if check::check_mut_impl(self.ptr.as_mut_ptr(), self.ptr.len()) {
            Ok(UserWrite {
                ptr: self.ptr,
                _access_user_guard,
            })
        } else {
            Err(errno::EFAULT)
        }
    }
}

impl UserCheck<u8> {
    /// 非 utf8 会返回 EINVAL
    pub fn check_cstr(&self) -> KResult<UserRead<str>> {
        if self.ptr as usize >= LOW_ADDRESS_END {
            return Err(errno::EFAULT);
        }
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

                if va >= LOW_ADDRESS_END {
                    return Err(errno::EFAULT);
                }

                if end - self.ptr as usize > MAX_PATHNAME_LEN {
                    warn!("user cstr too long, from {:p}", self.ptr);
                    return Err(errno::ENAMETOOLONG);
                }
            }
        }

        let bytes = unsafe { core::slice::from_raw_parts(self.ptr, end - self.ptr as usize) };
        let ret = core::str::from_utf8(bytes).map_err(|_error| {
            warn!("Not utf8 in {:#x}..{:#x}", self.ptr as usize, end);
            errno::EINVAL
        })?;
        Ok(UserRead {
            ptr: ret as _,
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
    ptr: *const T,
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
        let ptr = unsafe { self.ptr.as_ptr().add(index) };
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
        unsafe { &*self.ptr }
    }
}

impl Deref for UserRead<str> {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.ptr }
    }
}

// impl UserRead<str> {
// pub fn is_empty(&self) -> bool {
// self.ptr.len() == 0
// }
// }

pub struct UserWrite<T: ?Sized> {
    ptr: *mut T,
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
        unsafe { Out::from_raw(self.ptr) }
    }
}

unsafe impl<T: ?Sized> Send for UserRead<T> {}
unsafe impl<T: ?Sized> Send for UserWrite<T> {}

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
    use common::config::PAGE_SIZE;
    use riscv::register::stvec::{self, TrapMode};
    use riscv_guard::NoIrqGuard;

    use super::{
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
