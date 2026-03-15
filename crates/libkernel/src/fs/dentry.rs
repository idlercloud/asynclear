use alloc::collections::{btree_map::Entry, BTreeMap};
use core::hash::Hash;

use defines::error::{errno, KResult};
use ecow::EcoString;
use executor::time;
use klocks::{SpinMutex, SpinMutexGuard};
use smallvec::SmallVec;
use triomphe::Arc;

use super::inode::{DynBytesInode, DynDirInode, DynInode, InodeMeta, InodeMode};

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

    pub fn remove_dir(&self, child: Arc<DEntryDir>) -> KResult<()> {
        assert_eq!(
            Some((self as *const DEntryDir).addr()),
            child.parent.as_ref().map(|d| d.as_ptr().addr())
        );
        let mut children = self.lock_children();
        // 只允许移除空目录，也就是除了 `.` 和 `..` 没有别的条目
        if children.len() == 2 {
            return Err(errno::ENOTEMPTY);
        }
        children.remove(child.name());
        Ok(())
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
        while let Some(parent) = dir.parent() {
            dirs.push(dir);
            dir = parent;
        }

        let mut path = EcoString::from("/");

        for component in dirs.into_iter().rev().map(|dir| dir.name().as_str()).intersperse("/") {
            path.push_str(component);
        }

        path
    }

    pub fn rename(self: &Arc<Self>, new_dir: &Arc<DEntryDir>, new_name: EcoString) -> KResult {
        let mut old_children;
        let mut new_children;
        let Some(old_dir) = &self.parent else {
            // 根目录不许重命名
            return Err(errno::EBUSY);
        };
        // 按地址顺序加锁，防止死锁
        if old_dir.as_ptr().addr() < new_dir.as_ptr().addr() {
            old_children = old_dir.lock_children();
            new_children = new_dir.lock_children();
        } else if old_dir.as_ptr().addr() > new_dir.as_ptr().addr() {
            new_children = new_dir.lock_children();
            old_children = old_dir.lock_children();
        } else {
            // TODO: 看看能不能优化
            let mut children = new_dir.lock_children();
            children.remove(self.name());
            let new_entry = Arc::new(DEntryDir::new(
                Some(Arc::clone(new_dir)),
                new_name.clone(),
                Arc::clone(self.inode()),
            ));
            children.insert(new_name, DEntry::Dir(new_entry));
            return Ok(0);
        }

        old_children.remove(self.name());
        let new_entry = Arc::new(DEntryDir::new(
            Some(Arc::clone(new_dir)),
            new_name.clone(),
            Arc::clone(self.inode()),
        ));
        new_children.insert(new_name, DEntry::Dir(new_entry));
        Ok(0)
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

    pub fn rename(self: &Arc<Self>, new_dir: &Arc<DEntryDir>, new_name: EcoString) -> KResult {
        let mut old_children;
        let mut new_children;
        // 按地址顺序加锁，防止死锁
        if self.parent.as_ptr().addr() < new_dir.as_ptr().addr() {
            old_children = self.parent.lock_children();
            new_children = new_dir.lock_children();
        } else if self.parent.as_ptr().addr() > new_dir.as_ptr().addr() {
            new_children = new_dir.lock_children();
            old_children = self.parent.lock_children();
        } else {
            // TODO: 看看能不能优化
            let mut children = new_dir.lock_children();
            children.remove(self.name());
            let new_entry = Arc::new(DEntryBytes::new(
                Arc::clone(new_dir),
                new_name.clone(),
                Arc::clone(self.inode()),
            ));
            children.insert(new_name, DEntry::Bytes(new_entry));
            return Ok(0);
        }

        old_children.remove(self.name());
        let new_entry = Arc::new(DEntryBytes::new(
            Arc::clone(new_dir),
            new_name.clone(),
            Arc::clone(self.inode()),
        ));
        new_children.insert(new_name, DEntry::Bytes(new_entry));
        Ok(0)
    }
}
