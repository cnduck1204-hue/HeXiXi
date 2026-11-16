//! 极简 EXT4 探测：只读取超级块，用于识别官方评测盘。
//!
//! EXT4 超级块固定在字节偏移 1024 处（即 512 字节扇区的第 2 块）。

use crate::drivers::block::BLOCK_DEVICE;

pub const EXT4_MAGIC: u16 = 0xEF53;

/// EXT4 磁盘上的关键元数据（目前只解析超级块里的几个字段）
pub struct SuperBlock {
    pub block_size: usize,
    pub inodes_count: u32,
    pub blocks_count: u32,
    pub first_data_block: u32,
    pub inode_size: usize,
    pub blocks_per_group: u32,
    pub inodes_per_group: u32,
    pub volume_name: [u8; 16],
}

fn read_le16(buf: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([buf[off], buf[off + 1]])
}
fn read_le32(buf: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]])
}

/// 读取并解析超级块；若不是 EXT4 返回 None
pub fn read_superblock() -> Option<SuperBlock> {
    let mut buf = [0u8; 512];
    BLOCK_DEVICE.read_block(2, &mut buf);
    if read_le16(&buf, 56) != EXT4_MAGIC {
        return None;
    }
    let log_block_size = read_le32(&buf, 24);
    let mut name = [0u8; 16];
    name.copy_from_slice(&buf[120..136]);
    Some(SuperBlock {
        block_size: 1024usize << log_block_size,
        inodes_count: read_le32(&buf, 0),
        blocks_count: read_le32(&buf, 4),
        first_data_block: read_le32(&buf, 20),
        inode_size: read_le16(&buf, 88) as usize,
        blocks_per_group: read_le32(&buf, 32),
        inodes_per_group: read_le32(&buf, 40),
        volume_name: name,
    })
}

/// 启动时探测：命中 EXT4 就打印一行摘要
pub fn probe() -> bool {
    match read_superblock() {
        Some(sb) => {
            let name = core::str::from_utf8(&sb.volume_name)
                .unwrap_or("?")
                .trim_end_matches('\u{0}');
            println!(
                "[ext4] found: block_size={} inodes={} blocks={} inode_size={} vol=\"{}\"",
                sb.block_size, sb.inodes_count, sb.blocks_count, sb.inode_size, name
            );
            true
        }
        None => {
            println!("[ext4] no ext4 superblock on this disk");
            false
        }
    }
}
