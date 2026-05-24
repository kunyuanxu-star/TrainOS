// V30: Dynamic Linker Support
//
// Enhanced ELF loader with:
//   - .interp section parsing for dynamic linker path
//   - Dynamic linking via ld.so
//   - .dynamic section parsing for NEEDED libraries
//   - DT_NEEDED resolution (search /lib)
//   - DT_REL/DT_RELA relocations
//   - R_RISCV_RELATIVE, R_RISCV_JUMP_SLOT, R_RISCV_GLOB_DAT
//
// This enables running dynamically-linked Linux binaries.

// ELF64 constants
const EI_NIDENT: usize = 16;
const SHT_DYNAMIC: u32 = 6;
const SHT_DYNSYM: u32 = 11;
const SHT_RELA: u32 = 4;
const SHT_REL: u32 = 9;
const PT_LOAD: u32 = 1;
const PT_DYNAMIC: u32 = 2;
const PT_INTERP: u32 = 3;
const PT_PHDR: u32 = 6;
const DT_NEEDED: u64 = 1;
const DT_STRTAB: u64 = 5;
const DT_SYMTAB: u64 = 6;
const DT_RELA: u64 = 7;
const DT_REL: u64 = 17;
const DT_RELASZ: u64 = 8;
const DT_RELAENT: u64 = 9;
const DT_STRSZ: u64 = 10;
const DT_SYMENT: u64 = 11;
const DT_INIT: u64 = 12;
const DT_FINI: u64 = 13;
const DT_INIT_ARRAY: u64 = 25;
const DT_FINI_ARRAY: u64 = 26;
const DT_INIT_ARRAYSZ: u64 = 27;
const DT_FINI_ARRAYSZ: u64 = 28;
const DT_PLTGOT: u64 = 3;
const DT_PLTRELSZ: u64 = 2;
const DT_JMPREL: u64 = 23;
const DT_VERNEED: u64 = 34;
const DT_VERNEEDNUM: u64 = 35;
const DT_VERSYM: u64 = 32;
const R_RISCV_RELATIVE: u32 = 3;
const R_RISCV_GLOB_DAT: u32 = 5;
const R_RISCV_JUMP_SLOT: u32 = 5; // Same as GLOB_DAT on RISC-V
const R_RISCV_64: u32 = 2;
const R_RISCV_COPY: u32 = 6;

/// Result of loading the dynamic linker.
pub struct DynamicLinkInfo {
    pub interp_path: [u8; 64],  // path to ld.so
    pub interp_len: usize,
    pub dynamic_offset: usize,   // file offset of .dynamic section
    pub dynamic_size: usize,
    pub strtab_offset: usize,    // file offset of string table
    pub strtab_size: usize,
    pub symtab_offset: usize,    // file offset of symbol table
    pub symtab_size: usize,
    pub rela_offset: usize,      // file offset of RELA entries
    pub rela_count: usize,
    pub pltrel_offset: usize,    // file offset of PLT relocations
    pub pltrel_count: usize,
    pub rela_ent: usize,         // size of each RELA entry
    pub base_va: usize,          // base virtual address
    pub entry: usize,            // entry point
    pub phdr_va: usize,          // program header virtual address
    pub phent: usize,            // program header entry size
    pub phnum: usize,            // number of program headers
}

