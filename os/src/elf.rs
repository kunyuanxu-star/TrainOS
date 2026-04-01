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

/// ELF header (64-bit)
#[repr(C)]
pub struct Elf64Header {
    pub e_ident: [u8; 16],     // Magic number and other info
    pub e_type: u16,          // Object file type
    pub e_machine: u16,        // Architecture
    pub e_version: u32,       // Object file version
    pub e_entry: u64,         // Entry point virtual address
    pub e_phoff: u64,         // Program header table file offset
    pub e_shoff: u64,         // Section header table file offset
    pub e_flags: u32,         // Processor-specific flags
    pub e_ehsize: u16,         // ELF header size
    pub e_phentsize: u16,     // Program header table entry size
    pub e_phnum: u16,         // Program header table entry count
    pub e_shentsize: u16,      // Section header table entry size
    pub e_shnum: u16,          // Section header table entry count
    pub e_shstrndx: u16,       // Section header string table index
}

/// ELF program header (64-bit)
#[repr(C)]
pub struct Elf64Phdr {
    pub p_type: u32,           // Segment type
    pub p_flags: u32,           // Segment flags
    pub p_offset: u64,         // Segment file offset
    pub p_vaddr: u64,          // Segment virtual address
    pub p_paddr: u64,          // Segment physical address
    pub p_filesz: u64,         // Segment size in file
    pub p_memsz: u64,          // Segment size in memory
    pub p_align: u64,          // Segment alignment
}

/// ELF section header (64-bit)
#[repr(C)]
pub struct Elf64Shdr {
    pub sh_name: u32,           // Section name (string tbl index)
    pub sh_type: u32,           // Section type
    pub sh_flags: u64,         // Section flags
    pub sh_addr: u64,          // Section virtual addr at execution
    pub sh_offset: u64,         // Section file offset
    pub sh_size: u64,          // Section size in file
    pub sh_link: u32,           // Link to another section
    pub sh_info: u32,           // Additional section information
    pub sh_addralign: u64,      // Section alignment
    pub sh_entsize: u64,       // Entry size if section holds table
}

/// ELF symbol
#[repr(C)]
pub struct Elf64Sym {
    pub st_name: u32,           // Symbol name (string tbl index)
    pub st_info: u8,           // Symbol type and binding
    pub st_other: u8,           // Symbol visibility
    pub st_shndx: u16,         // Section index
    pub st_value: u64,          // Symbol value
    pub st_size: u64,          // Symbol size
}

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

    // Check machine type
    let header: &Elf64Header = unsafe { &*(data.as_ptr() as *const Elf64Header) };
    if header.e_machine != EM_RISCV {
        crate::println!("[elf] Wrong machine type");
        return ElfResult::Unsupported;
    }

    ElfResult::Success
}

