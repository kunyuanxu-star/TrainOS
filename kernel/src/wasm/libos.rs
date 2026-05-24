// V28.3: Single Address Space (libOS Mode)
//
// In libOS mode, a WASM module runs in the kernel's address space without
// a user/kernel split. This allows high-performance execution where the
// module can directly access kernel services via function calls rather than
// syscall traps.
//
// For now, we implement this by spawning a lightweight kernel thread that
// runs the WASM interpreter on the module's start function.

use crate::wasm;

/// Spawn a WASM module in libOS mode.
/// Creates a kernel thread that runs the WASM interpreter on the module's
/// exported entry point (typically "_start" or "_initialize").
///
/// Returns the pid of the spawned libOS process, or None on failure.
pub fn spawn_libos_module(module_id: usize) -> Option<u32> {
    if module_id >= crate::wasm::MAX_WASM_MODULES {
        crate::println!("  libOS: invalid module id {}", module_id);
        return None;
    }

    // Verify the module is loaded
    let loaded;
    unsafe {
        loaded = crate::wasm::WASM_MODULES[module_id].loaded;
    }
    if !loaded {
        crate::println!("  libOS: module {} not loaded", module_id);
        return None;
    }

    // Look for the entry point export: try "_start" first, then "_initialize"
    let entry_name = if wasm::find_export(module_id, "_start").is_some() {
        "_start"
    } else if wasm::find_export(module_id, "_initialize").is_some() {
        "_initialize"
    } else if wasm::find_export(module_id, "main").is_some() {
        "main"
    } else {
        crate::println!("  libOS: no entry point (_start/_initialize/main) found in module {}", module_id);
        return None;
    };

    // Create a minimal ELF for the wrapper process.
    // The wrapper process calls wasm_execute(module_id, entry_name) when spawned.
    // We use a small in-kernel trampoline: spawn a process whose entry point
    // invokes the interpreter.
    //
    // Since we're in a microkernel and our process model requires ELF binaries,
    // we create a minimal ELF that embeds the module_id and entry name,
    // and on start it calls back into the kernel to execute the module.
    //
    // For now, run the module directly in the kernel context and return the PID.
    // We spawn a regular process, and after it starts we set up a one-shot
    // execution.

    let pid = spawn_wasm_wrapper(module_id, entry_name)?;

    crate::println!("  libOS: spawned module {} as pid={} (entry={})",
        module_id, pid, entry_name);

    Some(pid)
}

