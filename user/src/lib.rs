#![no_std]
#![feature(linkage)]
#![feature(panic_info_message)]
#![feature(alloc_error_handler)]
#![feature(naked_functions)]

#[macro_use]
pub mod console;
mod lang_items;
mod syscall;

extern crate alloc;

use alloc::vec::Vec;
use core::{
    alloc::{GlobalAlloc, Layout},
    ffi::CStr,
    ptr::NonNull,
    time::Duration,
};

use buddy_system_allocator::Heap;
use defines::{
    fs::{OpenFlags, AT_FDCWD},
    misc::TimeSpec,
    signal::SIGCHLD,
};
use spin::mutex::Mutex;

pub use self::{
    console::{flush, STDIN, STDOUT},
    syscall::*,
};

#[global_allocator]
static HEAP: BrkBasedHeap = BrkBasedHeap::new();

struct BrkBasedHeap {
    inner: Mutex<HeapInner>,
}

unsafe impl Sync for BrkBasedHeap {}

impl BrkBasedHeap {
    const fn new() -> Self {
        Self {
            inner: Mutex::new(HeapInner {
                brk: 0,
                buddy_system: Heap::new(),
            }),
        }
    }

    fn init(&self) {
        let brk = sys_brk(0);
        self.inner.lock().brk = brk as usize;
    }
}

const PAGE_SIZE: usize = 4096;

unsafe impl GlobalAlloc for BrkBasedHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut inner = self.inner.lock();
        if let Ok(ret) = inner.buddy_system.alloc(layout) {
            return ret.as_ptr();
        }
        let size = usize::max(
            layout.size().next_power_of_two(),
            usize::max(layout.align(), core::mem::size_of::<usize>()),
        );
        let new_brk = sys_brk((inner.brk + size).next_multiple_of(PAGE_SIZE * 4)) as usize;
        if new_brk <= inner.brk {
            return core::ptr::null_mut();
        }
        let old_brk = inner.brk;
        unsafe { inner.buddy_system.add_to_heap(old_brk, new_brk) };
        inner.brk = new_brk;
        inner
            .buddy_system
            .alloc(layout)
            .ok()
            .map_or(core::ptr::null_mut(), |allocation| allocation.as_ptr())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.inner
            .lock()
            .buddy_system
            .dealloc(unsafe { NonNull::new_unchecked(ptr) }, layout);
    }
}

struct HeapInner {
    brk: usize,
    buddy_system: Heap<32>,
}

#[alloc_error_handler]
pub fn handle_alloc_error(layout: core::alloc::Layout) -> ! {
    panic!("Heap allocation error, layout = {:?}", layout);
}

fn clear_bss() {
    extern "C" {
        fn start_bss();
        fn end_bss();
    }
    unsafe {
        core::slice::from_raw_parts_mut(
            start_bss as usize as *mut u8,
            end_bss as usize - start_bss as usize,
        )
        .fill(0);
    }
}

#[no_mangle]
#[link_section = ".text.entry"]
pub extern "C" fn _start(argc: usize, argv: usize) -> ! {
    clear_bss();
    HEAP.init();
    let mut v: Vec<&'static str> = Vec::new();
    for i in 0..argc {
        let str_start =
            unsafe { ((argv + i * core::mem::size_of::<usize>()) as *const usize).read_volatile() };
        let len = (0usize..)
            .find(|i| unsafe { ((str_start + *i) as *const u8).read_volatile() == 0 })
            .unwrap();
        v.push(
            core::str::from_utf8(unsafe {
                core::slice::from_raw_parts(str_start as *const u8, len)
            })
            .unwrap(),
        );
    }
    exit(main(argc, v.as_slice()));
}

#[linkage = "weak"]
#[no_mangle]
fn main(_argc: usize, _argv: &[&str]) -> i32 {
    panic!("Cannot find main!");
}

pub fn open(path: &CStr, flags: OpenFlags) -> isize {
    sys_openat(AT_FDCWD, path, flags.bits(), 0)
}