/// Load an ELF file into user address space using Sv39 page tables
/// Returns (entry_point, user_sp, satp) on success
pub fn load_elf(data: &[u8], user_space: &mut crate::memory::Sv39::UserAddressSpace) -> Result<(usize, usize), ElfResult> {
    // Returns (entry_point, user_sp)
    if data.len() < core::mem::size_of::<Elf64Header>() {
        return Err(ElfResult::InvalidFormat);
    }

    let header: &Elf64Header = unsafe { &*(data.as_ptr() as *const Elf64Header) };

    // Validate
    match validate_elf(data) {
        ElfResult::Success => {}
        e => return Err(e),
    }

    crate::print!("[elf] Loading ELF\r\n");

    // Only support ET_EXEC (executable)
    if header.e_type != ET_EXEC {
        crate::print!("[elf] Only ET_EXEC supported\r\n");
        return Err(ElfResult::Unsupported);
    }

    // Get program headers
    let phdr_ptr = unsafe { data.as_ptr().add(header.e_phoff as usize) };
    let phdr_size = header.e_phentsize as usize;
    let phdr_count = header.e_phnum as usize;

    // Load each PT_LOAD segment
    for i in 0..phdr_count {
        let phdr: &Elf64Phdr = unsafe { &*(phdr_ptr.add(i * phdr_size) as *const Elf64Phdr) };

        if phdr.p_type == PT_LOAD {
            let file_offset = phdr.p_offset as usize;
            let vaddr = phdr.p_vaddr as usize;
            let filesz = phdr.p_filesz as usize;
            let memsz = phdr.p_memsz as usize;
            let flags = phdr.p_flags;

            // Align vaddr to page boundary
            let page_start = vaddr & !0xFFF;
            let page_end = ((vaddr + memsz) + 4095) & !0xFFF;
            let num_pages = (page_end - page_start) / 4096;

            crate::print!("[elf] Loading segment\r\n");

            // Map each page into user address space
            for p in 0..num_pages {
                let curr_vaddr = page_start + p * 4096;

                // Allocate physical page
                if let Some(phys_page) = crate::memory::allocator::alloc_page() {
                    // Determine page flags based on segment flags
                    let executable = (flags & PF_X) != 0;
                    let writable = (flags & PF_W) != 0;

                    // Map as user page (RX for code, RW for data)
                    if executable {
                        if user_space.map_user_rx(
                            crate::memory::Sv39::VirtAddr::new(curr_vaddr),
                            crate::memory::Sv39::PhysAddr::new(phys_page)
                        ).is_err() {
                            crate::print!("[elf] Failed to map page\r\n");
                            return Err(ElfResult::LoadError);
                        }
                    } else if writable {
                        if user_space.map_user_writable(
                            crate::memory::Sv39::VirtAddr::new(curr_vaddr),
                            crate::memory::Sv39::PhysAddr::new(phys_page)
                        ).is_err() {
                            crate::print!("[elf] Failed to map page\r\n");
                            return Err(ElfResult::LoadError);
                        }
                    } else {
                        if user_space.map_user_cow(
                            crate::memory::Sv39::VirtAddr::new(curr_vaddr),
                            crate::memory::Sv39::PhysAddr::new(phys_page)
                        ).is_err() {
                            crate::print!("[elf] Failed to map page\r\n");
                            return Err(ElfResult::LoadError);
                        }
                    }

                    // Copy data to the page
                    let kernel_va = 0x80000000 + phys_page;
                    let dst = kernel_va as *mut u8;

                    // Calculate what to copy
                    let offset_in_seg = if p == 0 { vaddr - page_start } else { 0 };
                    let file_pos = file_offset + offset_in_seg;
                    let copy_len = if p == 0 {
                        filesz.min(4096 - offset_in_seg)
                    } else if file_offset + p * 4096 < filesz + file_offset {
                        4096
                    } else {
                        0
                    };

                    if copy_len > 0 && file_pos < data.len() {
                        let src = unsafe { data.as_ptr().add(file_pos) };
                        unsafe {
                            core::ptr::copy_nonoverlapping(src, dst, copy_len);
                            // Zero BSS portion if any
                            if copy_len < 4096 {
                                core::ptr::write_bytes(dst.add(copy_len), 0, 4096 - copy_len);
                            }
                        }
                    } else {
                        // BSS - zero the page
                        unsafe {
                            core::ptr::write_bytes(dst, 0, 4096);
                        }
                    }
                } else {
                    crate::print!("[elf] Out of memory\r\n");
                    return Err(ElfResult::LoadError);
                }
            }
        }
    }

    // Set up user stack
    let stack_top = user_space.setup_user_stack()
        .map_err(|_| ElfResult::LoadError)?;

    crate::print!("[elf] Loaded successfully\r\n");
    Ok((header.e_entry as usize, stack_top - 16))
}

/// Get the entry point of an ELF file without loading it
pub fn get_entry_point(data: &[u8]) -> Option<usize> {
    if data.len() < core::mem::size_of::<Elf64Header>() {
        return None;
    }
    let header: &Elf64Header = unsafe { &*(data.as_ptr() as *const Elf64Header) };
    Some(header.e_entry as usize)
}

/// Get the number of program headers
pub fn get_phdr_count(data: &[u8]) -> Option<usize> {
    if data.len() < core::mem::size_of::<Elf64Header>() {
        return None;
    }
    let header: &Elf64Header = unsafe { &*(data.as_ptr() as *const Elf64Header) };
    Some(header.e_phnum as usize)
}

/// Parse ELF symbols (for debugging)
pub fn parse_symbols(data: &[u8]) {
    let header: &Elf64Header = unsafe { &*(data.as_ptr() as *const Elf64Header) };

    if header.e_shnum == 0 {
        return;
    }

    let shdr_ptr = unsafe { data.as_ptr().add(header.e_shoff as usize) };
    let shdr_size = header.e_shentsize as usize;

    // Find string table and symbol table
    for i in 0..header.e_shnum as usize {
        let shdr: &Elf64Shdr = unsafe { &*(shdr_ptr.add(i * shdr_size) as *const Elf64Shdr) };

        if shdr.sh_type == SHT_SYMTAB {
            crate::println!("[elf] Symbol table found");
        }
    }
}
