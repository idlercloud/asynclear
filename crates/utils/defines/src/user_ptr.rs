use compact_str::{CompactString, ToCompactString};

pub struct UserConst<T: ?Sized> {
    ptr: *const T,
}

impl<T: ?Sized> UserConst<T> {
    pub fn from_raw(ptr: *const T) -> Self {
        Self { ptr }
    }

    pub fn as_raw(&self) -> *const T {
        self.ptr
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
    pub fn from_raw(ptr: *mut T) -> Self {
        Self { ptr }
    }

    pub fn raw(&self) -> *mut T {
        self.ptr
    }
}

unsafe impl<T: ?Sized> Send for UserMut<T> {}
