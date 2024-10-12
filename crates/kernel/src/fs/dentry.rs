use alloc::collections::{btree_map::Entry, BTreeMap};
use core::hash::Hash;

use defines::error::{errno, KResult};
use ecow::EcoString;
use klocks::{SpinMutex, SpinMutexGuard};
use smallvec::SmallVec;
use triomphe::Arc;

use super::inode::{DynBytesInode, DynDirInode, DynInode, InodeMeta, InodeMode};
use crate::time;

#[derive(Clone)]
pub enum DEntry {
    Dir(Arc<DEntryDir>),
    Bytes(Arc<DEntryBytes>),
}

impl Hash for DEntry {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        state.write_usize(self.addr());
    }
}

impl PartialEq for DEntry {
    fn eq(&self, other: &Self) -> bool {
        self.addr() == other.addr()
    }
}

impl Eq for DEntry {}

impl DEntry {
    pub fn meta(&self) -> &InodeMeta {
        match self {
            DEntry::Dir(dir) => dir.inode.meta(),
            DEntry::Bytes(bytes) => bytes.inode.meta(),
        }
    }

    pub fn name(&self) -> &EcoString {
        match self {
            DEntry::Dir(dir) => dir.name(),
            DEntry::Bytes(bytes) => bytes.name(),
        }
    }

    fn addr(&self) -> usize {
        match self {
            DEntry::Dir(dir) => dir.as_ptr().addr(),
            DEntry::Bytes(bytes) => bytes.inode.as_ptr().addr(),
        }
    }

    pub fn is_dir(&self) -> bool {
        matches!(self, DEntry::Dir(_))
    }
}

pub struct DEntryDir {
    parent: Option<Arc<DEntryDir>>,
    name: EcoString,
    children: SpinMutex<BTreeMap<EcoString, DEntry>>,
    inode: Arc<DynDirInode>,
}

impl DEntryDir {
    pub fn new(parent: Option<Arc<DEntryDir>>, name: EcoString, inode: Arc<DynDirInode>) -> Self {
        Self {
            parent,
            name,
            children: SpinMutex::new(BTreeMap::new()),
            inode,
        }
    }

    pub fn lookup(self: &Arc<Self>, component: impl Into<EcoString> + AsRef<str>) -> Option<DEntry> {
        fn special(parent: &Arc<DEntryDir>, component: &str) -> Option<DEntry> {
            let curr_time = time::curr_time_spec();
            parent
                .inode
                .meta()
                .lock_inner_with(|inner| inner.access_time = curr_time);
            if component == "." {
                Some(DEntry::Dir(Arc::clone(parent)))
            } else if component == ".." {
                Some(DEntry::Dir(Arc::clone(parent.parent.as_ref().unwrap_or(parent))))
            } else {
                None
            }
        }
        fn general(parent: &Arc<DEntryDir>, component: EcoString) -> Option<DEntry> {
            let mut children = parent.children.lock();
            let entry = children.entry(component);
            match entry {
                Entry::Vacant(vacant) => {
                    let new_inode = parent.inode.lookup(vacant.key())?;
                    let new_dentry = match new_inode {
                        DynInode::Dir(dir) => DEntry::Dir(Arc::new(DEntryDir::new(
                            Some(Arc::clone(parent)),
                            vacant.key().clone(),
                            dir,
                        ))),
                        DynInode::Bytes(bytes) => DEntry::Bytes(Arc::new(DEntryBytes::new(
                            Arc::clone(parent),
                            vacant.key().clone(),
                            bytes,
                        ))),
                    };
                    vacant.insert(new_dentry.clone());
                    Some(new_dentry)
                }
                Entry::Occupied(occupied) => Some(occupied.get().clone()),
            }
        }
        special(self, component.as_ref()).or_else(|| general(self, component.into()))
    }

    pub fn mkdir(self: &Arc<Self>, component: EcoString) -> KResult<Arc<DEntryDir>> {
        if component == "." || component == ".." {
            return Err(errno::EINVAL);
        }
        let mut children = self.children.lock();
        let vacant = match children.entry(component) {
            Entry::Vacant(vacant) => vacant,
            Entry::Occupied(_) => return Err(errno::EEXIST),
        };
        let dir = self.inode.mkdir(vacant.key())?;
        let dentry = Arc::new(DEntryDir::new(Some(Arc::clone(self)), vacant.key().clone(), dir));
        vacant.insert(DEntry::Dir(Arc::clone(&dentry)));
        Ok(dentry)
    }

    pub fn mknod(self: &Arc<Self>, component: EcoString, mode: InodeMode) -> KResult<Arc<DEntryBytes>> {
        if matches!(mode, InodeMode::SymbolLink | InodeMode::Dir) || component == "." || component == ".." {
            return Err(errno::EINVAL);
        }
        let mut children = self.children.lock();
        let vacant = match children.entry(component) {
            Entry::Vacant(vacant) => vacant,
            Entry::Occupied(_) => return Err(errno::EEXIST),
        };
        let file = self.inode.mknod(vacant.key(), mode)?;
        let dentry = Arc::new(DEntryBytes::new(Arc::clone(self), vacant.key().clone(), file));
        vacant.insert(DEntry::Bytes(dentry.clone()));
        Ok(dentry)
    }

    pub fn unlink(self: &Arc<Self>, name: &str) -> KResult<()> {
        if name == "." || name == ".." {
            return Err(errno::EINVAL);
        }
        let mut children = self.lock_children();
        children.remove(name);
        // TODO: [low] 其实这里要考虑硬链接之类的问题？
        self.inode.unlink(name)
    }

    pub fn read_dir(self: &Arc<Self>) -> KResult<()> {
        let _enter = debug_span!("read_dir", name = self.name).entered();
        self.inode.read_dir(self)
    }

    pub fn parent(&self) -> Option<&Arc<DEntryDir>> {
        self.parent.as_ref()
    }

    pub fn name(&self) -> &EcoString {
        &self.name
    }

    pub fn lock_children(&self) -> SpinMutexGuard<'_, BTreeMap<EcoString, DEntry>> {
        self.children.lock()
    }

    pub fn inode(&self) -> &Arc<DynDirInode> {
        &self.inode
    }

    pub fn path(&self) -> EcoString {
        let mut dirs = SmallVec::<[&DEntryDir; 4]>::new();
        let mut dir = self;
        // 根目录 `/` 和 `\0`
        loop {
            let Some(parent) = dir.parent() else {
                break;
            };
            dirs.push(dir);
            dir = parent;
        }

        let mut path = EcoString::from("/");

        for component in dirs.into_iter().rev().map(|dir| dir.name().as_str()).intersperse("/") {
            path.push_str(component);
        }

        path
    }
}

pub struct DEntryBytes {
    parent: Arc<DEntryDir>,
    name: EcoString,
    inode: Arc<DynBytesInode>,
}

impl DEntryBytes {
    pub fn new(parent: Arc<DEntryDir>, name: EcoString, inode: Arc<DynBytesInode>) -> Self {
        Self { parent, name, inode }
    }

    pub fn parent(&self) -> &Arc<DEntryDir> {
        &self.parent
    }

    pub fn name(&self) -> &EcoString {
        &self.name
    }

    pub fn inode(&self) -> &Arc<DynBytesInode> {
        &self.inode
    }
}
