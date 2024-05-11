use alloc::collections::{btree_map::Entry, BTreeMap};

use compact_str::CompactString;
use defines::error::KResult;
use klocks::{SpinMutex, SpinMutexGuard};
use triomphe::Arc;

use super::inode::{DynDirInode, DynInode, DynPagedInode, InodeMeta};

#[derive(Clone)]
pub enum DEntry {
    Dir(Arc<DEntryDir>),
    Paged(DEntryPaged),
}

impl DEntry {
    pub fn meta(&self) -> &InodeMeta {
        match self {
            DEntry::Dir(dir) => dir.inode.meta(),
            DEntry::Paged(paged) => paged.inode.meta(),
        }
    }
}

pub struct DEntryDir {
    parent: Option<Arc<DEntryDir>>,
    children: SpinMutex<BTreeMap<CompactString, Option<DEntry>>>,
    inode: Arc<DynDirInode>,
}

impl DEntryDir {
    pub fn new(parent: Option<Arc<DEntryDir>>, inode: Arc<DynDirInode>) -> Self {
        Self {
            parent,
            children: SpinMutex::new(BTreeMap::new()),
            inode,
        }
    }

    pub fn lookup(self: &Arc<Self>, component: CompactString) -> Option<DEntry> {
        if component == "." {
            return Some(DEntry::Dir(Arc::clone(self)));
        } else if component == ".." {
            return Some(DEntry::Dir(Arc::clone(
                self.parent.as_ref().unwrap_or(self),
            )));
        }
        let mut children = self.children.lock();
        let entry = children.entry(component);
        match entry {
            Entry::Vacant(vacant) => {
                let Some(new_inode) = self.inode.lookup(vacant.key()) else {
                    vacant.insert(None);
                    return None;
                };
                let new_dentry = match new_inode {
                    DynInode::Dir(dir) => {
                        DEntry::Dir(Arc::new(DEntryDir::new(Some(Arc::clone(self)), dir)))
                    }
                    DynInode::Paged(paged) => {
                        DEntry::Paged(DEntryPaged::new(Arc::clone(self), paged))
                    }
                };
                vacant.insert(Some(new_dentry.clone()));
                Some(new_dentry)
            }
            Entry::Occupied(occupied) => occupied.get().clone(),
        }
    }

    pub fn read_dir(self: &Arc<Self>) -> KResult<()> {
        let _enter = debug_span!("read_dir", name = self.inode.meta().name()).entered();
        self.inode.read_dir(self)
    }

    pub fn lock_children(&self) -> SpinMutexGuard<'_, BTreeMap<CompactString, Option<DEntry>>> {
        self.children.lock()
    }

    pub fn inode(&self) -> &Arc<DynDirInode> {
        &self.inode
    }
}

#[derive(Clone)]
pub struct DEntryPaged {
    parent: Arc<DEntryDir>,
    inode: Arc<DynPagedInode>,
}

impl DEntryPaged {
    pub fn new(parent: Arc<DEntryDir>, inode: Arc<DynPagedInode>) -> Self {
        Self { parent, inode }
    }

    pub fn inode(&self) -> &Arc<DynPagedInode> {
        &self.inode
    }

    pub fn parent(&self) -> &Arc<DEntryDir> {
        &self.parent
    }
}
