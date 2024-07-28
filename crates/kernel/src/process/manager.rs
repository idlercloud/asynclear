use alloc::collections::BTreeMap;

use klocks::{SpinMutex, SpinMutexGuard};
use triomphe::Arc;

use super::Process;

pub struct ProcessManager(SpinMutex<BTreeMap<usize, Arc<Process>>>);

impl ProcessManager {
    pub const fn new() -> Self {
        Self(SpinMutex::new(BTreeMap::new()))
    }

    pub fn add(&self, pid: usize, process: Arc<Process>) {
        self.0.lock().insert(pid, process);
    }

    pub fn remove(&self, pid: usize) {
        self.0.lock().remove(&pid);
    }

    pub fn get(&self, pid: usize) -> Option<Arc<Process>> {
        self.0.lock().get(&pid).cloned()
    }

    pub fn init_proc(&self) -> Arc<Process> {
        Arc::clone(self.0.lock().get(&1).expect("initproc should never die"))
    }

    pub fn lock_all(&self) -> SpinMutexGuard<'_, BTreeMap<usize, Arc<Process>>> {
        self.0.lock()
    }
}
