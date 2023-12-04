use compact_str::{CompactString, ToCompactString};
use defines::error::{errno, Result};
use memory::{PTEFlags, VirtAddr};

use crate::hart::curr_process;

pub struct UserConst<T: ?Sized> {
    ptr: *const T,
}

impl<T: ?Sized> UserConst<T> {
    pub fn as_raw(&self) -> *const T {
        self.ptr
    }
}

impl UserConst<str> {
    pub fn from_utf8(v: UserConst<[u8]>) -> Self {
        let ptr = core::str::from_utf8(unsafe { &*v.ptr }).unwrap() as *const str;
        Self { ptr }
    }

    pub fn len(&self) -> usize {
        unsafe { (*self.ptr).len() }
    }
}

unsafe impl<T: ?Sized> Send for UserConst<T> {}

impl From<UserConst<str>> for CompactString {
    fn from(value: UserConst<str>) -> Self {
        unsafe { (&*value.ptr).to_compact_string() }
    }
}

pub struct UserMut<T: ?Sized> {
    ptr: *mut T,
}

impl<T: ?Sized> UserMut<T> {
    pub fn raw(&self) -> *mut T {
        self.ptr
    }
}

unsafe impl<T: ?Sized> Send for UserMut<T> {}

// /// 检查一个用户指针的可读性以及是否有 U 标记
// ///
// /// TODO: 目前只检查了一页的有效性，如果结构体跨多页，则可能有问题
// #[track_caller]
// pub fn check_ptr<T>(ptr: *const T) -> Result<UserConst<T>> {
//     curr_process().lock_inner(|inner| {
//         let va = VirtAddr::from(ptr);
//         if let Some(pte) = inner.memory_set.page_table().find_pte(va.vpn()) {
//             if pte.flags().contains(PTEFlags::R | PTEFlags::U) {
//                 return Ok(UserConst { ptr });
//             }
//         }
//         Err(errno::EFAULT)
//     })
// }

/// 检查一个指向连续切片的指针（如字符串）的可读性以及是否有 U 标志
/// 检查一个用户指针的读写性以及是否有 U 标记
///
/// TODO: 目前只检查了一页的有效性，如果结构体跨多页，则可能有问题
#[track_caller]
pub fn check_ptr_mut<T>(ptr: *mut T) -> Result<UserMut<T>> {
    curr_process().lock_inner(|inner| {
        let va = VirtAddr::from(ptr);
        if let Some(pte) = inner.memory_set.page_table().find_pte(va.vpn()) {
            if pte
                .flags()
                .contains(PTEFlags::R | PTEFlags::W | PTEFlags::U)
            {
                return Ok(UserMut { ptr });
            }
        }
        Err(errno::EFAULT)
    })
}

/// 检查一个指向连续切片的指针（如字符串）的可读性以及是否有 U 标志
///
/// TODO: 目前单纯是检查了下切片头部，未来可以根据长度计算是否跨等检查
///
/// # Safety
///
/// 需要用户保证 non-alias
#[track_caller]
pub fn check_slice<T>(ptr: *const T, len: usize) -> Result<UserConst<[T]>> {
    curr_process().lock_inner(|inner| {
        let va = VirtAddr::from(ptr);
        if let Some(pte) = inner.memory_set.page_table().find_pte(va.vpn()) {
            if pte.flags().contains(PTEFlags::R | PTEFlags::U) {
                return Ok(UserConst {
                    ptr: core::ptr::slice_from_raw_parts(ptr, len),
                });
            }
        }
        Err(errno::EFAULT)
    })
}

/// 检查一个指向连续切片的指针（如字符串）的读写性以及是否有 U 标志
///
/// TODO: 目前单纯是检查了下切片头部，未来可以根据长度计算是否跨等检查
///
/// # Safety
///
/// 需要用户保证 non-alias
#[track_caller]
pub fn check_slice_mut<T>(ptr: *mut T, len: usize) -> Result<UserMut<[T]>> {
    curr_process().lock_inner(|inner| {
        let va = VirtAddr::from(ptr);
        if let Some(pte) = inner.memory_set.page_table().find_pte(va.vpn()) {
            if pte
                .flags()
                .contains(PTEFlags::R | PTEFlags::W | PTEFlags::U)
            {
                return Ok(UserMut {
                    ptr: core::ptr::slice_from_raw_parts_mut(ptr, len),
                });
            }
        }
        Err(errno::EFAULT)
    })
}

/// 检查 null-terminated 的字符串指针（只读和 U 标志）
///
/// TODO: 目前只检查了字符串开头，未来应当根据跨页检查
///
/// # Safety
///
/// 需要用户保证 non-alias
#[track_caller]
pub fn check_cstr(ptr: *const u8) -> Result<UserConst<str>> {
    curr_process().lock_inner(|inner| {
        let va = VirtAddr::from(ptr);
        if let Some(pte) = inner.memory_set.page_table().find_pte(va.vpn()) {
            if pte.flags().contains(PTEFlags::R | PTEFlags::U) {
                return Ok(UserConst {
                    ptr: unsafe {
                        core::ffi::CStr::from_ptr(ptr.cast()).to_str().unwrap() as *const str
                    },
                });
            }
        }
        Err(errno::EFAULT)
    })
}
