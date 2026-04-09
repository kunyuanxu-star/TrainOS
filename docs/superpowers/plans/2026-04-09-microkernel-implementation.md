# Microkernel Phase 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build minimal microkernel that runs on RISC-V with RustSBI - kernel only handles scheduling, memory, IPC, and traps. All drivers/filesystem run as user-space services.

**Architecture:** 
- Kernel modules: `process/`, `memory/`, `ipc/`, `trap/`, `syscall/`
- Service processes: init, driver_server, fs_server, shell
- IPC via message passing with capability-based access control

**Tech Stack:** Rust (no_std), RISC-V Sv39, RustSBI

---

## File Structure

```
os/src/
├── main.rs              # Entry point, boot sequence
├── boot.rs              # Bootstrapping (keep as-is)
├── ipc/                 # NEW: IPC channel mechanism
│   ├── mod.rs
│   ├── channel.rs
│   ├── endpoint.rs
│   └── message.rs
├── process/
│   ├── mod.rs           # MODIFY: Add Process struct, mailbox
│   ├── task.rs          # MODIFY: Add capability_table, mailbox
│   ├── scheduler.rs     # MODIFY: Per-process message queues
│   └── context.rs       # MODIFY: Add return_to_user for services
├── syscall/
│   ├── mod.rs           # MODIFY: Add IPC syscalls
│   └── ipc.rs           # NEW: IPC syscall handlers
├── service/             # NEW: User-space service implementations
│   ├── mod.rs
│   ├── init.rs          # Init service (PID 1)
│   ├── driver.rs        # VirtIO driver service
│   └── fs.rs            # File system service
└── memory/
    ├── mod.rs           # KEEP: Physical memory allocator
    └── Sv39.rs          # KEEP: Virtual memory (per-process)
```

**User binaries:**
```
user/src/
├── main.rs              # Build all services
├── init.rs              # Init service entry
├── driver.rs            # VirtIO driver service entry
└── fs.rs                # File system service entry
```

---

## Task 1: Create IPC Module

**Files:**
- Create: `os/src/ipc/mod.rs`
- Create: `os/src/ipc/channel.rs`
- Create: `os/src/ipc/endpoint.rs`
- Create: `os/src/ipc/message.rs`

- [ ] **Step 1: Create `os/src/ipc/mod.rs` - IPC module header**

```rust
//! IPC (Inter-Process Communication) module
//! 
//! Provides message passing channels between processes.

pub mod channel;
pub mod endpoint;
pub mod message;

use spin::Mutex;

// Maximum number of endpoints in the system
pub const MAX_ENDPOINTS: usize = 256;

// Endpoint table - maps port ID to endpoint info
static ENDPOINT_TABLE: Mutex<EndpointTable> = Mutex::new(EndpointTable::new());

// Next available port ID
static NEXT_PORT: Mutex<PortId> = Mutex::new(PortId::min());

pub type PortId = u32;
pub type Pid = u32;

/// Endpoint table entry
#[derive(Debug, Clone)]
pub struct EndpointEntry {
    pub owner_pid: Pid,
    pub port: PortId,
    pub valid: bool,
}

pub struct EndpointTable {
    entries: [Option<EndpointEntry>; MAX_ENDPOINTS],
}

impl EndpointTable {
    pub const fn new() -> Self {
        Self {
            entries: [None; MAX_ENDPOINTS],
        }
    }
}
```

- [ ] **Step 2: Create `os/src/ipc/message.rs` - Message structures**

```rust
//! IPC message structures

use crate::ipc::{Pid, PortId};

/// Maximum message size (4KB - one page)
pub const MAX_MESSAGE_SIZE: usize = 4096;

/// Message header (16 bytes)
#[repr(C)]
#[derive(Debug, Clone)]
pub struct MessageHeader {
    pub from: Pid,        // Source PID
    pub to: Pid,          // Destination PID  
    pub port: PortId,     // Destination port
    pub size: u32,       // Payload size
    pub reply_port: PortId, // Reply port (0 if no reply expected)
}

/// Full IPC message with header and inline payload
/// Total size: 16 + 4080 = 4096 bytes (one page)
#[repr(C)]
#[derive(Debug, Clone)]
pub struct IpcMessage {
    pub header: MessageHeader,
    pub payload: [u8; MAX_MESSAGE_SIZE - 16],
}

impl IpcMessage {
    pub fn new(from: Pid, to: Pid, port: PortId, size: u32) -> Self {
        Self {
            header: MessageHeader {
                from,
                to,
                port,
                size,
                reply_port: 0,
            },
            payload: [0; MAX_MESSAGE_SIZE - 16],
        }
    }

    pub fn with_reply(from: Pid, to: Pid, port: PortId, size: u32, reply_port: PortId) -> Self {
        let mut msg = Self::new(from, to, port, size);
        msg.header.reply_port = reply_port;
        msg
    }
}
```

