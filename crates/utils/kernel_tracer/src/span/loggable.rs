use alloc::{string::String, vec::Vec};

use ecow::EcoString;

/// 可用于 span 宏键值对中值的类型
pub trait Loggable {
    fn log(&self, writer: &mut EcoString);
}

// 要经过这个转一道。
// 无法 impl<T: Display> Loggable for T 后再去给其他上游类型 impl Loggable 因为上游随时可能为该类型实现 Display，导致冲突
trait SpecDisplay: core::fmt::Display {}

macro_rules! spec_display_impl {
    ($($t:tt)*) => ($(
        impl SpecDisplay for $t {}
    )*);
}

spec_display_impl!(u8 u16 u32 u64 usize i8 i16 i32 i64 str char String EcoString);

impl<T: SpecDisplay + ?Sized> Loggable for T {
    fn log(&self, writer: &mut EcoString) {
        core::fmt::write(writer, format_args!("{self}")).unwrap();
    }
}

impl<T: SpecDisplay + ?Sized> SpecDisplay for &T {}

impl<T: SpecDisplay> Loggable for [T] {
    fn log(&self, writer: &mut EcoString) {
        writer.push_str("[");
        let mut rest = false;
        for t in self {
            if rest {
                writer.push_str(", ");
            }
            core::fmt::write(writer, format_args!("{t}")).unwrap();
            rest = true;
        }
        writer.push_str("]");
    }
}

impl<T: SpecDisplay> Loggable for Vec<T> {
    fn log(&self, writer: &mut EcoString) {
        self.as_slice().log(writer);
    }
}
