use core::fmt::Arguments;

use crate::Level;

#[derive(Clone, Debug)]
pub struct Record<'a> {
    level: Level,
    args: Arguments<'a>,
    // module_path: &'static str,
    // file: &'static str,
    // line: u32,
}

impl<'a> Record<'a> {
    #[inline]
    pub fn new(
        level: Level,
        args: Arguments<'a>,
        // module_path: &'static str,
        // file: &'static str,
        // line: u32,
    ) -> Self {
        Self {
            level,
            args,
            // module_path,
            // file,
            // line,
        }
    }

    /// The message body.
    #[inline]
    pub fn args(&self) -> &Arguments<'a> {
        &self.args
    }

    /// The verbosity level of the message.
    #[inline]
    pub fn level(&self) -> Level {
        self.level
    }

    // /// The module path of the message.
    // #[inline]
    // pub fn module_path(&self) -> &'static str {
    //     self.module_path
    // }

    // /// The source file containing the message.
    // #[inline]
    // pub fn file(&self) -> &'static str {
    //     self.file
    // }

    // /// The line containing the message.
    // #[inline]
    // pub fn line(&self) -> u32 {
    //     self.line
    // }
}
