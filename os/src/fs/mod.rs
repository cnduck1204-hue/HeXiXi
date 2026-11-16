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
}

pub use inode::{is_unlinked, list_apps, mark_unlinked, open_file, root_names, OSInode, OpenFlags};
pub use pipe::{make_pipe, Pipe};

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

/// 读出整个文件（内核内部用，例如 exec 加载 ELF）
pub fn read_file_all(name: &str) -> Option<Vec<u8>> {
    if let Some(fs) = EXT4.as_ref() {
        let inode = fs.lookup(name)?;
        Some(fs.read_file(&inode))
    } else {
        open_file(name, OpenFlags::RDONLY).map(|i| i.read_all())
    }
}

/// 打开文件并返回统一的 File 对象（供系统调用使用）
pub fn open_dyn(name: &str, flags: OpenFlags) -> Option<Arc<dyn File + Send + Sync>> {
    if let Some(fs) = EXT4.as_ref() {
        let inode = fs.lookup(name)?;
        Some(Arc::new(ext4::Ext4File::new(Arc::clone(fs), inode)))
    } else {
        open_file(name, flags).map(|i| i as Arc<dyn File + Send + Sync>)
    }
}

/// 根目录是否存在某文件
pub fn exists(name: &str) -> bool {
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
