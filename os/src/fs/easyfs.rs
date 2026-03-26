//! EasyFS - A Simple File System
//!
//! This is a simplified version of a UNIX-like file system.
//! Block size: 512 bytes
//! Superblock, inode bitmap, block bitmap, inode table, data blocks

pub const BLOCK_SIZE: usize = 512;

/// Superblock - describes the file system layout
#[repr(C)]
pub struct Superblock {
    pub magic: u32,           // Magic number for validation
    pub total_blocks: u32,   // Total number of blocks
    pub inode_bitmap_start: u32,
    pub inode_bitmap_blocks: u32,
    pub inode_table_start: u32,
    pub inode_table_blocks: u32,
    pub data_bitmap_start: u32,
    pub data_bitmap_blocks: u32,
    pub data_blocks_start: u32,
    pub data_blocks: u32,
}

impl Superblock {
    pub fn new() -> Self {
        Self {
            magic: 0x1_BAD_F00D,
            total_blocks: 0,
            inode_bitmap_start: 0,
            inode_bitmap_blocks: 0,
            inode_table_start: 0,
            inode_table_blocks: 0,
            data_bitmap_start: 0,
            data_bitmap_blocks: 0,
            data_blocks_start: 0,
            data_blocks: 0,
        }
    }

    pub fn is_valid(&self) -> bool {
        self.magic == 0x1_BAD_F00D
    }
}

/// Inode types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum InodeType {
    File = 1,
    Directory = 2,
}

/// Inode - describes a file or directory
#[repr(C)]
pub struct Inode {
    pub inode_type: u16,      // Type (file or directory)
    pub size: u32,             // Size in bytes
    pub direct_blocks: [u32; 10],  // Direct data block pointers
    pub indirect_block: u32,   // Single indirect block pointer
    pub pad: [u8; 12],
}

impl Inode {
    pub fn new() -> Self {
        Self {
            inode_type: 0,
            size: 0,
            direct_blocks: [0; 10],
            indirect_block: 0,
            pad: [0; 12],
        }
    }

    pub fn is_valid(&self) -> bool {
        self.inode_type == InodeType::File as u16 ||
        self.inode_type == InodeType::Directory as u16
    }
}

/// Directory entry
#[repr(C)]
pub struct DirEntry {
    pub inode_num: u32,
    pub name: [u8; 28],
}

impl DirEntry {
    pub fn new() -> Self {
        Self {
            inode_num: 0,
            name: [0; 28],
        }
    }
}