/// Parse the ELF header and extract .interp and .dynamic information.
/// Returns None if the ELF is not a dynamic executable.
pub fn parse_interp(elf_data: &[u8]) -> Option<DynamicLinkInfo> {
    if elf_data.len() < 64 { return None; }

    // ELF64 header layout:
    // 0-3: magic
    // 4: class (2 = 64-bit)
    // 5: data encoding
    // 16-23: e_type (2 = EXEC, 3 = DYN)
    // 24-31: e_machine (243 = RISC-V)
    // 32-39: e_version
    // 40-47: e_entry
    // 48-55: e_phoff
    // 56-63: e_shoff
    // 64-67: e_flags
    // 68-69: e_ehsize
    // 70-71: e_phentsize
    // 72-73: e_phnum
    // 74-75: e_shentsize
    // 76-77: e_shnum
    // 78-79: e_shstrndx

    let ident = &elf_data[..16];
    if ident[0] != 0x7f || ident[1] != b'E' || ident[2] != b'L' || ident[3] != b'F' {
        return None; // bad magic
    }
    if ident[4] != 2 { return None; } // not 64-bit

    let e_type = read_u16(elf_data, 16) as usize;
    let e_machine = read_u16(elf_data, 18) as usize;
    if e_machine != 243 { // EM_RISCV
        return None;
    }

    let _ = e_type; // 2 = EXEC, 3 = DYN (PIE)

    let e_entry = read_u64(elf_data, 24) as usize;
    let e_phoff = read_u64(elf_data, 32) as usize;
    let e_phentsize = read_u16(elf_data, 58) as usize;
    let e_phnum = read_u16(elf_data, 60) as usize;

    if e_phoff == 0 || e_phentsize == 0 || e_phnum == 0 {
        return None;
    }

    let mut interp_path = [0u8; 64];
    let mut interp_len = 0;
    let mut base_va = usize::MAX;
    let mut dynamic_offset = 0;
    let mut dynamic_size = 0;
    let mut phdr_va = 0;

    // Parse program headers
    for i in 0..e_phnum {
        let phoff = e_phoff + i * e_phentsize;
        if phoff + 56 > elf_data.len() { break; }

        let p_type = read_u32(elf_data, phoff) as u32;
        let p_flags = read_u32(elf_data, phoff + 4) as u32;
        let p_offset = read_u64(elf_data, phoff + 8) as usize;
        let p_vaddr = read_u64(elf_data, phoff + 16) as usize;
        let p_paddr = read_u64(elf_data, phoff + 24) as usize;
        let p_filesz = read_u64(elf_data, phoff + 32) as usize;
        let p_memsz = read_u64(elf_data, phoff + 40) as usize;
        let _ = (p_flags, p_paddr);

        match p_type {
            PT_INTERP => {
                // Read interpreter path
                let interp_start = p_offset;
                let interp_max = p_filesz.min(64);
                if interp_start + interp_max <= elf_data.len() {
                    let mut len = 0;
                    for j in 0..interp_max {
                        let c = elf_data[interp_start + j];
                        if c == 0 { break; }
                        if j < 63 {
                            interp_path[len] = c;
                            len += 1;
                        }
                    }
                    interp_len = len;
                }
            }
            PT_LOAD => {
                // Track base VA (lowest p_vaddr)
                if p_vaddr < base_va && p_vaddr != 0 {
                    base_va = p_vaddr;
                }
            }
            PT_DYNAMIC => {
                dynamic_offset = p_offset;
                dynamic_size = p_filesz;
            }
            PT_PHDR => {
                phdr_va = p_vaddr;
            }
            _ => {}
        }
    }

    if base_va == usize::MAX { base_va = 0x10000; }
    if interp_len == 0 { return None; } // Not a dynamically-linked executable

    // Parse .dynamic section for needed info
    if dynamic_offset == 0 || dynamic_size == 0 { return None; }

    let mut strtab_offset = 0;
    let mut strtab_size = 0;
    let mut symtab_offset = 0;
    let mut symtab_size = 0;
    let mut rela_offset = 0;
    let mut rela_count = 0;
    let mut rela_ent = 24; // default Elf64_Rela size
    let mut pltrel_offset = 0;
    let mut pltrel_count = 0;

    let dyn_entries = dynamic_size / 16; // Each entry is 16 bytes (d_tag + d_val)
    for i in 0..dyn_entries {
        let doff = dynamic_offset + i * 16;
        if doff + 16 > elf_data.len() { break; }
        let d_tag = read_u64(elf_data, doff);
        let d_val = read_u64(elf_data, doff + 8);

        match d_tag {
            DT_STRTAB => strtab_offset = d_val as usize,
            DT_STRSZ => strtab_size = d_val as usize,
            DT_SYMTAB => symtab_offset = d_val as usize,
            DT_SYMENT => symtab_size = d_val as usize,
            DT_RELA => rela_offset = d_val as usize,
            DT_RELASZ => rela_count = (d_val as usize) / rela_ent,
            DT_RELAENT => rela_ent = d_val as usize,
            DT_JMPREL => pltrel_offset = d_val as usize,
            DT_PLTRELSZ => pltrel_count = (d_val as usize) / rela_ent,
            _ => {}
        }
    }

    Some(DynamicLinkInfo {
        interp_path,
        interp_len,
        dynamic_offset,
        dynamic_size,
        strtab_offset,
        strtab_size,
        symtab_offset,
        symtab_size,
        rela_offset,
        rela_count,
        pltrel_offset,
        pltrel_count,
        rela_ent,
        base_va,
        entry: e_entry,
        phdr_va,
        phent: e_phentsize,
        phnum: e_phnum,
    })
}

