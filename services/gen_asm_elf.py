#!/usr/bin/env python3
"""Generate a minimal RISC-V 64-bit ELF binary with an assembly program.

The program prints "ASM program running!\r\n" via SBI ecall and then loops.
"""

import struct
import os

# ── Helper: RISC-V instruction encoding ──────────────────────────────────

def encode_i_type(opcode, funct3, rd, rs1, imm12):
    """I-type instruction encoding."""
    imm12 &= 0xFFF
    return (imm12 << 20) | (rs1 << 15) | (funct3 << 12) | (rd << 7) | opcode

def encode_u_type(opcode, rd, imm20):
    """U-type instruction encoding."""
    imm20 &= 0xFFFFF
    return (imm20 << 12) | (rd << 7) | opcode

def encode_b_type(funct3, rs1, rs2, offset):
    """B-type instruction encoding (BEQ, BNE, etc.).
    offset is a signed PC-relative byte offset (must be even).
    """
    assert offset % 2 == 0, "B-type offset must be even"
    imm = offset & 0x1FFF  # 13 bits
    # imm layout: imm[12|10:5|4:1|11]
    encoded = 0
    encoded |= ((imm >> 12) & 1) << 31  # imm[12]
    encoded |= ((imm >> 5) & 0x3F) << 25  # imm[10:5]
    encoded |= (rs2 & 0x1F) << 20
    encoded |= (rs1 & 0x1F) << 15
    encoded |= (funct3 & 0x7) << 12
    encoded |= ((imm >> 1) & 0xF) << 8  # imm[4:1]
    encoded |= ((imm >> 11) & 1) << 7  # imm[11]
    encoded |= 0x63  # BRANCH opcode
    return encoded

def encode_j_type(rd, offset):
    """J-type instruction encoding (JAL).
    offset is a signed PC-relative byte offset (must be even).
    """
    assert offset % 2 == 0, "J-type offset must be even"
    # Bits: imm[20|10:1|11|19:12] | rd | 1101111
    imm = offset & 0x1FFFFF  # 21 bits
    encoded = 0
    encoded |= ((imm >> 20) & 1) << 31  # imm[20]
    encoded |= ((imm >> 1) & 0x3FF) << 21  # imm[10:1]
    encoded |= ((imm >> 11) & 1) << 20  # imm[11]
    encoded |= ((imm >> 12) & 0xFF) << 12  # imm[19:12]
    encoded |= (rd & 0x1F) << 7
    encoded |= 0x6F  # JAL opcode
    return encoded

# ── Build the assembly program ──────────────────────────────────────────

msg_bytes = b"\r\nASM program running!\r\n\0"
msg_offset_from_start = 40  # after 10 instructions = 40 bytes

# _start at 0x10000
instructions = []

# inst 0: auipc a1, 0  -> a1 = PC (address of this instruction)
# auipc is U-type: opcode=0x17, rd=x11
instructions.append(encode_u_type(0x17, 11, 0))

# inst 1: addi a1, a1, msg_offset_from_start
# addi is I-type: opcode=0x13, funct3=0, rd=x11, rs1=x11
instructions.append(encode_i_type(0x13, 0, 11, 11, msg_offset_from_start))

# inst 2 (loop): lbu a0, 0(a1)  -- load char
# lbu is I-type (LOAD): opcode=0x03, funct3=4, rd=x10, rs1=x11
instructions.append(encode_i_type(0x03, 4, 10, 11, 0))

# inst 3: beqz a0, done (offset to done label)
# beqz a0, offset -> BEQ a0, x0, offset: funct3=0, rs1=x10, rs2=x0
# done is at instruction 8 (wfi), which is at offset 8*4=32 from _start
# This beqz is at PC = _start + 3*4 = _start + 12
# Offset = (_start + 8*4) - (_start + 3*4) = 20
instructions.append(encode_b_type(0, 10, 0, 20))

# inst 4: li a7, 1 -> addi a7, x0, 1
# I-type: opcode=0x13, funct3=0, rd=x17, rs1=x0
instructions.append(encode_i_type(0x13, 0, 17, 0, 1))

# inst 5: ecall
# I-type: opcode=0x73, funct3=0, rd=0, rs1=0, imm12=0
instructions.append(encode_i_type(0x73, 0, 0, 0, 0))

# inst 6: addi a1, a1, 1
# I-type: opcode=0x13, funct3=0, rd=x11, rs1=x11
instructions.append(encode_i_type(0x13, 0, 11, 11, 1))

