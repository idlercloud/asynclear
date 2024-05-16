use alloc::collections::{btree_map::Entry, BTreeMap};

use cervine::Cow;
use compact_str::CompactString;
use defines::{
    error::{errno, KResult},
    misc::TimeSpec,
};
use klocks::{SpinMutex, SpinMutexGuard};
use triomphe::Arc;

use super::inode::{DynDirInode, DynInode, DynPagedInode, InodeMeta, InodeMode};
use crate::time;

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

    pub fn lookup(self: &Arc<Self>, component: Cow<'_, CompactString, str>) -> Option<DEntry> {
        self.inode
            .meta()
            .lock_inner_with(|inner| inner.access_time = TimeSpec::from(time::curr_time()));
        if &component == "." {
            return Some(DEntry::Dir(Arc::clone(self)));
        } else if &component == ".." {
            return Some(DEntry::Dir(Arc::clone(
                self.parent.as_ref().unwrap_or(self),
            )));
        }
        let component = component.into_owned();
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
                    DynInode::Paged(paged) => DEntry::Paged(DEntryPaged::new(paged)),
                };
                vacant.insert(Some(new_dentry.clone()));
                Some(new_dentry)
            }
            Entry::Occupied(occupied) => occupied.get().clone(),
        }
    }

    pub fn mkdir(self: &Arc<Self>, component: CompactString) -> KResult<Arc<DEntryDir>> {
        if component == "." || component == ".." {
            return Err(errno::EINVAL);
        }
        let mut children = self.children.lock();
        let child_entry = children.entry(component);
        if let Entry::Occupied(occupied) = &child_entry
            && occupied.get().is_some()
        {
            return Err(errno::EEXIST);
        }
        let dir = self.inode.mkdir(child_entry.key())?;
        let dentry = Arc::new(DEntryDir::new(Some(Arc::clone(self)), dir));
        *child_entry.or_insert(None) = Some(DEntry::Dir(Arc::clone(&dentry)));
        Ok(dentry)
    }

    pub fn mknod(
        self: &Arc<Self>,
        component: CompactString,
        mode: InodeMode,
    ) -> KResult<DEntryPaged> {
        if matches!(mode, InodeMode::SymbolLink | InodeMode::Dir)
            || component == "."
            || component == ".."
        {
            return Err(errno::EINVAL);
        }
        let mut children = self.children.lock();
        let child_entry = children.entry(component);
        if let Entry::Occupied(occupied) = &child_entry
            && occupied.get().is_some()
        {
            return Err(errno::EEXIST);
        }
        let file = self.inode.mknod(child_entry.key(), mode)?;
        let dentry = DEntryPaged::new(file);
        *child_entry.or_insert(None) = Some(DEntry::Paged(dentry.clone()));
        Ok(dentry)
    }

    pub fn read_dir(self: &Arc<Self>) -> KResult<()> {
        let _enter = debug_span!("read_dir", name = self.inode.meta().name()).entered();
        self.inode.read_dir(self)
    }

    pub fn parent(&self) -> Option<&Arc<DEntryDir>> {
        self.parent.as_ref()
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
    inode: Arc<DynPagedInode>,
}

impl DEntryPaged {
    pub fn new(inode: Arc<DynPagedInode>) -> Self {
        Self { inode }
    }

    pub fn inode(&self) -> &Arc<DynPagedInode> {
        &self.inode
    }
}