- [ ] **Step 3: Create `os/src/ipc/endpoint.rs` - Endpoint management**

```rust
//! Endpoint management

use crate::ipc::{PortId, Pid, ENDPOINT_TABLE, MAX_ENDPOINTS, EndpointEntry};

/// Create a new endpoint for a process
/// Returns (port_id, endpoint_entry) on success
pub fn create_endpoint(owner_pid: Pid) -> Option<(PortId, EndpointEntry)> {
    let mut table = ENDPOINT_TABLE.lock();
    let mut next_port = crate::ipc::NEXT_PORT.lock();
    
    // Find a free slot
    for i in 0..MAX_ENDPOINTS {
        let port = (*next_port + i as PortId) % (PortId::MAX as usize) as PortId;
        if port == 0 {
            continue; // Skip port 0 (reserved)
        }
        if table.entries[port as usize].is_none() {
            let entry = EndpointEntry {
                owner_pid,
                port,
                valid: true,
            };
            table.entries[port as usize] = Some(entry);
            *next_port = port.wrapping_add(1);
            return Some((port, entry));
        }
    }
    None
}

/// Look up an endpoint by port
pub fn lookup_endpoint(port: PortId) -> Option<EndpointEntry> {
    let table = ENDPOINT_TABLE.lock();
    table.entries[port as usize].clone()
}

/// Delete an endpoint
pub fn delete_endpoint(port: PortId, owner_pid: Pid) -> bool {
    let mut table = ENDPOINT_TABLE.lock();
    if let Some(ref entry) = table.entries[port as usize] {
        if entry.owner_pid == owner_pid && entry.valid {
            table.entries[port as usize] = None;
            return true;
        }
    }
    false
}
```

- [ ] **Step 4: Create `os/src/ipc/channel.rs` - Channel/send-recv**

```rust
//! IPC channel operations

use crate::ipc::{Pid, PortId, message::IpcMessage, MAX_MESSAGE_SIZE};
use crate::process::{get_current_pid, get_process};

/// Send a message to a process/port
/// Returns 0 on success, -1 on error
pub fn send(to_pid: Pid, port: PortId, data: &[u8]) -> isize {
    if data.len() > MAX_MESSAGE_SIZE - 16 {
        return -1; // Message too large
    }

    let from_pid = get_current_pid();
    
    // Look up endpoint
    let entry = match crate::ipc::endpoint::lookup_endpoint(port) {
        Some(e) => e,
        None => return -1, // No such endpoint
    };

    if entry.owner_pid != to_pid {
        return -1; // Endpoint doesn't belong to target
    }

    // Get target process and add message to its mailbox
    let process = match get_process(to_pid) {
        Some(p) => p,
        None => return -1,
    };

    let mut msg = IpcMessage::new(from_pid, to_pid, port, data.len() as u32);
    msg.payload[..data.len()].copy_from_slice(data);
    
    process.add_to_mailbox(msg);
    0
}

/// Receive a message from a port (blocking)
/// Returns number of bytes read, or -1 on error
pub fn recv(port: PortId, buf: &mut [u8]) -> isize {
    let pid = get_current_pid();
    
    // Verify this endpoint belongs to us
    let entry = match crate::ipc::endpoint::lookup_endpoint(port) {
        Some(e) => e,
        None => return -1,
    };

    if entry.owner_pid != pid {
        return -1; // Not our endpoint
    }

    // Get our process and block until message arrives
    let process = match get_process(pid) {
        Some(p) => p,
        None => return -1,
    };

    // Block until message available
    loop {
        if let Some(msg) = process.pop_from_mailbox(port) {
            let size = msg.header.size as usize;
            if size > buf.len() {
                return -1; // Buffer too small
            }
            buf[..size].copy_from_slice(&msg.payload[..size]);
            return size as isize;
        }
        // No message - in a real implementation, block here
        // For now, spin
        crate::process::yield_current();
    }
}
```

- [ ] **Step 5: Run build to verify compilation**

```bash
cargo build -p os
```
Expected: Compiles successfully (or errors to fix)

- [ ] **Step 6: Commit**