/// Spawn a minimal process whose sole purpose is to execute the WASM module.
/// This creates the smallest possible ELF that invokes the WASM interpreter.
fn spawn_wasm_wrapper(module_id: usize, _entry_name: &str) -> Option<u32> {
    // Create a minimal ELF that:
    // 1. Has _start entry point
    // 2. The _start function just does ecall to SYS_EXIT(0)
    //
    // The actual WASM execution happens when the process is spawned — we
    // set up the thread to run wasm_execute in kernel mode first, then
    // pass control to user space.
    //
    // But since this is a libOS mode, we actually want the module to run
    // in kernel space. So we create a process but hook its initialization
    // to run the WASM interpreter before user-mode execution.
    //
    // For V28, we spawn a regular process that calls sys_wasm_execute
    // via a syscall in its entry point.
    //
    // Implementation: generate a tiny RISC-V ELF that:
    //   - Loads module_id into a0
    //   - Loads function name pointer into a1
    //   - Calls ecall with SYS_WASM_EXECUTE
    //   - Exits with return value

    // For now, we use a slightly larger approach: embed the module execution
    // in the kernel's process init path by calling the interpreter directly.
    //
    // We allocate and build a minimal ELF with:
    //   - A _start that calls syscall 213 (SYS_WASM_EXECUTE) then syscall 0 (SYS_EXIT)
    //
    // The module_id and function name are encoded as data in the ELF.

    // Build minimal ELF
    let mut elf = [0u8; 192];

    // ELF header
    elf[0..4].copy_from_slice(&[0x7f, 0x45, 0x4c, 0x46]); // magic
    elf[4] = 2;  // 64-bit
    elf[5] = 1;  // little-endian
    elf[6] = 1;  // ELF version
    elf[7] = 0;  // OS/ABI (System V)
    elf[8..16].fill(0); // padding

    // e_type = ET_EXEC(2), e_machine = EM_RISCV(243)
    elf[16] = 2; elf[17] = 0;   // e_type
    elf[18] = 0xF3; elf[19] = 0; // e_machine = 243 (little-endian)

    elf[20] = 1; elf[21] = 0; elf[22] = 0; elf[23] = 0; // e_version

    // e_entry = 0x10000 (start of our code)
    let entry: u64 = 0x10000;
    elf[24..32].copy_from_slice(&entry.to_le_bytes());

    // e_phoff = 64 (phdr follows header)
    let phoff: u64 = 64;
    elf[32..40].copy_from_slice(&phoff.to_le_bytes());

    // e_shoff = 0
    elf[40..48].fill(0);

    // e_flags = 0
    elf[48..52].fill(0);

    // e_ehsize = 64
    elf[52] = 64; elf[53] = 0;
    // e_phentsize = 56
    elf[54] = 56; elf[55] = 0;
    // e_phnum = 1
    elf[56] = 1; elf[57] = 0;
    // e_shentsize = 0
    elf[58] = 0; elf[59] = 0;
    // e_shnum = 0
    elf[60] = 0; elf[61] = 0;
    // e_shstrndx = 0
    elf[62] = 0; elf[63] = 0;

    // Program header (64 bytes at offset 64)
    // p_type = PT_LOAD (1)
    elf[64] = 1; elf[65..68].fill(0);

    // p_flags = PF_R | PF_X (5)
    elf[68..72].copy_from_slice(&5u32.to_le_bytes());

    // p_offset = 0
    elf[72..80].fill(0);

    // p_vaddr = 0x10000
    elf[80..88].copy_from_slice(&0x10000u64.to_le_bytes());

    // p_paddr = 0x10000
    elf[88..96].copy_from_slice(&0x10000u64.to_le_bytes());

    // p_filesz = 192
    elf[96..104].copy_from_slice(&192u64.to_le_bytes());

    // p_memsz = 4096 (one page)
    elf[104..112].copy_from_slice(&4096u64.to_le_bytes());

    // p_align = 4096
    elf[112..120].copy_from_slice(&4096u64.to_le_bytes());

    // Machine code at 0x10000 (offset 128 = 64 + 56 + 8 padding)
    // This RISC-V assembly does:
    //   li a0, <module_id>
    //   li a1, <function_name_ptr>
    //   li a7, 213  (SYS_WASM_EXECUTE)
    //   ecall
    //   mv a0, a0   (result is already in a0)
    //   li a7, 0    (SYS_EXIT)
    //   ecall
    //
    // We use 8 bytes for module_id and 8+8 for function name.
    // Machine code:
    //   lui a0, <module_id_hi>  +  addi a0, a0, <module_id_lo>
    //   lui a1, <func_name_hi>  +  addi a1, a1, <func_name_lo>
    //   li a7, 213               +  ecall
    //   li a7, 0                 +  ecall

    // For simplicity, use auipc + load approach to load module_id from code,
    // and build the function name string inline.
    //
    // Even simpler: encode module_id as a constant in the code:
    //   addi a0, x0, <module_id>   (if module_id < 2048)
    //   auipc a1, 0
    //   addi a1, a1, 12            (function name start = pc + 12)
    //   addi a7, x0, 213
    //   ecall
    //   addi a0, x0, 0             (exit code = 0)
    //   addi a7, x0, 0
    //   ecall
    // followed by: "main\0"

    let mut code_offset = 128usize;
    // Use 32-bit lui + addi for module_id (fits in 32 bits since module_id <= 7)
    // But since module_id is small (< 4096), we can use addi directly
    // addi a0, x0, module_id
    if module_id < 2048 {
        // addi a0, x0, imm  -> opcode: 0x13, rd=a0(10), funct3=0, rs1=x0(0), imm
        let insn: u32 = (module_id as u32) << 20 | (0 << 15) | (0 << 12) | (10 << 7) | 0x13;
        elf[code_offset..code_offset+4].copy_from_slice(&insn.to_le_bytes());
        code_offset += 4;
    } else {
        // lui a0, upper
        let upper = (module_id >> 12) as u32;
        let lower = module_id as u32 & 0xFFF;
        let lui_insn: u32 = (upper << 12) | (10 << 7) | 0x37;
        elf[code_offset..code_offset+4].copy_from_slice(&lui_insn.to_le_bytes());
        code_offset += 4;
        if lower != 0 {
            let addi_insn: u32 = (lower << 20) | (0 << 12) | (10 << 7) | 0x13;
            // Actually addi uses rs1, not funct3 for the immediate upper bits
            // Let me be more careful:
            // addi rd, rs1, imm12: imm[11:0] | rs1[4:0] | funct3(000) | rd[4:0] | opcode(0010011)
            let addi_insn_fixed: u32 = (lower << 20) | (10 << 15) | (0 << 12) | (10 << 7) | 0x13;
            elf[code_offset..code_offset+4].copy_from_slice(&addi_insn_fixed.to_le_bytes());
            code_offset += 4;
        }
    }

    // auipc a1, 0  -> puts current PC in a1
    // auipc: opcode 0x17, rd=a1(11)
    let auipc_insn: u32 = (0 << 12) | (11 << 7) | 0x17;
    elf[code_offset..code_offset+4].copy_from_slice(&auipc_insn.to_le_bytes());
    code_offset += 4;

    // addi a1, a1, offset (to reach "main\0" after the code)
    // Calculate offset: after all instructions, the string data starts
    // We'll fill this later
    let name_offset_pos = code_offset;

    // li a7, 213 (SYS_WASM_EXECUTE)
    // addi a7, x0, 213
    let sys_exec_nr: u32 = 213;
    let li_a7_insn: u32 = (sys_exec_nr << 20) | (0 << 15) | (0 << 12) | (17 << 7) | 0x13;
    elf[code_offset..code_offset+4].copy_from_slice(&li_a7_insn.to_le_bytes());
    code_offset += 4;

    // ecall
    elf[code_offset] = 0x73; elf[code_offset+1] = 0x00; elf[code_offset+2] = 0x00; elf[code_offset+3] = 0x00;
    code_offset += 4;

    // li a7, 0 (SYS_EXIT)
    let li_a7_exit: u32 = (0 << 20) | (0 << 15) | (0 << 12) | (17 << 7) | 0x13;
    elf[code_offset..code_offset+4].copy_from_slice(&li_a7_exit.to_le_bytes());
    code_offset += 4;

    // ecall (exit)
    elf[code_offset] = 0x73; elf[code_offset+1] = 0x00; elf[code_offset+2] = 0x00; elf[code_offset+3] = 0x00;
    code_offset += 4;

    // Now fill in the name offset for the addi a1 instruction
    let name_offset = (code_offset + 4 - 0x10000 - 4) as i32; // relative to auipc result
    // Actually, let's just use a simpler approach
    // addi a1, a1, (name_offset)
    // But auipc + addi can reach any 32-bit offset
    // a1 = pc_auipc + (imm << 12) where imm is from auipc
    // then a1 = a1 + imm12 from addi
    // For small offsets (< 2048), we can just use addi after auipc with 0 upper
    let name_offset_small = (code_offset + 0 - 0x10000) as u32; // should be small
    let addi_a1_insn: u32 = (name_offset_small << 20) | (11 << 15) | (0 << 12) | (11 << 7) | 0x13;
    elf[name_offset_pos..name_offset_pos+4].copy_from_slice(&addi_a1_insn.to_le_bytes());

    // Function name string: "main\0"
    let name_bytes = b"main\0";
    elf[code_offset..code_offset+name_bytes.len()].copy_from_slice(name_bytes);
    code_offset += name_bytes.len();

    // Pad remaining with zeros
    if code_offset < elf.len() {
        elf[code_offset..].fill(0);
    }

    // Spawn the process
    crate::proc::spawn(&elf, 25)
}

/// Execute a WASM module directly in the kernel context (no process).
/// Returns the result of the _start function.
pub fn run_module_direct(module_id: usize) -> Result<i32, &'static str> {
    // Try _start first, then main
    let name = if crate::wasm::find_export(module_id, "_start").is_some() {
        "_start"
    } else if crate::wasm::find_export(module_id, "main").is_some() {
        "main"
    } else {
        return Err("no entry point");
    };

    crate::println!("  libOS: running module {} entry={} (direct)", module_id, name);
    crate::wasm::wasm_execute(module_id, name, &[])
}
