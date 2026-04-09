# Phase 2: Driver Service Implementation Plan

> **Goal:** Move VirtIO drivers out of kernel into user-space driver_server service

**Architecture:**
- driver_server (PID 2) runs as user-space process
- Kernel maps VirtIO-MMIO region into driver_server's address space during process creation
- Driver server handles interrupts via IPC notifications from kernel
- Provides block I/O to fs_server via IPC

**Tech Stack:** Rust (no_std), RISC-V Sv39, RustSBI, QEMU virt

---

## File Structure Changes

```
os/src/
├── drivers/
│   ├── mod.rs              # MODIFY: Remove virtio_blk, keep only init
│   ├── virtio/
│   │   ├── mod.rs          # MODIFY: Keep constants/types only
│   │   ├── virtio_blk.rs   # MOVE to user/src/driver/virtio_blk.rs
│   │   └── ...             # Move all virtio code out
│   └── interrupt.rs        # MODIFY: Add notify_driver() for IPC
│
├── syscall/
│   └── device.rs           # NEW: sys_device_read/write for MMIO access

user/src/
├── driver/
│   ├── mod.rs              # NEW: Driver service main
│   ├── virtio_blk.rs       # MOVE from os/src/drivers/
│   ├── virtio_net.rs       # MOVE from os/src/drivers/
│   └── mmio.rs             # NEW: MMIO access helpers
└── main.rs                 # MODIFY: Add driver_server binary
```

---

## Task 1: Create Device Syscalls (Kernel)

**Files:**
- Create: `os/src/syscall/device.rs` - Device MMIO access syscalls
- Modify: `os/src/syscall/mod.rs` - Register device syscalls
- Modify: `os/src/process/mod.rs` - Add `setup_driver_service()`

**Steps:**

1. Create `os/src/syscall/device.rs`:
```rust
//! Device access syscalls for driver services

use crate::process::get_current_pid;

/// Device MMIO syscall numbers
pub const DEVICE_READ: usize = 1100;
pub const DEVICE_WRITE: usize = 1101;
pub const DEVICE_INTERRUPT_ENABLE: usize = 1102;

/// sys_device_read - Read from device MMIO
/// a0 = device_id, a1 = offset, a2 = count (in bytes)
pub fn sys_device_read(device_id: usize, offset: usize, count: usize) -> isize {
    // Validate device_id and access permissions
    // For now: device_id 0 = VirtIO block, 1 = VirtIO net
    match device_id {
        0 => crate::drivers::virtio_blk::virtio_blk_read(offset, count),
        _ => -1,
    }
}

/// sys_device_write - Write to device MMIO  
/// a0 = device_id, a1 = offset, a2 = data ptr, a3 = count
pub fn sys_device_write(device_id: usize, offset: usize, data_ptr: usize, count: usize) -> isize {
    match device_id {
        0 => crate::drivers::virtio_blk::virtio_blk_write(offset, data_ptr, count),
        _ => -1,
    }
}

/// sys_device_interrupt_enable - Enable interrupt delivery to this process
/// a0 = device_id, a1 = interrupt_id
pub fn sys_device_interrupt_enable(device_id: usize, interrupt_id: usize) -> isize {
    let pid = get_current_pid();
    crate::drivers::interrupt::register_driver_interrupt(pid, device_id, interrupt_id);
    0
}
```

2. Modify `os/src/syscall/mod.rs`:
- Add `pub mod device;`
- Add syscall numbers 1100-1102 to `nr` module
- Add to match in `do_syscall()`:
```rust
1100 => device::sys_device_read(get_arg0(), get_arg1(), get_arg2()),
1101 => device::sys_device_write(get_arg0(), get_arg1(), get_arg2(), get_arg3()),
1102 => device::sys_device_interrupt_enable(get_arg0(), get_arg1()),
```

3. Add interrupt registration to `os/src/drivers/interrupt.rs`:
```rust
/// Register a driver process to receive interrupts
pub fn register_driver_interrupt(pid: Pid, device_id: usize, intr_id: usize) {
    // Store mapping: interrupt_id -> pid
    // When interrupt fires, kernel sends IPC to registered driver
}
```

---

## Task 2: Move VirtIO Code to User Space

**Files:**
- Create: `user/src/driver/mod.rs`
- Create: `user/src/driver/virtio_blk.rs` (copied from kernel)
- Create: `user/src/driver/virtio_net.rs` (copied from kernel)
- Create: `user/src/driver/mmio.rs`

