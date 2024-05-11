use core::fmt::Arguments;

use crate::Level;

#[derive(Clone, Debug)]
pub struct Record<'a> {
    level: Level,
    args: Arguments<'a>,
}

impl<'a> Record<'a> {
    #[inline]
    pub fn new(level: Level, args: Arguments<'a>) -> Self {
        Self { level, args }
    }

    /// 消息内容
    #[inline]
    pub fn args(&self) -> &Arguments<'a> {
        &self.args
    }

    /// 消息的日志等级
    #[inline]
    pub fn level(&self) -> Level {
        self.level
    }
}
