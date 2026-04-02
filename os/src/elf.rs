//! ELF Binary Loader
//!
//! Loads and executes ELF binaries

/// ELF magic number
pub const ELF_MAGIC: [u8; 4] = [0x7F, b'E', b'L', b'F'];

/// ELF file class
pub const ELFCLASS32: u8 = 1;
pub const ELFCLASS64: u8 = 2;

/// ELF endianness
pub const ELFDATA2LSB: u8 = 1;  // Little endian
pub const ELFDATA2MSB: u8 = 2;  // Big endian

/// ELF type
pub const ET_NONE: u16 = 0;
pub const ET_REL: u16 = 1;     // Relocatable file
pub const ET_EXEC: u16 = 2;     // Executable file
pub const ET_DYN: u16 = 3;     // Shared object file
pub const ET_CORE: u16 = 4;    // Core file

/// ELF machine types
pub const EM_RISCV: u16 = 243;

/// ELF program header types
pub const PT_LOAD: u32 = 1;

/// ELF section header types
pub const SHT_NULL: u32 = 0;
pub const SHT_PROGBITS: u32 = 1;
pub const SHT_SYMTAB: u32 = 2;
pub const SHT_STRTAB: u32 = 3;

/// ELF segment flags
pub const PF_X: u32 = 1;  // Executable
pub const PF_W: u32 = 2;  // Writable
pub const PF_R: u32 = 4;  // Readable

/// ELF result
pub enum ElfResult {
    Success,
    InvalidFormat,
    Unsupported,
    LoadError,
}

/// Check if data is a valid ELF file
pub fn is_elf_file(data: &[u8]) -> bool {
    if data.len() < 4 {
        return false;
    }
    data[0..4] == ELF_MAGIC
}

/// Get ELF class (32 or 64 bit)
pub fn get_elf_class(data: &[u8]) -> Option<u8> {
    if data.len() < 5 {
        return None;
    }
    Some(data[4])
}

/// Get ELF endianness
pub fn get_elf_endian(data: &[u8]) -> Option<u8> {
    if data.len() < 6 {
        return None;
    }
    Some(data[5])
}

/// Validate ELF header for RISC-V
pub fn validate_elf(data: &[u8]) -> ElfResult {
    // Check magic
    if !is_elf_file(data) {
        return ElfResult::InvalidFormat;
    }

    // Check class
    if data[4] != ELFCLASS64 {
        crate::println!("[elf] Only 64-bit ELF supported");
        return ElfResult::Unsupported;
    }

    // Check endianness (little endian)
    if data[5] != ELFDATA2LSB {
        crate::println!("[elf] Only little-endian ELF supported");
        return ElfResult::Unsupported;
    }

    // Check version
    if data[6] != 1 {
        return ElfResult::InvalidFormat;
    }

    // Check machine type (e_machine at offset 18)
    if data[18] != (EM_RISCV as u8) || data[19] != (EM_RISCV >> 8) as u8 {
        crate::println!("[elf] Wrong machine type");
        return ElfResult::Unsupported;
    }

    ElfResult::Success
}

/// Read a u16 from data at offset (little endian)
unsafe fn read_u16(data: &[u8], offset: usize) -> u16 {
    let ptr = data.as_ptr().add(offset) as *const u16;
    ptr.read_unaligned()
}

/// Read a u32 from data at offset (little endian)
unsafe fn read_u32(data: &[u8], offset: usize) -> u32 {
    let ptr = data.as_ptr().add(offset) as *const u32;
    ptr.read_unaligned()
}

/// Read a u64 from data at offset (little endian)
unsafe fn read_u64(data: &[u8], offset: usize) -> u64 {
    let ptr = data.as_ptr().add(offset) as *const u64;
    ptr.read_unaligned()
}