# inst 7: j loop (back to inst 2)
# jal x0, offset: J-type, rd=x0
# This jal is at PC = _start + 7*4 = _start + 28
# Target = _start + 2*4 = _start + 8
# Offset = 8 - 28 = -20
instructions.append(encode_j_type(0, -20))

# inst 8 (done): wfi
# wfi is I-type: opcode=0x73, funct3=0, rd=0, rs1=0, imm12=0x105
instructions.append(encode_i_type(0x73, 0, 0, 0, 0x105))

# inst 9: j done (infinite loop back to wfi)
# This jal is at PC = _start + 9*4 = _start + 36
# Target = _start + 8*4 = _start + 32
# Offset = 32 - 36 = -4
instructions.append(encode_j_type(0, -4))

# ── Verification (print disassembly) ────────────────────────────────────
code_bytes = b"".join(struct.pack("<I", inst) for inst in instructions)
print(f"Code size: {len(code_bytes)} bytes ({len(code_bytes)//4} instructions)")
print(f"Message: {msg_bytes!r}")
print(f"Total binary payload: {len(code_bytes) + len(msg_bytes)} bytes")

# ── Build ELF file ──────────────────────────────────────────────────────

# ELF header (64 bytes for ELF64)
e_ident = b"\x7fELF"  # magic
e_ident += struct.pack("B", 2)   # ELFCLASS64
e_ident += struct.pack("B", 1)   # ELFDATA2LSB
e_ident += struct.pack("B", 1)   # EV_CURRENT
e_ident += struct.pack("B", 0)   # ELFOSABI_SYSV
e_ident += b"\x00" * 7           # padding

e_type = 2         # ET_EXEC
e_machine = 243    # EM_RISCV
e_version = 1
e_entry = 0x10000
e_phoff = 64       # program header right after ELF header
e_shoff = 0        # no section headers
e_flags = 0
e_ehsize = 64
e_phentsize = 56   # sizeof(Elf64_Phdr)
e_phnum = 1
e_shentsize = 64
e_shnum = 0
e_shstrndx = 0

elf_header = struct.pack(
    "<16sHHIQQQIHHHHHH",
    e_ident, e_type, e_machine, e_version,
    e_entry, e_phoff, e_shoff, e_flags,
    e_ehsize, e_phentsize, e_phnum,
    e_shentsize, e_shnum, e_shstrndx
)

# Program header (56 bytes) - PT_LOAD
payload_offset = 64 + 56  # after ELF header + program header
p_type = 1       # PT_LOAD
p_flags = 5      # PF_R | PF_X (read + execute)
p_offset = payload_offset
p_vaddr = 0x10000
p_paddr = 0x10000
p_filesz = len(code_bytes) + len(msg_bytes)
p_memsz = len(code_bytes) + len(msg_bytes)
p_align = 0x1000

phdr = struct.pack(
    "<IIQQQQQQ",
    p_type, p_flags, p_offset, p_vaddr,
    p_paddr, p_filesz, p_memsz, p_align
)

# Combine everything
elf_data = elf_header + phdr + code_bytes + msg_bytes

# ── Verify ELF ───────────────────────────────────────────────────────────
# Check header
assert elf_data[:4] == b"\x7fELF", "Bad ELF magic"
# Check program header location (e_phoff at offset 32, u64)
phdr_offset = struct.unpack("<Q", elf_data[32:40])[0]
assert phdr_offset == 64, f"Bad phoff: {phdr_offset}"
# Check entry
entry = struct.unpack("<Q", elf_data[24:32])[0]
assert entry == 0x10000, f"Bad entry: {entry:#x}"
# Check machine
machine = struct.unpack("<H", elf_data[18:20])[0]
assert machine == 243, f"Bad machine: {machine}"

print(f"\nELF binary size: {len(elf_data)} bytes")
print("ELF verification passed!")

# ── Write file ──────────────────────────────────────────────────────────
os.makedirs("/home/xukunyuan/code/AI4OSE/testOS/TrainOS/services/test_c", exist_ok=True)
out_path = "/home/xukunyuan/code/AI4OSE/testOS/TrainOS/services/test_c/hello.elf"
with open(out_path, "wb") as f:
    f.write(elf_data)

dest_path = "/home/xukunyuan/code/AI4OSE/testOS/TrainOS/kernel/src/test_c.elf"
with open(dest_path, "wb") as f:
    f.write(elf_data)

print(f"\nWritten to: {out_path}")
print(f"Copied to:  {dest_path}")