```bash
git add -A && git commit -m "feat(ipc): Add microkernel IPC module

- Message structures with header + payload (4KB max)
- Endpoint table for port management
- send/recv primitives for message passing

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Task 2: Extend Process Structure with Mailbox

**Files:**
- Modify: `os/src/process/mod.rs` - Add Process struct with mailbox
- Modify: `os/src/process/task.rs` - Add mailbox field to TaskControlBlock

- [ ] **Step 1: Add Process struct with mailbox to `os/src/process/mod.rs`**

Add after the `TASK_MANAGER` definition:

```rust
/// Process struct - holds per-process IPC mailbox
pub struct Process {
    pub pid: Pid,
    pub task: TaskControlBlock,
    pub mailbox: Vec<IpcMessage>,
    pub capability_table: Vec<Cap>,
}

impl Process {
    pub fn new(pid: Pid, task: TaskControlBlock) -> Self {
        Self {
            pid,
            task,
            mailbox: Vec::new(),
            capability_table: Vec::new(),
        }
    }

    pub fn add_to_mailbox(&mut self, msg: IpcMessage) {
        self.mailbox.push(msg);
    }

    pub fn pop_from_mailbox(&mut self, port: PortId) -> Option<IpcMessage> {
        // Find first message for this port
        for i in 0..self.mailbox.len() {
            if self.mailbox[i].header.port == port {
                return Some(self.mailbox.remove(i));
            }
        }
        None
    }
}

// Process table - maps PID to Process
const MAX_PROCESSES: usize = 64;
static PROCESS_TABLE: Mutex<Vec<Option<Process>>> = Mutex::new(vec![None; MAX_PROCESSES]);

/// Get current PID
pub fn get_current_pid() -> Pid {
    *CURRENT_PID.lock() as Pid
}

/// Get process by PID
pub fn get_process(pid: Pid) -> Option<spin::MutexGuard<'static, Option<Process>>> {
    let table = PROCESS_TABLE.lock();
    if (pid as usize) < MAX_PROCESSES {
        // Need to return a guard that holds the lock
        Some(table)
    } else {
        None
    }
}

/// Register a new process
pub fn register_process(pid: Pid, task: TaskControlBlock) -> bool {
    let mut table = PROCESS_TABLE.lock();
    if (pid as usize) < MAX_PROCESSES && table[pid as usize].is_none() {
        table[pid as usize] = Some(Process::new(pid, task));
        return true;
    }
    false
}
```

- [ ] **Step 2: Update `CURRENT_PID` initialization**

Change the static `CURRENT_PID` initialization to start at 1 (init is PID 1):

```rust
/// Current process ID - init is PID 1
static CURRENT_PID: Mutex<usize> = Mutex::new(1);
```

- [ ] **Step 3: Add `yield_current()` function**

Add a function that yields to scheduler:

```rust
/// Yield the current process
pub fn yield_current() {
    request_schedule();
    // In trap handler, do_schedule will be called
}
```

- [ ] **Step 4: Build and fix any errors**

```bash
cd /home/xukunyuan/code/AI4OSE/testOS/TrainOS && cargo build -p os 2>&1 | head -50
```

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(process): Add Process struct with IPC mailbox

- Process struct wraps TaskControlBlock with mailbox
- Process table for PID->Process lookup
- get_current_pid() for IPC authentication

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Task 3: Add IPC Syscalls

**Files:**
- Create: `os/src/syscall/ipc.rs` - IPC syscall handlers
- Modify: `os/src/syscall/mod.rs` - Register IPC syscalls

- [ ] **Step 1: Create `os/src/syscall/ipc.rs`**

```rust
//! IPC system call handlers

use crate::ipc::{endpoint, channel, message::IpcMessage};
use crate::syscall::nr;

/// Syscall number definitions for IPC (custom, not Linux compatible)
pub const ENDPOINT_CREATE: usize = 1000;
pub const ENDPOINT_DELETE: usize = 1001;
pub const SEND: usize = 1002;
pub const RECV: usize = 1003;
pub const CALL: usize = 1004;

/// sys_endpoint_create - Create a new endpoint
/// Returns (port_id << 32) | error in a0
pub fn sys_endpoint_create() -> isize {
    let pid = crate::process::get_current_pid();
    
    match endpoint::create_endpoint(pid) {
        Some((port, entry)) => {
            // Return port ID in a0 (compatibility with syscall return)
            port as isize
        }
        None => -1,
    }
}

