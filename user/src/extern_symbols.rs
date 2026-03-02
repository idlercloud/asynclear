//! 外部符号的集中声明
//!
//! 本模块集中声明所有通过 `extern "C"` 访问的外部符号。

unsafe extern "C" {
    /// BSS 段起始地址
    pub fn start_bss();
    /// BSS 段结束地址
    pub fn end_bss();
}
