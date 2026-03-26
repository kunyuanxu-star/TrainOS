//! Sv39 Virtual Memory Management
//!
//! RISC-V Sv39 is a 3-level page table with 4KB pages.
//! - 9 bits per level (512 entries per page)
//! - 27 bits for VPN (3 levels)
//! - 44 bits for PPN
//! - 12 bits offset

/// Virtual Page Number
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct VPN(pub usize);

/// Physical Page Number
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PPN(pub usize);

/// Page table entry flags
#[derive(Debug, Clone, Copy)]
pub struct PTEFlags {
    pub valid: bool,      // V - Valid
    pub read: bool,       // R - Readable
    pub write: bool,      // W - Writable
    pub execute: bool,    // X - Executable
    pub user: bool,       // U - User accessible
    pub global: bool,     // G - Global mapping
    pub accessed: bool,   // A - Accessed
    pub dirty: bool,      // D - Dirty
}

impl PTEFlags {
    pub fn new() -> Self {
        Self {
            valid: false,
            read: false,
            write: false,
            execute: false,
            user: false,
            global: false,
            accessed: false,
            dirty: false,
        }
    }

    pub fn bits(&self) -> usize {
        let mut bits = 0usize;
        if self.valid    { bits |= 1 << 0; }
        if self.read     { bits |= 1 << 1; }
        if self.write    { bits |= 1 << 2; }
        if self.execute  { bits |= 1 << 3; }
        if self.user     { bits |= 1 << 4; }
        if self.global   { bits |= 1 << 5; }
        if self.accessed { bits |= 1 << 6; }
        if self.dirty    { bits |= 1 << 7; }
        bits
    }

    pub fn from_bits(bits: usize) -> Self {
        Self {
            valid:    (bits & (1 << 0)) != 0,
            read:     (bits & (1 << 1)) != 0,
            write:    (bits & (1 << 2)) != 0,
            execute:  (bits & (1 << 3)) != 0,
            user:     (bits & (1 << 4)) != 0,
            global:   (bits & (1 << 5)) != 0,
            accessed: (bits & (1 << 6)) != 0,
            dirty:    (bits & (1 << 7)) != 0,
        }
    }
}

/// Page Table Entry
#[derive(Debug, Clone, Copy)]
pub struct PageTableEntry {
    pub ppn: PPN,
    pub flags: PTEFlags,
}

impl PageTableEntry {
    pub fn new() -> Self {
        Self {
            ppn: PPN(0),
            flags: PTEFlags::new(),
        }
    }

    pub fn bits(&self) -> usize {
        self.ppn.0 << 10 | self.flags.bits()
    }

    pub fn from_bits(bits: usize) -> Self {
        Self {
            ppn: PPN(bits >> 10),
            flags: PTEFlags::from_bits(bits & 0xFF),
        }
    }
}

/// A page table page (512 entries)
pub const PAGE_SIZE: usize = 4096;
pub const PTE_COUNT: usize = 512;

/// Virtual address for Sv39
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct VirtAddr(pub usize);

/// Physical address
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PhysAddr(pub usize);

impl VirtAddr {
    pub fn page_offset(&self) -> usize {
        self.0 & 0xFFF
    }

    pub fn vpn(&self) -> VPN {
        VPN(self.0 >> 12)
    }

    pub fn as_ptr<T>(&self) -> *const T {
        self.0 as *const T
    }

    pub fn as_mut_ptr<T>(&self) -> *mut T {
        self.0 as *mut T
    }
}

impl PhysAddr {
    pub fn as_ptr<T>(&self) -> *const T {
        self.0 as *const T
    }

    pub fn as_mut_ptr<T>(&self) -> *mut T {
        self.0 as *mut T
    }
}

/// Extract VPN indices from a VPN
pub fn vpn_indices(vpn: VPN) -> [usize; 3] {
    let vpn_bits = vpn.0;
    [
        (vpn_bits >> 18) & 0x1FF,  // Level 0 (root)
        (vpn_bits >> 9) & 0x1FF,   // Level 1
        vpn_bits & 0x1FF,          // Level 2 (leaf)
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vpn_indices() {
        let vpn = VPN(0x12345);
        let indices = vpn_indices(vpn);
        assert_eq!(indices[0], 0);  // Top 9 bits
        assert_eq!(indices[1], 0x123);  // Middle 9 bits
        assert_eq!(indices[2], 0x345);  // Bottom 9 bits
    }
}