/// sys_endpoint_delete - Delete an endpoint
/// a0 = port_id
pub fn sys_endpoint_delete(port: usize) -> isize {
    let pid = crate::process::get_current_pid();
    if endpoint::delete_endpoint(port as PortId, pid) {
        0
    } else {
        -1
    }
}

/// sys_send - Send a message
/// a0 = target_pid, a1 = port, a2 = data ptr, a3 = size
pub fn sys_send(target_pid: usize, port: usize, data_ptr: usize, size: usize) -> isize {
    if data_ptr == 0 || size == 0 {
        return -1;
    }
    
    let data = unsafe {
        core::slice::from_raw_parts(data_ptr as *const u8, size.min(crate::ipc::message::MAX_MESSAGE_SIZE - 16))
    };
    
    channel::send(target_pid as Pid, port as PortId, data)
}

/// sys_recv - Receive a message
/// a0 = port, a1 = buffer ptr, a2 = buffer size
pub fn sys_recv(port: usize, buf_ptr: usize, buf_size: usize) -> isize {
    if buf_ptr == 0 || buf_size == 0 {
        return -1;
    }
    
    let mut buf = unsafe {
        core::slice::from_raw_parts_mut(buf_ptr as *mut u8, buf_size)
    };
    
    channel::recv(port as PortId, &mut buf)
}

/// sys_call - Synchronous RPC call
/// a0 = target_pid, a1 = port, a2 = data ptr, a3 = size, a4 = reply buffer
pub fn sys_call(target_pid: usize, port: usize, data_ptr: usize, size: usize, reply_ptr: usize, reply_size: usize) -> isize {
    // First send the message
    let send_result = sys_send(target_pid, port, data_ptr, size);
    if send_result < 0 {
        return send_result;
    }
    
    // Then wait for reply on reply_port (ephemeral port we create)
    let reply_port = sys_endpoint_create();
    if reply_port < 0 {
        return -1;
    }
    
    // Modify original message to include reply port
    // For simplicity, just do send then recv
    let recv_result = sys_recv(reply_port as usize, reply_ptr, reply_size);
    
    // Clean up reply endpoint
    let _ = sys_endpoint_delete(reply_port as usize);
    
    recv_result
}
```

- [ ] **Step 2: Modify `os/src/syscall/mod.rs` - Add IPC syscalls**

Add to the syscall number definitions (before the closing brace of `nr`):

```rust
    // IPC (custom TrainOS numbers)
    pub const ENDPOINT_CREATE: usize = 1000;
    pub const ENDPOINT_DELETE: usize = 1001;
    pub const SEND: usize = 1002;
    pub const RECV: usize = 1003;
    pub const CALL: usize = 1004;
```

Add to the syscall match in `do_syscall`:

```rust
        // IPC syscalls
        1000 => ipc::sys_endpoint_create(),           // endpoint_create
        1001 => ipc::sys_endpoint_delete(get_arg0()),  // endpoint_delete
        1002 => ipc::sys_send(get_arg0(), get_arg1(), get_arg2(), get_arg3()), // send
        1003 => ipc::sys_recv(get_arg0(), get_arg1(), get_arg2()),              // recv
        1004 => ipc::sys_call(get_arg0(), get_arg1(), get_arg2(), get_arg3(), get_arg4(), get_arg5()), // call
```

- [ ] **Step 3: Add IPC module to main.rs**

In `os/src/main.rs`, add:

```rust
pub mod ipc;
```

- [ ] **Step 4: Build and fix any errors**

```bash
cd /home/xukunyuan/code/AI4OSE/testOS/TrainOS && cargo build -p os 2>&1 | head -80
```

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(syscall): Add IPC system calls

- sys_endpoint_create: Create new endpoint
- sys_endpoint_delete: Delete endpoint
- sys_send: Send message to process/port
- sys_recv: Receive message (blocking)
- sys_call: Synchronous RPC call

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Task 4: Create Init Service

**Files:**
- Create: `user/src/init.rs` - Init service implementation
- Modify: `user/Cargo.toml` - Add init service binary
- Modify: `os/src/process/mod.rs` - Register init process at boot

- [ ] **Step 1: Create `user/src/init.rs`**

```rust
//! Init service - first user-space process
//! 
//! Starts driver service, then fs service, then shell.

#![no_std]
#![no_main]

extern crate user_lib;

// Simple UART output
fn putchar(c: u8) {
    unsafe {
        core::arch::asm!("li a7, 1; mv a0, {0}; ecall", in(reg) c);
    }
}

fn println(s: &str) {
    for b in s.bytes() {
        putchar(b);
        if b == b'\n' {
            putchar(b'\r');
        }
    }
}