**Steps:**

1. Create `user/src/driver/mod.rs`:
```rust
//! Driver service - runs as user-space process
//! 
//! Handles VirtIO device access for other services.

#![no_std]

pub mod virtio_blk;
pub mod mmio;

/// Driver service entry
pub fn driver_service_main() {
    // 1. Create endpoint for block I/O requests
    // 2. Register for device interrupts  
    // 3. Loop: receive IPC requests, process device I/O, send responses
}
```

2. Copy `virtio_blk.rs` from kernel and adapt:
- Remove kernel dependencies
- Use sys_device_read/write syscalls for MMIO access
- Keep same API but implement via syscalls

3. Create `user/src/driver/mmio.rs`:
```rust
//! MMIO access via syscalls

/// Read from MMIO region
pub fn mmio_read(offset: usize, count: usize) -> isize {
    unsafe {
        core::arch::asm!(
            "mv a0, {0}",
            "mv a1, {1}",
            "mv a2, {2}",
            "li a7, 1100",  // DEVICE_READ
            "ecall",
            out("a0") _,
            in(reg) 0usize,  // device_id
            in(reg) offset,
            in(reg) count,
        )
    }
}

/// Write to MMIO region
pub fn mmio_write(offset: usize, data: u32) -> isize {
    unsafe {
        core::arch::asm!(
            // Pass data via a2 (pointer to data)
            "mv a0, {0}",
            "mv a1, {1}",
            "mv a2, {2}",
            "li a7, 1101",  // DEVICE_WRITE
            "ecall",
            out("a0") _,
            in(reg) 0usize,
            in(reg) offset,
            in(reg) data,
        )
    }
}
```

---

## Task 3: Create Driver Service Binary

**Files:**
- Create: `user/src/driver.rs` - Driver service entry point
- Modify: `user/Cargo.toml` - Add driver binary

**Steps:**

1. Create `user/src/driver.rs`:
```rust
//! Driver service entry point

#![no_std]
#![no_main]

extern crate user_lib;

fn print(s: &str) {
    user_lib::print(s);
}

#[no_mangle]
pub extern "C" fn _start() {
    print("driver: VirtIO driver service starting\n");
    
    // In a full implementation:
    // 1. Enable interrupt delivery for our devices
    // 2. Create endpoint for I/O requests (port 2)
    // 3. Wait for fs_server to connect
    // 4. Handle block I/O requests via IPC
    
    print("driver: Placeholder - driver service\n");
    
    loop {
        unsafe { core::arch::asm!("wfi"); }
    }
}
```

2. Build and verify driver service compiles

---

## Task 4: Integrate Driver Service into Boot

**Files:**
- Modify: `os/src/process/mod.rs` - Spawn driver_server after init
- Modify: `os/src/boot.rs` - Set up PMP for driver service

**Steps:**

1. Modify boot to:
   - After creating init process, spawn driver_server
   - Configure PMP to allow driver_server access to VirtIO-MMIO region
   - Map VirtIO-MMIO into driver_server's address space

2. For now, keep existing boot flow working - driver_server will be spawned by init via IPC in Phase 3

---

## Task 5: Build and Test

**Steps:**

1. Full build:
```bash
cargo build -p os
cargo build -p user
```

2. Run in QEMU and verify:
- Kernel boots successfully
- Init process runs
- Driver service placeholder loads (conceptually)

---

## Key Design Decisions

### Why sys_device_read/write instead of direct MMIO mapping?
- Simpler for Phase 2 - no need to modify page table mapping
- Kernel retains control over device access
- Security: kernel can validate all device access

### Why not handle interrupts directly in driver service?
- RISC-V interrupts always trap to kernel (supervisor mode)
- Driver service would need kernel to deliver interrupt as IPC
- For Phase 2: just poll or block on recv()

### Future (Phase 3+):
- Kernel sends IPC to driver when interrupt fires
- Driver processes interrupt, completes I/O
- Sends response to fs_server

---

## Commit Strategy

- Task 1: "feat(syscall): Add device MMIO access syscalls"
- Task 2: "feat(driver): Move VirtIO to user-space driver module"
- Task 3: "feat(driver): Add driver service binary"
- Task 4: "feat(boot): Add driver service spawning (stub)"
- Task 5: "feat(phase2): Driver service Phase 2 complete"