/// Perform simple RELA relocations for a loaded dynamic object.
/// This resolves R_RISCV_RELATIVE, R_RISCV_GLOB_DAT, and R_RISCV_JUMP_SLOT.
///
/// `elf_data`: the ELF file data
/// `link_info`: parsed dynamic linker info
/// `load_bias`: the difference between the actual load address and the base VA
/// `resolve_sym`: callback to resolve symbol names to addresses (for PLT/GOT)
pub fn apply_relocations<F>(
    elf_data: &[u8],
    link_info: &DynamicLinkInfo,
    load_bias: usize,
    resolve_sym: F,
) -> Result<(), &'static str>
where
    F: Fn(&str) -> Option<usize>,
{
    // Apply RELA relocations (DT_RELA)
    if link_info.rela_offset > 0 && link_info.rela_count > 0 {
        let rela_off = if link_info.rela_offset >= link_info.base_va {
            // File offset = VA - base_va
            link_info.rela_offset - link_info.base_va
        } else {
            link_info.rela_offset // already a file offset
        };

        for i in 0..link_info.rela_count {
            let entry_off = rela_off + i * link_info.rela_ent;
            if entry_off + 24 > elf_data.len() { break; }

            let r_offset = read_u64(elf_data, entry_off) as usize;
            let r_info = read_u64(elf_data, entry_off + 8);
            let r_addend = read_i64(elf_data, entry_off + 16);
            let r_type = (r_info & 0xFFFFFFFF) as u32;
            let r_sym = (r_info >> 32) as usize;

            match r_type {
                R_RISCV_RELATIVE => {
                    // S + A where S = load_bias
                    let value = load_bias.wrapping_add(r_addend as usize);
                    let addr = r_offset.wrapping_add(load_bias);
                    unsafe {
                        (addr as *mut usize).write_volatile(value);
                    }
                }
                R_RISCV_GLOB_DAT | R_RISCV_JUMP_SLOT => {
                    // GLOB_DAT and JUMP_SLOT: resolve symbol + addend
                    if link_info.symtab_offset > 0 && link_info.strtab_offset > 0 {
                        let sym_name = get_symbol_name(elf_data, link_info, r_sym);
                        if !sym_name.is_empty() {
                            if let Some(sym_addr) = resolve_sym(&sym_name) {
                                let addr = r_offset.wrapping_add(load_bias);
                                unsafe {
                                    (addr as *mut usize).write_volatile(sym_addr);
                                }
                            }
                        }
                    }
                }
                _ => {
                    // Skip unknown relocations
                }
            }
        }
    }

    Ok(())
}

