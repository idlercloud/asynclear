#![no_std]

use alloc::vec::Vec;

extern crate alloc;

/// 基于回收的分配器，即用 vector 收集释放的 id
#[derive(Clone)]
pub struct RecycleAllocator {
    current: usize,
    recycled: Vec<usize>,
}

impl Default for RecycleAllocator {
    fn default() -> Self {
        Self::new()
    }
}

impl RecycleAllocator {
    /// 默认从 0 开始分配
    pub const fn new() -> Self {
        RecycleAllocator {
            current: 0,
            recycled: Vec::new(),
        }
    }

    /// 显式决定从哪个数开始分配
    pub const fn begin_with(begin: usize) -> Self {
        RecycleAllocator {
            current: begin,
            recycled: Vec::new(),
        }
    }

    pub fn alloc(&mut self) -> usize {
        if let Some(id) = self.recycled.pop() {
            id
        } else {
            self.current += 1;
            self.current - 1
        }
    }

    pub fn dealloc(&mut self, id: usize) {
        debug_assert!(id < self.current);
        debug_assert!(!self.recycled.iter().any(|i| *i == id), "id {id} has been deallocated!",);
        self.recycled.push(id);
    }

    /// 释放所使用的内存。一般而言，释放之后不应该再使用
    pub fn release(&mut self) {
        self.recycled = Vec::new();
    }
}