pub fn close(fd: i32) -> isize {
    if fd == STDOUT {
        console::flush();
    }
    sys_close(fd)
}

pub fn lseek(fd: i32, offset: i64, whence: usize) -> isize {
    sys_lseek(fd, offset, whence)
}

pub fn read(fd: i32, buf: &mut [u8]) -> isize {
    sys_read(fd, buf)
}

pub fn write(fd: i32, buf: &[u8]) -> isize {
    sys_write(fd, buf.as_ptr(), buf.len())
}

pub fn write_all(fd: i32, buf: &[u8]) -> isize {
    let mut n_write = 0;
    while n_write < buf.len() {
        let ret = write(fd, buf);
        if ret < 0 {
            return ret;
        }
        if ret == 0 {
            break;
        }
        n_write = n_write.wrapping_add_signed(ret);
    }
    n_write as isize
}

// pub fn link(old_path: &str, new_path: &str) -> isize {
//     sys_linkat(AT_FDCWD as usize, old_path, AT_FDCWD as usize, new_path, 0)
// }

// pub fn unlink(path: &str) -> isize {
//     sys_unlinkat(AT_FDCWD as usize, path, 0)
// }

// pub fn fstat(fd: usize, st: &Stat) -> isize {
//     sys_fstat(fd, st)
// }

pub fn exit(exit_code: i32) -> ! {
    console::flush();
    sys_exit(exit_code);
}

pub fn yield_() -> isize {
    sys_yield()
}

pub fn getpid() -> isize {
    sys_getpid()
}

pub fn getppid() -> isize {
    sys_getppid()
}

pub fn fork() -> isize {
    sys_clone(SIGCHLD as usize)
}

pub fn exec(path: &CStr, args: &[*const u8]) -> isize {
    unsafe { sys_execve(path.as_ptr().cast(), args.as_ptr().cast()) }
}

pub fn set_priority(prio: isize) -> isize {
    sys_set_priority(prio)
}

pub fn wait(exit_code: &mut i32) -> isize {
    loop {
        match sys_waitpid(-1, exit_code as *mut _) {
            0 => {
                sys_yield();
            }
            n => {
                return n;
            }
        }
    }
}

pub fn waitpid(pid: usize, exit_code: &mut i32) -> isize {
    loop {
        match sys_waitpid(pid as isize, exit_code as *mut _) {
            0 => {
                sys_yield();
            }
            n => {
                if n > 0 {
                    *exit_code = (*exit_code & 0xff00) >> 8;
                    if *exit_code & 0b10000000 != 0 {
                        *exit_code |= 0xffffff00u32 as i32;
                    }
                }
                return n;
            }
        }
    }
}

pub fn munmap(start: usize, len: usize) -> isize {
    sys_munmap(start, len)
}

pub fn chdir(path: &CStr) -> isize {
    sys_chdir(path)
}

// pub fn dup(fd: usize) -> isize {
//     sys_dup(fd)
// }

// pub fn pipe(pipe_fd: &mut [usize]) -> isize {
//     sys_pipe(pipe_fd)
// }

pub fn gettid() -> isize {
    sys_gettid()
}

pub fn gettime() -> TimeSpec {
    let mut ts = TimeSpec::default();
    if sys_clock_gettime(0, &mut ts as _) < 0 {
        println!("gettime failed");
    }
    ts
}

pub fn pipe(pipe_fd: &mut [i32; 2]) -> isize {
    sys_pipe(pipe_fd, 0)
}

pub fn test_main(name: &str, f: impl FnOnce()) {
    println!("----{} begins----", name);
    f();
    println!("----{} ends  ----", name);
}

pub fn bench_main(name: &str, mut f: impl FnMut(), time: usize) {
    println!("===={} begins====", name);
    let begin = Duration::try_from(gettime()).unwrap();
    for _ in 0..time {
        f();
    }
    let end = Duration::try_from(gettime()).unwrap();
    let elasped = end - begin;
    println!("===={} ends {}ms ====", name, elasped.as_millis());
}