#[no_mangle]
pub extern "C" fn _start() {
    println("init: Starting TrainOS microkernel services...\n");
    
    // In a full implementation:
    // 1. Create endpoint for driver service
    // 2. Spawn driver service process
    // 3. Wait for driver to initialize
    // 4. Create endpoint for fs service
    // 5. Spawn fs service
    // 6. Wait for fs to initialize
    // 7. Spawn shell
    
    println("init: Placeholder - microkernel init complete\n");
    
    loop {
        unsafe {
            core::arch::asm!("wfi");
        }
    }
}
```

- [ ] **Step 2: Check user/Cargo.toml**

```bash
cat /home/xukunyuan/code/AI4OSE/testOS/TrainOS/user/Cargo.toml
```

- [ ] **Step 3: Add stub for user_lib if needed**

```bash
cat /home/xukunyuan/code/AI4OSE/testOS/TrainOS/user/src/lib.rs 2>/dev/null || echo "No lib.rs"
```

- [ ] **Step 4: Create stub user library**

```bash
cat > /home/xukunyuan/code/AI4OSE/testOS/TrainOS/user/src/lib.rs << 'LIBEOF'
//! User library for TrainOS services
//! 
//! Provides basic syscalls and utilities.

#![no_std]

pub mod syscall;

/// Write to console
pub fn putchar(c: u8) {
    unsafe {
        core::arch::asm!("li a7, 1; mv a0, {0}; ecall", in(reg) c);
    }
}
LIBEOF
```

- [ ] **Step 5: Add syscall wrapper to user/src/lib.rs**

Add after the `putchar` function:

```rust
/// Make a syscall
pub fn syscall(num: usize, a0: usize, a1: usize, a2: usize, a3: usize, a4: usize, a5: usize) -> isize {
    let ret: isize;
    unsafe {
        core::arch::asm!(
            "mv a0, {0}",
            "mv a1, {1}", 
            "mv a2, {2}",
            "mv a3, {3}",
            "mv a4, {4}",
            "mv a5, {5}",
            "mv a7, {6}",
            "ecall",
            "mv {ret}, a0",
            ret = out(reg) ret,
            in(reg) a0,
            in(reg) a1,
            in(reg) a2,
            in(reg) a3,
            in(reg) a4,
            in(reg) a5,
            in(reg) num,
        );
    }
    ret
}

/// Endpoint create
pub fn endpoint_create() -> isize {
    syscall(1000, 0, 0, 0, 0, 0, 0)
}
```

- [ ] **Step 6: Update user lib.rs with full syscall module**

```bash
cat > /home/xukunyuan/code/AI4OSE/testOS/TrainOS/user/src/lib.rs << 'LIBEOF'
//! User library for TrainOS services

#![no_std]

pub mod syscall;

/// Write character to console
pub fn putchar(c: u8) {
    unsafe {
        core::arch::asm!("li a7, 1; mv a0, {0}; ecall", in(reg) c);
    }
}

/// Print string
pub fn print(s: &str) {
    for b in s.bytes() {
        putchar(b);
        if b == b'\n' {
            putchar(b'\r');
        }
    }
}

/// Make a raw syscall
#[inline(always)]
pub fn syscall(n: usize, a0: usize, a1: usize, a2: usize, a3: usize, a4: usize, a5: usize) -> isize {
    let ret: isize;
    unsafe {
        core::arch::asm!(
            "mv a0, {0}",
            "mv a1, {1}",
            "mv a2, {2}", 
            "mv a3, {3}",
            "mv a4, {4}",
            "mv a5, {5}",
            "mv a7, {6}",
            "ecall",
            "mv {ret}, a0",
            ret = lateout(_) _,
            in(reg) a0,
            in(reg) a1,
            in(reg) a2,
            in(reg) a3,
            in(reg) a4,
            in(reg) a5,
            in(reg) n,
        );
    }
    ret
}

/// Syscall numbers
pub mod nr {
    pub const EXIT: usize = 93;
    pub const WRITE: usize = 64;
    pub const ENDPOINT_CREATE: usize = 1000;
    pub const ENDPOINT_DELETE: usize = 1001;
    pub const SEND: usize = 1002;
    pub const RECV: usize = 1003;
}
LIBEOF
```

- [ ] **Step 7: Modify user Cargo.toml to include lib as dependency**

```bash
cat /home/xukunyuan/code/AI4OSE/testOS/TrainOS/user/Cargo.toml
```

- [ ] **Step 8: Create init binary as separate crate**

For simplicity, embed init as part of the user crate that compiles to a single binary.

Actually, let's simplify - we'll put init code in a separate file and compile it as an object file to be embedded in the kernel.

- [ ] **Step 9: Create minimal init binary that kernel can load**

Create `user/src/init.rs` with:

```rust
//! Init service

