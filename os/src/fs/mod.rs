pub mod ext4;
mod inode;
mod pipe;
mod stdio;

use crate::mm::UserBuffer;

pub trait File: Send + Sync {
    fn readable(&self) -> bool;
    fn writable(&self) -> bool;
    fn read(&self, buf: UserBuffer) -> usize;
    fn write(&self, buf: UserBuffer) -> usize;
    /// 文件大小（fstat/mmap 需要）
    fn size(&self) -> usize {
        0
    }
}

pub use inode::{is_unlinked, list_apps, mark_unlinked, open_file, root_names, OSInode, OpenFlags};
pub use pipe::{make_pipe, Pipe};

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use lazy_static::*;

lazy_static! {
    /// 启动探测到的 EXT4 根文件系统（只读）
    pub static ref EXT4: Option<Arc<ext4::Ext4Fs>> = ext4::Ext4Fs::new().map(Arc::new);
}

pub fn has_ext4() -> bool {
    EXT4.is_some()
}

/// EXT4 是只读的，这里用内存 overlay 支持创建/删除，
/// 让 close/open(O_CREATE)/mkdir/unlink 这类用例可以工作。
lazy_static! {
    static ref OVERLAY: crate::sync::UPSafeCell<BTreeMap<String, Arc<crate::sync::UPSafeCell<Vec<u8>>>>> =
        unsafe { crate::sync::UPSafeCell::new(BTreeMap::new()) };
}

pub type OverlayData = Arc<crate::sync::UPSafeCell<Vec<u8>>>;

pub fn overlay_get(name: &str) -> Option<OverlayData> {
    OVERLAY.exclusive_access().get(name).cloned()
}

pub fn overlay_create(name: &str) -> OverlayData {
    let mut map = OVERLAY.exclusive_access();
    if let Some(d) = map.get(name) {
        return d.clone();
    }
    let d: OverlayData = Arc::new(unsafe { crate::sync::UPSafeCell::new(Vec::new()) });
    map.insert(String::from(name), d.clone());
    d
}

pub fn overlay_remove(name: &str) {
    OVERLAY.exclusive_access().remove(name);
}

pub struct OverlayFile {
    data: OverlayData,
    offset: crate::sync::UPSafeCell<usize>,
}

impl OverlayFile {
    pub fn new(data: OverlayData) -> Self {
        Self {
            data,
            offset: unsafe { crate::sync::UPSafeCell::new(0) },
        }
    }
}

impl File for OverlayFile {
    fn readable(&self) -> bool {
        true
    }
    fn writable(&self) -> bool {
        true
    }
    fn read(&self, buf: UserBuffer) -> usize {
        let data = self.data.exclusive_access();
        let mut off = self.offset.exclusive_access();
        let mut n = 0;
        for ptr in buf {
            if *off >= data.len() {
                break;
            }
            unsafe { ptr.write_volatile(data[*off]); }
            *off += 1;
            n += 1;
        }
        n
    }
    fn size(&self) -> usize {
        self.data.exclusive_access().len()
    }
    fn write(&self, buf: UserBuffer) -> usize {
        let mut data = self.data.exclusive_access();
        let mut off = self.offset.exclusive_access();
        let mut n = 0;
        for ptr in buf {
            let b = unsafe { ptr.read_volatile() };
            if *off < data.len() {
                data[*off] = b;
            } else {
                data.push(b);
            }
            *off += 1;
            n += 1;
        }
        n
    }
}

/// 读出整个文件（内核内部用，例如 exec 加载 ELF）
pub fn read_file_all(name: &str) -> Option<Vec<u8>> {
    read_file_all_inner(name)
}

fn read_file_all_inner(name: &str) -> Option<Vec<u8>> {
    if let Some(d) = overlay_get(name) {
        return Some(d.exclusive_access().clone());
    }
    if let Some(fs) = EXT4.as_ref() {
        let inode = fs.lookup(name)?;
        Some(fs.read_file(&inode))
    } else {
        open_file(name, OpenFlags::RDONLY).map(|i| i.read_all())
    }
}

/// 打开文件并返回统一的 File 对象（供系统调用使用）
pub fn open_dyn(name: &str, flags: OpenFlags) -> Option<Arc<dyn File + Send + Sync>> {
    // 根目录用一个伪文件代表，供 getdents 使用
    if is_root_path(name) && EXT4.is_some() {
        return Some(Arc::new(OverlayFile::new(overlay_create("/.rootdir"))));
    }
    if let Some(d) = overlay_get(name) {
        return Some(Arc::new(OverlayFile::new(d)));
    }
    if let Some(fs) = EXT4.as_ref() {
        if flags.contains(OpenFlags::CREATE) {
            return Some(Arc::new(OverlayFile::new(overlay_create(name))));
        }
        let inode = fs.lookup(name)?;
        Some(Arc::new(ext4::Ext4File::new(Arc::clone(fs), inode)))
    } else {
        open_file(name, flags).map(|i| i as Arc<dyn File + Send + Sync>)
    }
}

/// 根目录是否存在某文件
/// 把相对路径解析成 EXT4/easy-fs 里的实际路径（拼上进程 cwd）
pub fn resolve_path(name: &str) -> String {
    let name = name.trim_start_matches("./");
    if name.starts_with('/') {
        return String::from(name.trim_start_matches('/'));
    }
    let cwd = crate::task::current_process().inner_exclusive_access().cwd.clone();
    if cwd.is_empty() || cwd == "/" {
        String::from(name)
    } else {
        let mut s = cwd;
        s.push('/');
        s.push_str(name);
        s
    }
}

pub fn exists(name: &str) -> bool {
    if overlay_get(name).is_some() {
        return true;
    }
    if let Some(fs) = EXT4.as_ref() {
        fs.lookup(name).is_some()
    } else {
        open_file(name, OpenFlags::RDONLY).is_some()
    }
}

/// 根目录下的名字列表
pub fn names() -> Vec<String> {
    if let Some(fs) = EXT4.as_ref() {
        match fs.inode(2) {
            Some(root) => fs.list_dir(&root).into_iter().map(|(n, _, _)| n).collect(),
            None => Vec::new(),
        }
    } else {
        root_names()
    }
}
pub use stdio::{Stdin, Stdout};

/// 路径是否是目录
pub fn is_dir_path(name: &str) -> bool {
    let name = name.trim_start_matches("./").trim_start_matches('/');
    if name.is_empty() || name == "." {
        return true;
    }
    if let Some(fs) = EXT4.as_ref() {
        return fs.lookup(name).map(|i| i.is_dir()).unwrap_or(false);
    }
    false
}

/// 列出某个目录下的条目（名字, inode, 类型）
pub fn list_dir_path(name: &str) -> Vec<(String, u32, u8)> {
    let name = name.trim_start_matches("./").trim_start_matches('/');
    if let Some(fs) = EXT4.as_ref() {
        let inode = if name.is_empty() || name == "." {
            fs.inode(2)
        } else {
            fs.lookup(name)
        };
        if let Some(ino) = inode {
            return fs.list_dir(&ino);
        }
    }
    // easy-fs：只有扁平根目录
    root_names()
        .into_iter()
        .enumerate()
        .map(|(i, n)| (n, (i + 1) as u32, 1u8))
        .collect()
}

/// 在 open_dyn 里处理根目录等特殊路径
pub fn is_root_path(name: &str) -> bool {
    let n = name.trim_start_matches("./").trim_start_matches('/');
    n.is_empty() || n == "."
}