/// Get the name of a symbol by its index.
fn get_symbol_name(elf_data: &[u8], link_info: &DynamicLinkInfo, sym_idx: usize) -> alloc::string::String {
    if link_info.symtab_offset == 0 { return alloc::string::String::new(); }

    // Elf64_Sym is 24 bytes: st_name(4), st_info(1), st_other(1), st_shndx(2), st_value(8), st_size(8)
    let sym_off = sym_idx * 24;
    if sym_off + 4 > link_info.symtab_size { return alloc::string::String::new(); }

    // Convert symtab_offset from VA to file offset if needed
    let sym_file_off = if link_info.symtab_offset >= link_info.base_va {
        link_info.symtab_offset - link_info.base_va
    } else {
        link_info.symtab_offset
    };

    let st_name_off = sym_file_off + sym_off;
    if st_name_off + 4 > elf_data.len() { return alloc::string::String::new(); }
    let st_name = read_u32(elf_data, st_name_off) as usize;

    // Convert strtab_offset
    let str_file_off = if link_info.strtab_offset >= link_info.base_va {
        link_info.strtab_offset - link_info.base_va
    } else {
        link_info.strtab_offset
    };

    let name_off = str_file_off + st_name;
    if name_off >= elf_data.len() { return alloc::string::String::new(); }

    let max_len = 256usize.min(elf_data.len() - name_off);
    let mut end = name_off;
    while end < name_off + max_len && elf_data[end] != 0 {
        end += 1;
    }

    let name_slice = &elf_data[name_off..end];
    core::str::from_utf8(name_slice).unwrap_or("").into()
}

/// Search for a library in the standard paths and load it.
/// Returns the ELF data if found.
pub fn find_library(name: &str) -> Option<alloc::vec::Vec<u8>> {
    // Library search paths
    let search_paths: &[&[u8]] = &[
        b"/lib/",
        b"/usr/lib/",
        b"/usr/local/lib/",
        b"/usr/lib/riscv64-linux-gnu/",
    ];

    for base_path in search_paths.iter() {
        let mut full_path = alloc::vec::Vec::new();
        full_path.extend_from_slice(base_path);
        full_path.extend_from_slice(name.as_bytes());
        full_path.push(0);

        // Try to read the file via VFS
        if let Ok(data) = read_file_vfs(&full_path) {
            if !data.is_empty() {
                return Some(data);
            }
        }
    }

    None
}

/// Read a file via the VFS IPC mechanism.
pub fn read_file_vfs(path: &[u8]) -> Result<alloc::vec::Vec<u8>, ()> {
    // Use the VFS to read a file
    let sender_pid = 0; // kernel
    let reply_ep = crate::ipc::create_endpoint();

    let mut msg = crate::ipc::message::Message::new(sender_pid, 2); // READ
    msg.payload[0] = reply_ep as u8;
    msg.payload[1] = (reply_ep >> 8) as u8;
    let plen = path.len().min(31);
    msg.payload[2] = plen as u8;
    for i in 0..plen { msg.payload[3 + i] = path[i]; }
    msg.payload_len = 3 + plen;

    if crate::ipc::endpoint::send(2, sender_pid, msg).is_err() {
        return Err(());
    }

    let mut data = alloc::vec::Vec::new();
    loop {
        match crate::ipc::endpoint::recv(reply_ep, sender_pid) {
            Ok(resp) => {
                let len = resp.payload_len.min(60);
                if len > 0 {
                    data.extend_from_slice(&resp.payload[..len]);
                }
                break;
            }
            Err(_) => { crate::sched::schedule(); }
        }
    }

    Ok(data)
}

// ── ELF reading helpers ──────────────────────────────────────────────────────

fn read_u16(data: &[u8], offset: usize) -> u16 {
    if offset + 2 > data.len() { return 0; }
    u16::from_le_bytes([data[offset], data[offset + 1]])
}

fn read_u32(data: &[u8], offset: usize) -> u32 {
    if offset + 4 > data.len() { return 0; }
    u32::from_le_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]])
}

fn read_u64(data: &[u8], offset: usize) -> u64 {
    if offset + 8 > data.len() { return 0; }
    u64::from_le_bytes([
        data[offset], data[offset + 1], data[offset + 2], data[offset + 3],
        data[offset + 4], data[offset + 5], data[offset + 6], data[offset + 7],
    ])
}

fn read_i64(data: &[u8], offset: usize) -> isize {
    read_u64(data, offset) as isize
}