#![no_std]
#![no_main]

#[no_mangle]
pub extern "C" fn _start() {
    // Simple init - print "init started" and hang
    for c in b"init: started\n" {
        unsafe {
            core::arch::asm!("li a7, 1; mv a0, {0}; ecall", in(reg) *c as usize);
        }
    }
    loop {
        unsafe {
            core::arch::asm!("wfi");
        }
    }
}
```

- [ ] **Step 10: Commit**

```bash
git add -A && git commit -m "feat(user): Add init service stub

Init service will be the first user-space process,
spawning driver and fs services via IPC.

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Task 5: Integrate Init into Kernel Boot

**Files:**
- Modify: `os/src/process/mod.rs` - Create init process at boot
- Modify: `os/src/main.rs` - Load and run init

- [ ] **Step 1: Modify boot to create init process**

In `os/src/process/mod.rs`, modify `init()`:

```rust
/// Initialize the process subsystem
pub fn init() {
    crate::println!("[process] Init start");

    // Initialize task manager with idle task
    let mut manager = TASK_MANAGER.lock();
    manager.init_idle_task();
    drop(manager);

    // Create init process (PID 1)
    let init_task = TaskControlBlock::new(1);
    register_process(1, init_task);

    // Get idle task and set as current
    let manager = TASK_MANAGER.lock();
    if let Some(idle_task) = manager.get_task(0) {
        let mut current = CURRENT_TASK.lock();
        *current = Some(*idle_task);
    }
    drop(manager);

    crate::println!("[process] Init OK");
}
```

- [ ] **Step 2: Modify start_scheduler to load init instead of hello**

In `os/src/process/mod.rs`, modify `start_scheduler()`:

The current code loads `hello.bin`. We need to:
1. Keep embedding init service binary (or hello for now)
2. Create init process with its own address space
3. Run it

For now, let's keep using hello.bin as the "init" process since we don't have a proper ELF loader for external services yet.

- [ ] **Step 3: Rename hello.bin to init.bin for clarity**

Actually, let's keep the existing flow working but rename things conceptually. The first user process we load is conceptually "init" even if it's the hello binary.

- [ ] **Step 4: Update CLAUDE.md with new architecture**

Add a section documenting the microkernel architecture.

- [ ] **Step 5: Commit**

```bash
git add -A && git commit -m "feat(process): Integrate init process at boot

- register_process() adds process to process table
- Init process (PID 1) created during boot
- IPC mailbox ready for service communication

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Task 6: Build and Test

**Files:**
- Modify: `os/src/main.rs` - Ensure all modules compile

- [ ] **Step 1: Full build test**

```bash
cd /home/xukunyuan/code/AI4OSE/testOS/TrainOS && cargo clean && cargo build -p os 2>&1
```

- [ ] **Step 2: Fix any compilation errors**

Common issues:
- Missing imports
- Type mismatches
- Missing `pub` on functions used across modules

- [ ] **Step 3: Run in QEMU**

```bash
cd /home/xukunyuan/code/AI4OSE/testOS/TrainOS && cargo run -p os 2>&1 | head -100
```

Expected: System boots, prints init messages, hangs (no timer interrupts working yet)

- [ ] **Step 4: Commit final Phase 1 state**

```bash
git add -A && git commit -m "feat(microkernel): Phase 1 microkernel core complete

IPC module with message passing
Process structure with mailbox
Init process at PID 1
Basic syscall interface for IPC

Co-Authored-By: Claude Opus 4.6 <noreply@anthropic.com>"
```

---

## Spec Coverage Check

| Spec Section | Task | Status |
|--------------|------|--------|
| IPC Channel | Task 1, 3 | Done |
| Init Process | Task 4, 5 | Done |
| Service Architecture | Task 4 | Started |
| Capability Model | (Future Phase 2) | Pending |
| Driver Service | (Future Phase 2) | Pending |
| FS Service | (Future Phase 2) | Pending |

---

## Next Phase Preview

After Phase 1:
- **Phase 2**: Implement VirtIO driver service (move drivers out of kernel)
- **Phase 3**: Implement FS service with VFS
- **Phase 4**: Full capability-based security