/// Load an ELF file into user address space using kernel page table directly
/// This maps user pages into the kernel page table instead of creating a separate user page table.
/// Returns (entry_point, user_sp) on success
pub fn load_elf(data: &[u8], user_space: &mut crate::memory::Sv39::UserAddressSpace) -> Result<(usize, usize), ElfResult> {
    use crate::memory::Sv39::{VirtAddr, PhysAddr};

    // Validate ELF header
    match validate_elf(data) {
        ElfResult::Success => {}
        e => return Err(e),
    }

    // Read ELF header fields
    let e_entry = unsafe { read_u64(data, 24) } as usize;
    let e_phoff = unsafe { read_u64(data, 32) } as usize;
    let e_phentsize = unsafe { read_u16(data, 54) } as usize;
    let e_phnum = unsafe { read_u16(data, 56) } as usize;

    // Only load the FIRST PT_LOAD segment
    for i in 0..e_phnum {
        let phdr_offset = e_phoff + i * e_phentsize;
        let p_type = unsafe { read_u32(data, phdr_offset) };

        if p_type == PT_LOAD {
            let p_offset = unsafe { read_u64(data, phdr_offset + 8) } as usize;
            let p_vaddr = unsafe { read_u64(data, phdr_offset + 16) } as usize;
            let p_filesz = unsafe { read_u64(data, phdr_offset + 32) } as usize;
            let p_memsz = unsafe { read_u64(data, phdr_offset + 40) } as usize;
            let p_flags = unsafe { read_u32(data, phdr_offset + 4) };

            // Align vaddr to page boundary
            let page_start = p_vaddr & !0xFFF;
            let page_end = ((p_vaddr + p_memsz) + 4095) & !0xFFF;
            let num_pages = (page_end - page_start) / 4096;

            // Map each page into the user page table
            for p in 0..num_pages {
                let curr_vaddr = page_start + p * 4096;

                // Allocate physical page
                let phys_page = match crate::memory::allocator::alloc_page() {
                    Some(pp) => pp,
                    None => return Err(ElfResult::LoadError),
                };

                // Determine flags
                let executable = (p_flags & PF_X) != 0;
                let writable = (p_flags & PF_W) != 0;
                let result = if executable {
                    user_space.map_user_rx(VirtAddr::new(curr_vaddr), PhysAddr::new(phys_page))
                } else if writable {
                    user_space.map_user_writable(VirtAddr::new(curr_vaddr), PhysAddr::new(phys_page))
                } else {
                    user_space.map_user_cow(VirtAddr::new(curr_vaddr), PhysAddr::new(phys_page))
                };

                if result.is_err() {
                    return Err(ElfResult::LoadError);
                }

                // Copy data to the page
                let offset_in_seg = if curr_vaddr >= p_vaddr {
                    curr_vaddr - p_vaddr
                } else {
                    0
                };

                let file_pos = p_offset + offset_in_seg;
                let copy_len = if p_filesz > offset_in_seg {
                    (p_filesz - offset_in_seg).min(4096)
                } else {
                    0
                };

                if copy_len > 0 && file_pos < data.len() {
                    let dst = phys_page as *mut u8;
                    let src = unsafe { data.as_ptr().add(file_pos) };
                    unsafe {
                        core::ptr::copy_nonoverlapping(src, dst, copy_len);
                        if copy_len < 4096 {
                            core::ptr::write_bytes(dst.add(copy_len), 0, 4096 - copy_len);
                        }
                    }
                } else {
                    let dst = phys_page as *mut u8;
                    unsafe {
                        core::ptr::write_bytes(dst, 0, 4096);
                    }
                }
            }

            break; // Only load first segment
        }
    }

    // Set up user stack at a high VA
    // Use 256KB stack (64 pages) for reliability
    let stack_base = 0x3FFFFFFFE80;
    let stack_size = 0x40000; // 256KB stack
    let stack_top = stack_base + stack_size;

    // Map stack pages using user address space
    for i in 0..(stack_size / 4096) {
        let va = stack_top - (i + 1) * 4096;
        let phys_page = match crate::memory::allocator::alloc_page() {
            Some(p) => p,
            None => return Err(ElfResult::LoadError),
        };

        if user_space.map_user_writable(VirtAddr::new(va), PhysAddr::new(phys_page)).is_err() {
            return Err(ElfResult::LoadError);
        }
    }

    Ok((e_entry, stack_top - 16))
}

/// Get the entry point of an ELF file without loading it
pub fn get_entry_point(data: &[u8]) -> Option<usize> {
    if data.len() < 64 {
        return None;
    }
    Some(unsafe { read_u64(data, 24) } as usize)
}

/// Get the number of program headers
pub fn get_phdr_count(data: &[u8]) -> Option<usize> {
    if data.len() < 64 {
        return None;
    }
    Some(unsafe { read_u16(data, 56) } as usize)
}

/// Parse ELF symbols (for debugging)
pub fn parse_symbols(data: &[u8]) {
    if data.len() < 64 {
        return;
    }

    let e_shnum = unsafe { read_u16(data, 60) } as usize;
    if e_shnum == 0 {
        return;
    }
}