mod stdio;

use alloc::boxed::Box;
use triomphe::Arc;

use async_trait::async_trait;
use defines::error::Result;
use user_check::{UserCheck, UserCheckMut};

use self::stdio::{read_stdin, write_stdout};

#[derive(Clone)]
pub enum File {
    Stdin,
    Stdout,
    DynFile(Arc<dyn DynFile>),
}

impl File {
    pub async fn read(&self, buf: UserCheckMut<[u8]>) -> Result<usize> {
        match self {
            File::Stdin => read_stdin(buf).await,
            File::Stdout => panic!("stdout cannot be read"),
            File::DynFile(dyn_file) => dyn_file.read(buf).await,
        }
    }

    pub async fn write(&self, buf: UserCheck<[u8]>) -> Result<usize> {
        match self {
            File::Stdin => panic!("stdout cannot be read"),
            File::Stdout => write_stdout(buf),
            File::DynFile(dyn_file) => dyn_file.write(buf).await,
        }
    }
}

#[async_trait]
pub trait DynFile: Send + Sync {
    async fn read(&self, buf: UserCheckMut<[u8]>) -> Result<usize>;
    async fn write(&self, buf: UserCheck<[u8]>) -> Result<usize>;
}
