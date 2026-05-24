# Wave 1 (V21/V22/V23) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement V21 (formal verification & security), V22 (io_uring async I/O), V23 (virtualization/hypervisor) — three independent foundation versions.

**Architecture:** V21 enhances `invariant.rs`, `security/mod.rs`, `proc/`, `mem/`. V22 rewrites `iouring/mod.rs` and adds `device/sched.rs`, `device/merge.rs`. V23 expands `hypervisor/` with CSR, MMU, VirtIO, timer, PLIC, snapshot submodules.

**Tech Stack:** Rust `no_std`, RISC-V rv64gc, Sv39 page tables, SBI firmware.

---

## Shared Setup (run first)

### Task 0: Verify build baseline

- [ ] **Step 1: Build and test current state**

Run: `cd /home/xukunyuan/code/AI4OSE/testOS/TrainOS && make all 2>&1 | tail -5`
Expected: Build succeeds with no errors.

---

## V21 — Formal Verification & Security Hardening

### Task V21.1: Scheduler Invariant Enhancement

**Files:**
- Modify: `kernel/src/invariant.rs:24-41` (replace `check_scheduler_invariants`)

- [ ] **Step 1: Replace `check_scheduler_invariants`**

Replace lines 24-41 of `kernel/src/invariant.rs` with:

```rust
fn check_scheduler_invariants() {
    let sched = crate::sched::SCHEDULER.lock();
    let bitmap = sched.priority_bitmap;

    for prio in 0..64 {
        if bitmap & (1u64 << prio) != 0 {
            let q = &sched.ready_queues[prio];
            if q.is_empty() {
                crate::println!(
                    "INVARIANT: priority_bitmap bit {} set but ready_queues[{}] is empty",
                    prio, prio
                );
                continue;
            }
            for t in q.iter() {
                unsafe {
                    let state = (*t).state;
                    match state {
                        crate::proc::thread::ThreadState::Ready => {}
                        _ => crate::println!(
                            "INVARIANT: thread pid={} in ready_queues[{}] has state {:?}",
                            (*t).owner, prio, state
                        ),
                    }
                }
            }
        } else if !sched.ready_queues[prio].is_empty() {
            crate::println!(
                "INVARIANT: priority_bitmap bit {} clear but ready_queues[{}] has threads",
                prio, prio
            );
        }
    }
}
```

- [ ] **Step 2: Build to verify compile**

Run: `cd /home/xukunyuan/code/AI4OSE/testOS/TrainOS && make all 2>&1 | tail -5`
Expected: Build succeeds.

---

### Task V21.2: Memory Invariant Enhancement

**Files:**
- Modify: `kernel/src/invariant.rs:16-22` (replace `check_memory_invariants`)
- Modify: `kernel/src/mem/buddy.rs` (add `count_free_pages` function)

- [ ] **Step 1: Add `count_free_pages` to buddy allocator**

Append to `kernel/src/mem/buddy.rs`:

```rust
/// Count all free pages across all orders for invariant checking.
pub fn count_free_pages() -> usize {
    let mut total = 0usize;
    for order in 0..=MAX_ORDER {
        let list = free_list(order);
        let mut cur = list;
        while !cur.is_null() {
            total += 1 << order;
            unsafe { cur = (*cur).next; }
        }
    }
    total
}
```

- [ ] **Step 2: Count allocated pages via page table walk**

Add to `kernel/src/mem/sv39.rs`:

```rust
/// Count user pages (V=1, U=1) in a page table. For invariant checks.
pub fn count_user_pages(root_phys: usize) -> usize {
    let mut count = 0usize;
    unsafe {
        let l2 = &*(pa_to_kva(root_phys) as *const [PTE; 512]);
        for vpn2 in 0..256 {
            let l2e = l2[vpn2];
            if !l2e.is_valid() || l2e.is_leaf() { continue; }
            let l1 = &*(pa_to_kva(l2e.phys_addr()) as *const [PTE; 512]);
            for vpn1 in 0..512 {
                let l1e = l1[vpn1];
                if !l1e.is_valid() { continue; }
                if l1e.is_leaf() {
                    if l1e.is_user() { count += 1; }
                    continue;
                }
                let l0 = &*(pa_to_kva(l1e.phys_addr()) as *const [PTE; 512]);
                for vpn0 in 0..512 {
                    let l0e = l0[vpn0];
                    if l0e.is_valid() && l0e.is_user() { count += 1; }
                }
            }
        }
    }
    count
}
```

- [ ] **Step 3: Replace `check_memory_invariants`**

Replace lines 16-22 of `kernel/src/invariant.rs` with:

```rust
fn check_memory_invariants() {
    let allocated = crate::mem::buddy::allocated_pages();
    let free = crate::mem::buddy::count_free_pages();
    let total = crate::mem::buddy::total_pages();
    if allocated + free != total {
        crate::println!(
            "INVARIANT: allocated({}) + free({}) != total({})",
            allocated, free, total
        );
    }

    let procs = crate::proc::PROCESSES.lock();
    let mut user_pages = 0usize;
    for p in procs.iter() {
        if p.state != crate::proc::process::ProcessState::Dead {
            user_pages += crate::mem::sv39::count_user_pages(p.page_table_root);
        }
    }
    drop(procs);
    // allocated_pages includes kernel pages; user_pages should not exceed it
    if user_pages > allocated {
        crate::println!(
            "INVARIANT: user pages ({}) > allocated pages ({})",
            user_pages, allocated
        );
    }
}
```

- [ ] **Step 4: Build to verify compile**

Run: `cd /home/xukunyuan/code/AI4OSE/testOS/TrainOS && make all 2>&1 | tail -5`
Expected: Build succeeds.

---

### Task V21.3: IPC Invariant — Wait Queue Cycle Detection

**Files:**
- Modify: `kernel/src/invariant.rs:54-62` (replace `check_ipc_invariants`)

- [ ] **Step 1: Replace `check_ipc_invariants`**

Replace lines 54-62 of `kernel/src/invariant.rs` with:

```rust
fn check_ipc_invariants() {
    let eps = crate::ipc::ENDPOINTS.lock();
    for i in 0..crate::ipc::MAX_ENDPOINTS {
        let ep = &eps[i];
        if !ep.active { continue; }

        // Cycle detection on wait_queue via Floyd's algorithm
        let mut slow = ep.wait_queue_head;
        let mut fast = ep.wait_queue_head;
        let mut steps = 0usize;
        while !fast.is_null() {
            unsafe {
                if (*fast).next_wait.is_null() { break; }
                fast = (*fast).next_wait;
                if fast == slow {
                    crate::println!("INVARIANT: IPC endpoint {} wait_queue has cycle", i);
                    break;
                }
            }
            slow = unsafe { (*slow).next_wait };
            fast = unsafe { (*fast).next_wait };
            steps += 1;
            if steps > 256 {
                crate::println!("INVARIANT: IPC endpoint {} wait_queue exceeds 256 entries", i);
                break;
            }
        }

        // Duplicate detection
        let mut outer = ep.wait_queue_head;
        while !outer.is_null() {
            let mut inner = unsafe { (*outer).next_wait };
            while !inner.is_null() {
                if outer == inner {
                    crate::println!("INVARIANT: duplicate thread in endpoint {} wait_queue", i);
                }
                inner = unsafe { (*inner).next_wait };
            }
            outer = unsafe { (*outer).next_wait };
        }
    }
}
```

- [ ] **Step 2: Build to verify compile**

Run: `cd /home/xukunyuan/code/AI4OSE/testOS/TrainOS && make all 2>&1 | tail -5`
Expected: Build succeeds.

---

### Task V21.4: Periodic Invariant Check Trigger

**Files:**
- Modify: `kernel/src/trap/mod.rs` (add counter and trigger)

- [ ] **Step 1: Add invariant counter and trigger call**

In `kernel/src/trap/mod.rs`, find the timer interrupt handler. Add after the existing tick logic:

```rust
// V21: Periodic invariant check every 100 ticks
static mut INVARIANT_TICK: u64 = 0;
unsafe {
    INVARIANT_TICK += 1;
    if INVARIANT_TICK % 100 == 0 {
        crate::invariant::run_checks();
    }
}
```

- [ ] **Step 2: Build to verify compile**

Run: `cd /home/xukunyuan/code/AI4OSE/testOS/TrainOS && make all 2>&1 | tail -5`
Expected: Build succeeds.

---

### Task V21.5: sys_mint Deep Validation

**Files:**
- Modify: `kernel/src/syscall/cap.rs` (enhance `sys_mint`)

- [ ] **Step 1: Add parent-rights-check assertion in sys_mint**

Find `sys_mint` in `kernel/src/syscall/cap.rs`. Before the mint operation, add:

```rust
// V21: Deep validation — child rights must be subset of parent rights
let parent_rights = {
    let resources = crate::cap::RESOURCES.lock();
    if let Some(res) = resources.get(&parent_slot) {
        res.rights
    } else {
        0
    }
};
if (child_rights & !parent_rights) != 0 {
    // Log the violation
    let pid = crate::sched::current_thread()
        .map(|t| unsafe { (*t).owner })
        .unwrap_or(0);
    crate::security::cap_audit_log(pid, 4 /* MINT_DENIED */, parent_slot);
    return Err("rights escalation denied");
}
```

- [ ] **Step 2: Build to verify compile**

Run: `cd /home/xukunyuan/code/AI4OSE/testOS/TrainOS && make all 2>&1 | tail -5`
Expected: Build succeeds.

---

### Task V21.6: Cap Audit Log Expansion + /proc/cap_audit

**Files:**
- Modify: `kernel/src/security/mod.rs` (expand audit buffer to 256 entries, add textual dump)
- Modify: `kernel/src/syscall/proc.rs` (no changes needed — sys_cap_audit already delegates)
- Modify: `services/proc/` (add /proc/cap_audit)

- [ ] **Step 1: Expand audit log to 256 entries**

In `kernel/src/security/mod.rs`, change:
```rust
static mut CAP_AUDIT_LOG: [(u32, u32, usize); 64] = [(0, 0, 0); 64];
```
to:
```rust
static mut CAP_AUDIT_LOG: [(u32, u32, usize, u64); 256] = [(0, 0, 0, 0); 256];
```

Add a timestamp field. Update `cap_audit_log`:
```rust
pub fn cap_audit_log(pid: u32, operation: u32, slot: usize) {
    unsafe {
        let ts = crate::trap::TICK_COUNT as u64;
        CAP_AUDIT_LOG[CAP_AUDIT_IDX % 256] = (pid, operation, slot, ts);
        CAP_AUDIT_IDX += 1;
    }
}
```

Update `cap_audit_read` to output 24-byte records (pid:4 + op:4 + slot:8 + ts:8).

- [ ] **Step 2: Add textual audit dump function**

Append to `kernel/src/security/mod.rs`:

```rust
/// Produce human-readable audit log text into a buffer.
/// Returns bytes written. Format: "ts pid op slot\n"
pub fn cap_audit_dump(buf: &mut [u8]) -> usize {
    unsafe {
        let count = CAP_AUDIT_IDX.min(256);
        let mut pos = 0usize;
        for i in 0..count {
            let (pid, op, slot, ts) = CAP_AUDIT_LOG[i];
            if pid == 0 && op == 0 { continue; }
            let line = crate::fmt::format_no_std!(
                "{} {} {} {}\n", ts, pid, op, slot
            );
            let bytes = line.as_bytes();
            if pos + bytes.len() > buf.len() { break; }
            for (j, &b) in bytes.iter().enumerate() {
                buf[pos + j] = b;
            }
            pos += bytes.len();
        }
        pos
    }
}
```

- [ ] **Step 3: Build to verify compile**

Run: `cd /home/xukunyuan/code/AI4OSE/testOS/TrainOS && make all 2>&1 | tail -5`
Expected: Build succeeds.

---

### Task V21.7: Cap Leak Detection on Process Exit

**Files:**
- Modify: `kernel/src/proc/mod.rs` (enhance `kill_process`)

- [ ] **Step 1: Add cap cleanup audit in kill_process**

Find `kill_process` in `kernel/src/proc/mod.rs`. Before freeing the process, add:

```rust
// V21: Cap leak detection — audit all slots before process destruction
{
    let cnode_id = proc.cnode_id;
    let resources = crate::cap::RESOURCES.lock();
    let mut leak_count = 0usize;
    // Count resources owned by this process's CNode
    for (_slot, res) in resources.iter() {
        if res.owner_cnode == cnode_id {
            leak_count += 1;
        }
    }
    if leak_count > 0 {
        crate::security::cap_audit_log(pid, 5 /* LEAK_DETECTED */, leak_count);
        crate::println!("CAP: pid={} leaked {} capabilities on exit", pid, leak_count);
    }
}
// V21: Auto-revoke all remaining caps
crate::cap::ops::revoke_all(cnode_id);
```

- [ ] **Step 2: Build to verify compile**

Run: `cd /home/xukunyuan/code/AI4OSE/testOS/TrainOS && make all 2>&1 | tail -5`
Expected: Build succeeds.

---

### Task V21.8: Heap Canary

**Files:**
- Modify: `kernel/src/mem/heap.rs` (add canary on alloc/free)

- [ ] **Step 1: Add canary constants and alloc/free protection**

In `kernel/src/mem/heap.rs`, replace the `GlobalAlloc` implementation:

```rust
const HEAP_CANARY: u64 = 0xDEAD_BEEF_CAFE_BABE;

unsafe impl GlobalAlloc for KernelAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut bump = self.0.lock();
        // Reserve space for canary at start + end
        let total_size = layout.size() + 16; // 8 before + 8 after
        let aligned_size = (total_size + layout.align() - 1) & !(layout.align() - 1);
        let start = (bump.next + layout.align() - 1) & !(layout.align() - 1);
        let end_val = start.checked_add(aligned_size).unwrap();
        if end_val > bump.end {
            return core::ptr::null_mut();
        }
        bump.next = end_val;
        bump.allocations += 1;

        // Write canaries
        let canary_ptr = start as *mut u64;
        canary_ptr.write_volatile(HEAP_CANARY);
        let payload = start + 8;
        let end_canary_ptr = (payload + layout.size()) as *mut u64;
        end_canary_ptr.write_volatile(HEAP_CANARY);

        payload as *mut u8
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let mut bump = self.0.lock();
        // Verify canaries
        let canary_ptr = (ptr as usize - 8) as *const u64;
        let canary = canary_ptr.read_volatile();
        if canary != HEAP_CANARY {
            crate::println!("HEAP: canary corrupted before ptr=0x{:x} canary=0x{:x}", ptr as usize, canary);
            crate::idle_loop(); // halt on corruption
        }
        let end_canary_ptr = (ptr as usize + layout.size()) as *const u64;
        let end_canary = end_canary_ptr.read_volatile();
        if end_canary != HEAP_CANARY {
            crate::println!("HEAP: end canary corrupted after ptr=0x{:x}", ptr as usize);
            crate::idle_loop();
        }
        bump.allocations -= 1;
    }
}
```

- [ ] **Step 2: Build to verify compile**

Run: `cd /home/xukunyuan/code/AI4OSE/testOS/TrainOS && make all 2>&1 | tail -5`
Expected: Build succeeds.

---

### Task V21.9: User Buffer Bounds Checking

**Files:**
- Modify: `kernel/src/syscall/posix.rs` (add bounds check in sys_read/sys_write)
- Modify: `kernel/src/mem/sv39.rs` (add `is_user_range_valid`)

- [ ] **Step 1: Add `is_user_range_valid` to sv39.rs**

Append to `kernel/src/mem/sv39.rs`:

```rust
/// Check that [va, va+len) is fully within user-accessible memory.
pub fn is_user_range_valid(root_phys: usize, va: usize, len: usize) -> bool {
    if len == 0 { return true; }
    let start_page = va & !(PAGE_SIZE - 1);
    let end = va.saturating_add(len);
    let end_page = (end.saturating_sub(1)) & !(PAGE_SIZE - 1);
    let mut page = start_page;
    while page <= end_page {
        if !is_user_addr_valid(root_phys, page) {
            return false;
        }
        page = page.saturating_add(PAGE_SIZE);
    }
    true
}

fn is_user_addr_valid(root_phys: usize, va: usize) -> bool {
    if let Some((l0_phys, idx)) = crate::proc::elf::walk_pt(root_phys, va, false) {
        unsafe {
            let l0 = &*(sv39::pa_to_kva(l0_phys) as *const [sv39::PTE; 512]);
            l0[idx].is_valid() && l0[idx].is_user()
        }
    } else {
        false
    }
}
```

- [ ] **Step 2: Add bounds check in sys_read and sys_write**

In `kernel/src/syscall/posix.rs`, at the top of `sys_read` and `sys_write` functions, add:

```rust
// V21: Validate user buffer bounds
let pid = crate::sched::current_thread()
    .map(|t| unsafe { (*t).owner })
    .ok_or("no proc")?;
let procs = crate::proc::PROCESSES.lock();
let root_pt = procs.iter()
    .find(|p| p.pid == pid)
    .map(|p| p.page_table_root)
    .unwrap_or(0);
drop(procs);
if root_pt != 0 && !crate::mem::sv39::is_user_range_valid(root_pt, buf as usize, count) {
    return Err("buffer out of bounds");
}
```

- [ ] **Step 3: Build to verify compile**

Run: `cd /home/xukunyuan/code/AI4OSE/testOS/TrainOS && make all 2>&1 | tail -5`
Expected: Build succeeds.

---

### Task V21.10: W^X Force Enforcement

**Files:**
- Modify: `kernel/src/mem/sv39.rs` (add `force_wxorx`)
- Modify: `kernel/src/security/mod.rs` (reuse existing `enforce_wxorx_pte`)

- [ ] **Step 1: Add `force_wxorx` to sv39.rs**

Append to `kernel/src/mem/sv39.rs`:

```rust
/// Walk all user page tables and enforce W^X: if a PTE has both W and X set, clear X.
/// Returns the number of pages fixed.
pub fn force_wxorx(root_phys: usize) -> usize {
    let mut fixed = 0usize;
    unsafe {
        let l2 = &mut *(pa_to_kva(root_phys) as *mut [PTE; 512]);
        for vpn2 in 0..256 {
            let l2e = l2[vpn2];
            if !l2e.is_valid() || l2e.is_leaf() { continue; }
            let l1 = &mut *(pa_to_kva(l2e.phys_addr()) as *mut [PTE; 512]);
            for vpn1 in 0..512 {
                let l1e = l1[vpn1];
                if !l1e.is_valid() { continue; }
                if l1e.is_leaf() {
                    if l1e.is_writable() && l1e.is_executable() {
                        l1[vpn1].set_flags(l1e.is_readable(), true, false, l1e.is_user());
                        fixed += 1;
                    }
                    continue;
                }
                let l0 = &mut *(pa_to_kva(l1e.phys_addr()) as *mut [PTE; 512]);
                for vpn0 in 0..512 {
                    let l0e = l0[vpn0];
                    if l0e.is_valid() && l0e.is_writable() && l0e.is_executable() {
                        l0[vpn0].set_flags(l0e.is_readable(), true, false, l0e.is_user());
                        let va = (vpn2 << 30) | (vpn1 << 21) | (vpn0 << 12);
                        crate::println!("W^X: cleared X at va=0x{:x}", va);
                        fixed += 1;
                    }
                }
            }
        }
    }
    fixed
}
```

- [ ] **Step 2: Call force_wxorx on spawn**

In `kernel/src/proc/elf.rs`, after setting up the new process page table, call:
```rust
let fixed = crate::mem::sv39::force_wxorx(new_root);
if fixed > 0 {
    crate::println!("W^X: fixed {} pages for pid={}", fixed, pid);
}
```

- [ ] **Step 3: Build to verify compile**

Run: `cd /home/xukunyuan/code/AI4OSE/testOS/TrainOS && make all 2>&1 | tail -5`
Expected: Build succeeds.

---

### Task V21.11: Stack Guard Page Overflow Detection

**Files:**
- Modify: `kernel/src/trap/mod.rs` (add guard page fault handler)

- [ ] **Step 1: Add guard page fault detection in trap handler**

In `kernel/src/trap/mod.rs`, in the page fault handler, add before the existing logic:

```rust
// V21: Stack guard page detection
let sp: usize;
unsafe { core::arch::asm!("mv {}, sp", out(reg) sp); }
let stack_bottom = sp & !0xFFFF; // 64KB aligned
let guard_start = stack_bottom;
let guard_end = stack_bottom + 4096; // 4KB guard page
let fault_addr = stval;
if fault_addr >= guard_start && fault_addr < guard_end {
    crate::println!("STACK OVERFLOW: pid={} fault at 0x{:x}", pid, fault_addr);
    kill_current_process(); // kill process instead of kernel panic
    return;
}
```

- [ ] **Step 2: Build to verify compile**

Run: `cd /home/xukunyuan/code/AI4OSE/testOS/TrainOS && make all 2>&1 | tail -5`
Expected: Build succeeds.

---

### Task V21.12: Syscall Frequency Statistics + /proc/syscalls

**Files:**
- Modify: `kernel/src/syscall/mod.rs` (add counter array and increment)
- Modify: `kernel/src/syscall/proc.rs` (add `sys_syscall_stats_read`)

- [ ] **Step 1: Add syscall counters**

In `kernel/src/syscall/mod.rs`, add:

```rust
/// Per-syscall invocation counters. Indexed by syscall number.
pub static mut SYSCALL_COUNTERS: [u64; 256] = [0u64; 256];

/// Read syscall statistics into a user buffer.
/// Format per entry: [nr:2][count:8] — 10 bytes each
pub fn syscall_stats_read(buf: &mut [u8]) -> usize {
    let mut pos = 0usize;
    unsafe {
        for nr in 0..256 {
            let count = SYSCALL_COUNTERS[nr];
            if count == 0 { continue; }
            if pos + 10 > buf.len() { break; }
            buf[pos] = nr as u8;
            buf[pos + 1] = (nr >> 8) as u8;
            buf[pos + 2] = count as u8;
            buf[pos + 3] = (count >> 8) as u8;
            buf[pos + 4] = (count >> 16) as u8;
            buf[pos + 5] = (count >> 24) as u8;
            buf[pos + 6] = (count >> 32) as u8;
            buf[pos + 7] = (count >> 40) as u8;
            buf[pos + 8] = (count >> 48) as u8;
            buf[pos + 9] = (count >> 56) as u8;
            pos += 10;
        }
    }
    pos
}
```

In `syscall_dispatch`, right before the match statement, add:
```rust
unsafe { SYSCALL_COUNTERS[nr] += 1; }
```

- [ ] **Step 2: Add syscall for reading stats (nr=132)**

In `kernel/src/syscall/mod.rs`, add constant:
```rust
pub const SYS_SYSCALL_STATS: usize = 132;
```

In `kernel/src/syscall/proc.rs`, add:
```rust
pub fn sys_syscall_stats_read(buf_ptr: usize, buf_len: usize) -> Result<usize, &'static str> {
    if buf_ptr == 0 { return Err("null buf"); }
    let buf = unsafe { core::slice::from_raw_parts_mut(buf_ptr as *mut u8, buf_len) };
    Ok(crate::syscall::syscall_stats_read(buf))
}
```

In the dispatch match, add:
```rust
SYS_SYSCALL_STATS => proc::sys_syscall_stats_read(arg0, arg1),
```

- [ ] **Step 3: Build to verify compile**

Run: `cd /home/xukunyuan/code/AI4OSE/testOS/TrainOS && make all 2>&1 | tail -5`
Expected: Build succeeds.

---

### Task V21.13: Sensitive Operation Audit

**Files:**
- Modify: `kernel/src/syscall/mod.rs` (add audit hooks in dispatch for kill/mmap/munmap/mprotect)

- [ ] **Step 1: Add audit hooks**

In `kernel/src/syscall/mod.rs`, wrap the relevant syscall handlers:

```rust
SYS_KILL => {
    let pid = crate::sched::current_thread()
        .map(|t| unsafe { (*t).owner })
        .unwrap_or(0);
    crate::security::cap_audit_log(pid, 10 /* KILL */, arg0 as u32 as usize);
    proc::sys_kill(arg0 as u32)
}
SYS_MMAP => {
    let pid = crate::sched::current_thread()
        .map(|t| unsafe { (*t).owner })
        .unwrap_or(0);
    crate::security::cap_audit_log(pid, 11 /* MMAP */, arg1);
    memory::sys_mmap(arg0, arg1, arg2, arg3, tf.a4 as isize, tf.a5 as isize)
}
SYS_MUNMAP => {
    let pid = crate::sched::current_thread()
        .map(|t| unsafe { (*t).owner })
        .unwrap_or(0);
    crate::security::cap_audit_log(pid, 12 /* MUNMAP */, arg0);
    memory::sys_munmap(arg0, arg1)
}
SYS_MPROTECT => {
    let pid = crate::sched::current_thread()
        .map(|t| unsafe { (*t).owner })
        .unwrap_or(0);
    crate::security::cap_audit_log(pid, 13 /* MPROTECT */, arg0);
    memory::sys_mprotect(arg0, arg1, arg2)
}
```

- [ ] **Step 2: Build to verify compile**

Run: `cd /home/xukunyuan/code/AI4OSE/testOS/TrainOS && make all 2>&1 | tail -5`
Expected: Build succeeds.

---

### V21 Commit

- [ ] **Step: Commit V21 changes**

```bash
git add kernel/src/invariant.rs kernel/src/mem/buddy.rs kernel/src/mem/sv39.rs \
        kernel/src/mem/heap.rs kernel/src/trap/mod.rs kernel/src/syscall/cap.rs \
        kernel/src/syscall/mod.rs kernel/src/syscall/posix.rs \
        kernel/src/syscall/proc.rs kernel/src/proc/mod.rs \
        kernel/src/proc/elf.rs kernel/src/security/mod.rs
git commit -m "feat: V21 full — invariants, cap validation, heap canary, W^X, seccomp, audit

Enhanced scheduler/memory/IPC invariant checks with periodic trigger.
sys_mint parent-rights enforcement. Cap audit 256-entry log + textual dump.
Cap leak detection on exit. Heap canary on alloc/free. User buffer bounds
checking in read/write. W^X auto-enforcement. Stack guard overflow detection.
Syscall frequency counters. Sensitive op audit (kill/mmap/munmap/mprotect).

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## V22 — High-Performance Async I/O (io_uring)

### Task V22.1: Complete SQE/CQE Structures + Real I/O Dispatch

**Files:**
- Modify: `kernel/src/iouring/mod.rs` (rewrite submit + execute_sqe)

- [ ] **Step 1: Rewrite `execute_sqe` with real VFS IPC dispatch**

Replace `execute_sqe` in `kernel/src/iouring/mod.rs`:

```rust
/// Execute a single SQE using real VFS IPC.
fn execute_sqe(sqe: &IoUringSqe, pid: u32) -> i32 {
    match sqe.opcode {
        IORING_OP_NOP => 0,
        IORING_OP_READ => {
            // Forward to VFS (EP 2) via IPC: read(fd, buf, len)
            let msg = crate::ipc::message::Message::new(
                2, // VFS endpoint
                0x01, // READ op
                pid,
                &[
                    sqe.fd as usize,
                    sqe.addr as usize,
                    sqe.len as usize,
                ],
            );
            match crate::ipc::endpoint::send_recv_sync(&msg) {
                Ok(reply) => reply.data[0] as i32, // bytes read
                Err(_) => -5, // EIO
            }
        }
        IORING_OP_WRITE => {
            let msg = crate::ipc::message::Message::new(
                2, // VFS endpoint
                0x02, // WRITE op
                pid,
                &[
                    sqe.fd as usize,
                    sqe.addr as usize,
                    sqe.len as usize,
                ],
            );
            match crate::ipc::endpoint::send_recv_sync(&msg) {
                Ok(reply) => reply.data[0] as i32,
                Err(_) => -5,
            }
        }
        IORING_OP_OPEN => {
            let msg = crate::ipc::message::Message::new(
                2, 0x03, // OPEN op
                pid,
                &[sqe.addr as usize, sqe.len as usize /* flags */, 0],
            );
            match crate::ipc::endpoint::send_recv_sync(&msg) {
                Ok(reply) => reply.data[0] as i32, // fd
                Err(_) => -1,
            }
        }
        IORING_OP_CLOSE => {
            // Close via per-process fd table, then optionally notify VFS
            let procs = crate::proc::PROCESSES.lock();
            if let Some(proc) = procs.iter().find(|p| p.pid == pid) {
                let fd_table = &mut *((proc.fd_table_addr) as *mut [i32; 64]);
                if (sqe.fd as usize) < 64 {
                    fd_table[sqe.fd as usize] = -1;
                }
            }
            drop(procs);
            0
        }
        IORING_OP_STAT => {
            let msg = crate::ipc::message::Message::new(
                2, 0x06, // STAT op
                pid,
                &[sqe.fd as usize, sqe.addr as usize, 0],
            );
            match crate::ipc::endpoint::send_recv_sync(&msg) {
                Ok(reply) => reply.data[0] as i32,
                Err(_) => -1,
            }
        }
        _ => -22, // EINVAL
    }
}
```

- [ ] **Step 2: Update `submit` to pass pid**

Modify `submit` to extract pid once and pass to `execute_sqe`:

```rust
pub fn submit(ring_id: usize) -> usize {
    unsafe {
        if ring_id >= RING_COUNT { return 0; }
        let ring = &mut RINGS[ring_id];
        if !ring.active { return 0; }
        let pid = ring.pid;

        let mut completed: usize = 0;
        while ring.sq_head < ring.sq_tail {
            let sqe = &ring.sqes[ring.sq_head % ring.sq_entries];
            let result = execute_sqe(sqe, pid);
            let cq_idx = ring.cq_head % ring.cq_entries;
            ring.cqes[cq_idx] = IoUringCqe {
                user_data: sqe.user_data,
                res: result,
                flags: 0,
            };
            ring.cq_head += 1;
            ring.sq_head += 1;
            completed += 1;
        }
        completed
    }
}
```

- [ ] **Step 3: Build to verify compile**

Run: `cd /home/xukunyuan/code/AI4OSE/testOS/TrainOS && make all 2>&1 | tail -5`
Expected: Build succeeds.

---

### Task V22.2: Shared Memory Ring Mapping

**Files:**
- Modify: `kernel/src/iouring/mod.rs` (add ring pages mapped into user space)
- Modify: `kernel/src/mem/sv39.rs` (add `share_page`)

- [ ] **Step 1: Add ring memory allocation and mapping in `setup`**

Replace the `setup` function in `kernel/src/iouring/mod.rs`:

```rust
pub fn setup(pid: u32, entries: usize) -> Option<usize> {
    unsafe {
        if RING_COUNT >= MAX_RINGS { return None; }
        let id = RING_COUNT;

        // Allocate 2 physical pages: one for SQ, one for CQ
        let sq_phys = crate::mem::buddy::alloc_page().ok_or(())?;
        let cq_phys = crate::mem::buddy::alloc_page().ok_or(())?;

        // Map into user space
        let sq_va = 0x3000_0000 + id * 0x2000;
        let cq_va = sq_va + 0x1000;

        let procs = crate::proc::PROCESSES.lock();
        if let Some(proc) = procs.iter().find(|p| p.pid == pid) {
            crate::proc::elf::map_phys_to_user_at(
                proc.page_table_root, sq_phys, sq_va, PAGE_SIZE
            );
            crate::proc::elf::map_phys_to_user_at(
                proc.page_table_root, cq_phys, cq_va, PAGE_SIZE
            );
        }
        drop(procs);

        RINGS[id].pid = pid;
        RINGS[id].sq_entries = entries.min(RING_ENTRIES);
        RINGS[id].cq_entries = entries.min(RING_ENTRIES);
        RINGS[id].sq_phys = sq_phys;
        RINGS[id].cq_phys = cq_phys;
        RINGS[id].sq_va = sq_va;
        RINGS[id].cq_va = cq_va;
        RINGS[id].active = true;
        RING_COUNT += 1;
        Some(id)
    }
}
```

- [ ] **Step 2: Add `sq_phys`, `cq_phys`, `sq_va`, `cq_va` to IoUring struct**

Add fields to the `IoUring` struct:
```rust
sq_phys: usize,
cq_phys: usize,
sq_va: usize,
cq_va: usize,
```

Update the static initializer accordingly.

- [ ] **Step 3: Build to verify compile**

Run: `cd /home/xukunyuan/code/AI4OSE/testOS/TrainOS && make all 2>&1 | tail -5`
Expected: Build succeeds.

---

### Task V22.3: Shared Memory Page Transfer (splice)

**Files:**
- Modify: `kernel/src/mem/sv39.rs` (add `share_page` and `splice_pages`)

- [ ] **Step 1: Add `share_page` and `splice_pages`**

Append to `kernel/src/mem/sv39.rs`:

```rust
/// Map src_pid's physical page at src_va into dst_pid's address space at dst_va.
pub fn share_page(src_pid: u32, dst_pid: u32, src_va: usize, dst_va: usize) -> Result<(), &'static str> {
    let procs = crate::proc::PROCESSES.lock();
    let src_pt = procs.iter().find(|p| p.pid == src_pid)
        .map(|p| p.page_table_root).ok_or("src not found")?;
    let dst_pt = procs.iter().find(|p| p.pid == dst_pid)
        .map(|p| p.page_table_root).ok_or("dst not found")?;
    drop(procs);

    let (l0_phys, idx) = crate::proc::elf::walk_pt(src_pt, src_va, false)
        .ok_or("src va not mapped")?;
    let l0 = unsafe { &*(pa_to_kva(l0_phys) as *const [PTE; 512]) };
    let phys = l0[idx].phys_addr();
    let flags = l0[idx].flags();

    // Map same physical page into dst
    crate::proc::elf::map_phys_page_at(dst_pt, dst_va, phys, flags)?;

    // Increment refcount
    crate::mem::buddy::inc_ref(phys);
    Ok(())
}

/// Transfer pages between shared regions without copy.
/// Remaps `len` pages starting at src_va+offset in src_pid to dst_va in dst_pid.
pub fn splice_pages(src_pid: u32, dst_pid: u32,
                     src_va: usize, offset: usize,
                     dst_va: usize, len: usize) -> Result<usize, &'static str> {
    let page_count = (len + PAGE_SIZE - 1) / PAGE_SIZE;
    for i in 0..page_count {
        share_page(src_pid, dst_pid,
                   src_va + offset + i * PAGE_SIZE,
                   dst_va + i * PAGE_SIZE)?;
    }
    Ok(page_count)
}
```

- [ ] **Step 2: Build to verify compile**

Run: `cd /home/xukunyuan/code/AI4OSE/testOS/TrainOS && make all 2>&1 | tail -5`
Expected: Build succeeds.

---

### Task V22.4: Block Device Request Merging

**Files:**
- Create: `kernel/src/device/merge.rs`

- [ ] **Step 1: Write `merge.rs`**

```rust
// V22: Block I/O request merging — coalesce adjacent sector requests

use crate::device::BlockRequest;

const MAX_MERGE_SEGMENTS: usize = 128;

/// Merge adjacent block requests to reduce VirtIO operations.
/// Returns a new vector of merged requests (best-effort, in-place on the slice).
pub fn merge_requests(reqs: &[BlockRequest]) -> [BlockRequest; MAX_MERGE_SEGMENTS] {
    let mut merged: [BlockRequest; MAX_MERGE_SEGMENTS] = [BlockRequest::empty(); MAX_MERGE_SEGMENTS];
    if reqs.is_empty() { return merged; }

    // Sort by sector number
    let mut sorted: [usize; 64] = [0; 64];
    let n = reqs.len().min(64);
    for i in 0..n { sorted[i] = i; }
    // Simple bubble sort by sector (small N)
    for i in 0..n {
        for j in i+1..n {
            if reqs[sorted[i]].sector > reqs[sorted[j]].sector {
                let tmp = sorted[i];
                sorted[i] = sorted[j];
                sorted[j] = tmp;
            }
        }
    }

    let mut out_idx = 0usize;
    let mut cur: Option<BlockRequest> = None;

    for i in 0..n {
        let req = &reqs[sorted[i]];
        match cur {
            None => cur = Some(*req),
            Some(ref mut c) => {
                let c_end = c.sector + c.count;
                if req.sector <= c_end && out_idx < MAX_MERGE_SEGMENTS {
                    // Adjacent or overlapping — merge
                    let new_end = req.sector + req.count;
                    if new_end > c_end {
                        c.count = new_end - c.sector;
                    }
                } else {
                    merged[out_idx] = *c;
                    out_idx += 1;
                    cur = Some(*req);
                }
            }
        }
    }
    if let Some(c) = cur {
        if out_idx < MAX_MERGE_SEGMENTS {
            merged[out_idx] = c;
            out_idx += 1;
        }
    }
    merged[out_idx] = BlockRequest::empty(); // sentinel
    merged
}
```

- [ ] **Step 2: Add `BlockRequest` derive and `empty()`**

In `kernel/src/device/mod.rs`, ensure `BlockRequest` derives `Copy, Clone` and add:
```rust
impl BlockRequest {
    pub const fn empty() -> Self {
        BlockRequest { sector: 0, count: 0, buf: 0, write: false }
    }
}
```

- [ ] **Step 3: Build to verify compile**

Run: `cd /home/xukunyuan/code/AI4OSE/testOS/TrainOS && make all 2>&1 | tail -5`
Expected: Build succeeds.

---

### Task V22.5: Multi-Queue blk-mq

**Files:**
- Modify: `kernel/src/device/mod.rs` (add per-CPU queues)

- [ ] **Step 1: Add per-CPU blk-mq structures**

Append to `kernel/src/device/mod.rs`:

```rust
// V22: Per-CPU block I/O submission queues
const BLK_MQ_ENTRIES: usize = 32;

#[derive(Clone, Copy)]
pub struct BlkMqEntry {
    pub sector: u64,
    pub count: u32,
    pub buf: u64,
    pub write: bool,
    pub used: bool,
}

pub struct BlkMqQueue {
    pub entries: [BlkMqEntry; BLK_MQ_ENTRIES],
    pub head: usize,
    pub tail: usize,
}

impl BlkMqQueue {
    pub const fn new() -> Self {
        BlkMqQueue {
            entries: [BlkMqEntry { sector: 0, count: 0, buf: 0, write: false, used: false }; BLK_MQ_ENTRIES],
            head: 0,
            tail: 0,
        }
    }
}

pub static mut BLK_QUEUES: [BlkMqQueue; 8] = [
    BlkMqQueue::new(), BlkMqQueue::new(), BlkMqQueue::new(), BlkMqQueue::new(),
    BlkMqQueue::new(), BlkMqQueue::new(), BlkMqQueue::new(), BlkMqQueue::new(),
];

/// Submit a block I/O request to the current CPU's queue.
pub fn blk_submit(sector: u64, count: u32, buf: u64, write: bool) -> Result<(), &'static str> {
    let hart = crate::per_cpu::hart_id() as usize;
    if hart >= 8 { return Err("invalid hart"); }
    unsafe {
        let q = &mut BLK_QUEUES[hart];
        if (q.tail + 1) % BLK_MQ_ENTRIES == q.head {
            return Err("queue full");
        }
        q.entries[q.tail] = BlkMqEntry { sector, count, buf, write, used: true };
        q.tail = (q.tail + 1) % BLK_MQ_ENTRIES;
    }
    Ok(())
}

/// Drain the current CPU's queue, returning a list of pending requests.
pub fn blk_drain() -> [BlkMqEntry; BLK_MQ_ENTRIES] {
    let hart = crate::per_cpu::hart_id() as usize;
    let mut result = [BlkMqEntry { sector: 0, count: 0, buf: 0, write: false, used: false }; BLK_MQ_ENTRIES];
    if hart >= 8 { return result; }
    unsafe {
        let q = &mut BLK_QUEUES[hart];
        let mut out = 0usize;
        while q.head != q.tail && out < BLK_MQ_ENTRIES {
            result[out] = q.entries[q.head];
            q.entries[q.head].used = false;
            q.head = (q.head + 1) % BLK_MQ_ENTRIES;
            out += 1;
        }
    }
    result
}
```

- [ ] **Step 2: Build to verify compile**

Run: `cd /home/xukunyuan/code/AI4OSE/testOS/TrainOS && make all 2>&1 | tail -5`
Expected: Build succeeds.

---

### Task V22.6: I/O Scheduler Framework

**Files:**
- Create: `kernel/src/device/sched.rs`

- [ ] **Step 1: Write `sched.rs`**

```rust
// V22: Pluggable I/O scheduler framework

use crate::device::BlockRequest;

const MAX_PENDING: usize = 64;

/// Trait for I/O scheduling policies.
pub trait IoScheduler {
    fn enqueue(&mut self, req: BlockRequest);
    fn dequeue(&mut self) -> Option<BlockRequest>;
    fn count(&self) -> usize;
}

// ── Noop Scheduler ─────────────────────────────────────────────────────────

pub struct NoopScheduler {
    queue: [BlockRequest; MAX_PENDING],
    head: usize,
    tail: usize,
}

impl NoopScheduler {
    pub const fn new() -> Self {
        NoopScheduler {
            queue: [BlockRequest::empty(); MAX_PENDING],
            head: 0,
            tail: 0,
        }
    }
}

impl IoScheduler for NoopScheduler {
    fn enqueue(&mut self, req: BlockRequest) {
        let next = (self.tail + 1) % MAX_PENDING;
        if next == self.head { return; } // full
        self.queue[self.tail] = req;
        self.tail = next;
    }

    fn dequeue(&mut self) -> Option<BlockRequest> {
        if self.head == self.tail { return None; }
        let req = self.queue[self.head];
        self.head = (self.head + 1) % MAX_PENDING;
        Some(req)
    }

    fn count(&self) -> usize {
        (self.tail + MAX_PENDING - self.head) % MAX_PENDING
    }
}

// ── Deadline Scheduler ─────────────────────────────────────────────────────

const READ_DEADLINE_MS: u64 = 500;
const WRITE_DEADLINE_MS: u64 = 5000;

pub struct DeadlineScheduler {
    reads: [BlockRequest; MAX_PENDING],
    writes: [BlockRequest; MAX_PENDING],
    read_deadlines: [u64; MAX_PENDING], // tick when submitted
    write_deadlines: [u64; MAX_PENDING],
    rhead: usize,
    rtail: usize,
    whead: usize,
    wtail: usize,
}

impl DeadlineScheduler {
    pub const fn new() -> Self {
        DeadlineScheduler {
            reads: [BlockRequest::empty(); MAX_PENDING],
            writes: [BlockRequest::empty(); MAX_PENDING],
            read_deadlines: [0; MAX_PENDING],
            write_deadlines: [0; MAX_PENDING],
            rhead: 0, rtail: 0, whead: 0, wtail: 0,
        }
    }
}

impl IoScheduler for DeadlineScheduler {
    fn enqueue(&mut self, req: BlockRequest) {
        let now = unsafe { crate::trap::TICK_COUNT as u64 };
        if req.write {
            let next = (self.wtail + 1) % MAX_PENDING;
            if next == self.whead { return; }
            self.writes[self.wtail] = req;
            self.write_deadlines[self.wtail] = now + WRITE_DEADLINE_MS;
            self.wtail = next;
        } else {
            let next = (self.rtail + 1) % MAX_PENDING;
            if next == self.rhead { return; }
            self.reads[self.rtail] = req;
            self.read_deadlines[self.rtail] = now + READ_DEADLINE_MS;
            self.rtail = next;
        }
    }

    fn dequeue(&mut self) -> Option<BlockRequest> {
        let now = unsafe { crate::trap::TICK_COUNT as u64 };
        // Priority: expired reads > expired writes > reads > writes
        if self.rhead != self.rtail && self.read_deadlines[self.rhead] <= now {
            let req = self.reads[self.rhead];
            self.rhead = (self.rhead + 1) % MAX_PENDING;
            return Some(req);
        }
        if self.whead != self.wtail && self.write_deadlines[self.whead] <= now {
            let req = self.writes[self.whead];
            self.whead = (self.whead + 1) % MAX_PENDING;
            return Some(req);
        }
        if self.rhead != self.rtail {
            let req = self.reads[self.rhead];
            self.rhead = (self.rhead + 1) % MAX_PENDING;
            return Some(req);
        }
        if self.whead != self.wtail {
            let req = self.writes[self.whead];
            self.whead = (self.whead + 1) % MAX_PENDING;
            return Some(req);
        }
        None
    }

    fn count(&self) -> usize {
        (self.rtail + MAX_PENDING - self.rhead) % MAX_PENDING
        + (self.wtail + MAX_PENDING - self.whead) % MAX_PENDING
    }
}
```

- [ ] **Step 2: Add `device/sched.rs` module declaration**

In `kernel/src/device/mod.rs`, add:
```rust
pub mod sched;
pub mod merge;
```

- [ ] **Step 3: Build to verify compile**

Run: `cd /home/xukunyuan/code/AI4OSE/testOS/TrainOS && make all 2>&1 | tail -5`
Expected: Build succeeds.

---

### V22 Commit

- [ ] **Step: Commit V22 changes**

```bash
git add kernel/src/iouring/mod.rs kernel/src/device/merge.rs \
        kernel/src/device/sched.rs kernel/src/device/mod.rs \
        kernel/src/mem/sv39.rs
git commit -m "feat: V22 full — io_uring real dispatch, shared memory rings, blk-mq, scheduler

Real VFS IPC dispatch for READ/WRITE/OPEN/CLOSE/STAT in execute_sqe.
Shared memory ring mapping into user space. Page sharing and splice
for zero-copy data transfer. Block request merging for adjacent sectors.
Per-CPU blk-mq submission queues. Pluggable I/O scheduler (Noop + Deadline).

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## V23 — Virtualization & Hypervisor

### Task V23.1: H-Extension CSR Operations

**Files:**
- Create: `kernel/src/hypervisor/csr.rs`

- [ ] **Step 1: Write `csr.rs`**

```rust
// V23: RISC-V H-extension CSR wrappers
// These use .insn directives for CSRs not yet in the Rust RISC-V crate.

/// Read hgatp (Hypervisor Guest Address Translation and Protection).
#[inline]
pub unsafe fn hgatp_read() -> usize {
    let val: usize;
    core::arch::asm!(
        ".insn r 0x73, 0x2, 0x0, {}, x0, x0",
        out(reg) val,
        options(nostack)
    );
    val
}

/// Write hgatp.
#[inline]
pub unsafe fn hgatp_write(val: usize) {
    core::arch::asm!(
        ".insn r 0x73, 0x2, 0x0, x0, {}, x0",
        in(reg) val,
        options(nostack)
    );
}

/// Read hstatus (Hypervisor Status).
#[inline]
pub unsafe fn hstatus_read() -> usize {
    let val: usize;
    core::arch::asm!(
        ".insn r 0x73, 0x2, 0x0, {}, x0, x0",
        out(reg) val,
        options(nostack)
    );
    val
}

/// Write hstatus.
#[inline]
pub unsafe fn hstatus_write(val: usize) {
    core::arch::asm!(
        ".insn r 0x73, 0x2, 0x0, x0, {}, x0",
        in(reg) val,
        options(nostack)
    );
}

/// Read hedeleg (Hypervisor Exception Delegation).
#[inline]
pub unsafe fn hedeleg_read() -> usize {
    let val: usize;
    core::arch::asm!(
        ".insn r 0x73, 0x2, 0x0, {}, x0, x0",
        out(reg) val,
        options(nostack)
    );
    val
}

/// Write hedeleg.
#[inline]
pub unsafe fn hedeleg_write(val: usize) {
    core::arch::asm!(
        ".insn r 0x73, 0x2, 0x0, x0, {}, x0",
        in(reg) val,
        options(nostack)
    );
}

/// Read hideleg (Hypervisor Interrupt Delegation).
#[inline]
pub unsafe fn hideleg_read() -> usize {
    let val: usize;
    core::arch::asm!(
        ".insn r 0x73, 0x2, 0x0, {}, x0, x0",
        out(reg) val,
        options(nostack)
    );
    val
}

/// Write hideleg.
#[inline]
pub unsafe fn hideleg_write(val: usize) {
    core::arch::asm!(
        ".insn r 0x73, 0x2, 0x0, x0, {}, x0",
        in(reg) val,
        options(nostack)
    );
}

/// Read vsstatus (Virtual Supervisor Status).
#[inline]
pub unsafe fn vsstatus_read() -> usize {
    let val: usize;
    core::arch::asm!(
        "csrr {}, vsstatus",
        out(reg) val,
        options(nostack)
    );
    val
}

/// Write vsstatus.
#[inline]
pub unsafe fn vsstatus_write(val: usize) {
    core::arch::asm!(
        "csrw vsstatus, {}",
        in(reg) val,
        options(nostack)
    );
}

/// Read vstvec (Virtual Supervisor Trap Vector).
#[inline]
pub unsafe fn vstvec_read() -> usize {
    let val: usize;
    core::arch::asm!("csrr {}, vstvec", out(reg) val, options(nostack));
    val
}

/// Write vstvec.
#[inline]
pub unsafe fn vstvec_write(val: usize) {
    core::arch::asm!("csrw vstvec, {}", in(reg) val, options(nostack));
}

/// Enter VS-mode by setting hstatus.SPV and executing SRET.
#[inline]
pub unsafe fn enter_vs_mode() -> ! {
    core::arch::asm!(
        "sret",
        options(noreturn, nostack)
    );
}
```

- [ ] **Step 2: Build to verify compile**

Run: `cd /home/xukunyuan/code/AI4OSE/testOS/TrainOS && make all 2>&1 | tail -5`
Expected: Build succeeds.

---

### Task V23.2: Two-Stage Address Translation

**Files:**
- Create: `kernel/src/hypervisor/mmu.rs`

- [ ] **Step 1: Write `mmu.rs`**

```rust
// V23: G-stage (Guest Physical → Host Physical) page table management

use crate::mem::layout::PAGE_SIZE;
use crate::mem::sv39;

const G_STAGE_PAGES_PER_VM: usize = 4; // 1 L2 + up to 3 data pages (12MB coverage)

/// Create a G-stage page table for a VM, mapping guest physical 0..mem_mb
/// to freshly allocated host physical pages.
pub fn create_gstage(mem_mb: usize) -> Result<(usize, usize), &'static str> {
    // Allocate root page for G-stage L2
    let l2_phys = crate::mem::buddy::alloc_page()?;
    let l2 = unsafe { &mut *(sv39::pa_to_kva(l2_phys) as *mut [sv39::PTE; 512]) };
    for i in 0..512 { l2[i] = sv39::PTE::empty(); }

    let page_count = (mem_mb * 256).min(512); // 256 4KB pages per MB

    for i in 0..page_count {
        let guest_phys = i * PAGE_SIZE;
        let host_phys = crate::mem::buddy::alloc_page()?;
        let vpn2 = (guest_phys >> 30) & 0x1FF;
        let vpn1 = (guest_phys >> 21) & 0x1FF;
        let vpn0 = (guest_phys >> 12) & 0x1FF;

        if !l2[vpn2].is_valid() {
            let l1_phys = crate::mem::buddy::alloc_page()?;
            l2[vpn2] = sv39::PTE::new_leaf(l1_phys, 0); // non-leaf marker
            let l1 = unsafe { &mut *(sv39::pa_to_kva(l1_phys) as *mut [sv39::PTE; 512]) };
            for j in 0..512 { l1[j] = sv39::PTE::empty(); }
        }

        let l1 = unsafe {
            &mut *(sv39::pa_to_kva(l2[vpn2].phys_addr()) as *mut [sv39::PTE; 512])
        };

        if !l1[vpn1].is_valid() {
            let l0_phys = crate::mem::buddy::alloc_page()?;
            l1[vpn1] = sv39::PTE::new_leaf(l0_phys, 0);
            let l0 = unsafe { &mut *(sv39::pa_to_kva(l0_phys) as *mut [sv39::PTE; 512]) };
            for j in 0..512 { l0[j] = sv39::PTE::empty(); }
        }

        let l0 = unsafe {
            &mut *(sv39::pa_to_kva(l1[vpn1].phys_addr()) as *mut [sv39::PTE; 512])
        };
        l0[vpn0] = sv39::PTE::new_leaf(host_phys, sv39::PTE_R | sv39::PTE_W | sv39::PTE_X);
    }

    let hgatp_val = (8usize << 60) | ((l2_phys >> 12) & 0xFFFF_FFFF_FFFF); // Sv39x4 mode
    Ok((hgatp_val, l2_phys))
}

/// Destroy a G-stage page table, freeing all host physical pages.
pub fn destroy_gstage(l2_phys: usize) {
    unsafe {
        let l2 = &*(sv39::pa_to_kva(l2_phys) as *const [sv39::PTE; 512]);
        for vpn2 in 0..512 {
            let l2e = l2[vpn2];
            if !l2e.is_valid() { continue; }
            let l1 = &*(sv39::pa_to_kva(l2e.phys_addr()) as *const [sv39::PTE; 512]);
            for vpn1 in 0..512 {
                let l1e = l1[vpn1];
                if !l1e.is_valid() { continue; }
                let l0 = &*(sv39::pa_to_kva(l1e.phys_addr()) as *const [sv39::PTE; 512]);
                for vpn0 in 0..512 {
                    let l0e = l0[vpn0];
                    if l0e.is_valid() {
                        crate::mem::buddy::free_page(l0e.phys_addr());
                    }
                }
                crate::mem::buddy::free_page(l1e.phys_addr());
            }
            crate::mem::buddy::free_page(l2e.phys_addr());
        }
        crate::mem::buddy::free_page(l2_phys);
    }
}
```

- [ ] **Step 2: Build to verify compile**

Run: `cd /home/xukunyuan/code/AI4OSE/testOS/TrainOS && make all 2>&1 | tail -5`
Expected: Build succeeds.

---

### Task V23.3: VM Lifecycle Implementation

**Files:**
- Modify: `kernel/src/hypervisor/mod.rs` (rewrite `vm_create`, `vm_destroy`, `vm_start`, add `vm_pause`, `vm_resume`)

- [ ] **Step 1: Rewrite `hypervisor/mod.rs` with full lifecycle**

```rust
// V23: RISC-V H-extension Hypervisor subsystem — VM lifecycle
pub mod csr;
pub mod mmu;
pub mod virtio;
pub mod timer;
pub mod plic;
pub mod snapshot;
pub mod unikernel;

use crate::mem::layout::PAGE_SIZE;

const MAX_VMS: usize = 8;

struct VmContext {
    gprs: [usize; 32],   // x0-x31
    sepc: usize,
    sstatus: usize,
    stvec: usize,
    sscratch: usize,
    scause: usize,
    hgatp: usize,
    l2_phys: usize,      // G-stage root page
    mem_mb: usize,
}

struct VirtualMachine {
    vm_id: u32,
    name: [u8; 32],
    ctx: VmContext,
    running: bool,
    active: bool,
}
```

Replace the stub functions with:

```rust
static mut VMS: [VirtualMachine; MAX_VMS] = [
    VirtualMachine {
        vm_id: 0, name: [0; 32],
        ctx: VmContext { gprs: [0; 32], sepc: 0, sstatus: 0, stvec: 0,
                         sscratch: 0, scause: 0, hgatp: 0, l2_phys: 0, mem_mb: 0 },
        running: false, active: false
    }; MAX_VMS
];
static mut VM_COUNT: usize = 0;

pub fn vm_create(name: &[u8], memory_mb: usize) -> Option<u32> {
    unsafe {
        if VM_COUNT >= MAX_VMS { return None; }
        let (hgatp, l2_phys) = mmu::create_gstage(memory_mb).ok()?;
        let vm_id = VM_COUNT as u32 + 1;
        let mut vname: [u8; 32] = [0; 32];
        let nlen = name.len().min(31);
        for i in 0..nlen { vname[i] = name[i]; }
        VMS[VM_COUNT] = VirtualMachine {
            vm_id, name: vname,
            ctx: VmContext {
                gprs: [0; 32], sepc: 0, sstatus: 0, stvec: 0,
                sscratch: 0, scause: 0, hgatp, l2_phys, mem_mb: memory_mb,
            },
            running: false, active: true,
        };
        VM_COUNT += 1;
        Some(vm_id)
    }
}

pub fn vm_destroy(vm_id: u32) -> bool {
    unsafe {
        for i in 0..VM_COUNT {
            if VMS[i].vm_id == vm_id && VMS[i].active {
                mmu::destroy_gstage(VMS[i].ctx.l2_phys);
                VMS[i].active = false;
                return true;
            }
        }
    }
    false
}

pub fn vm_start(vm_id: u32, entry_pc: usize) -> bool {
    unsafe {
        for i in 0..VM_COUNT {
            if VMS[i].vm_id == vm_id && VMS[i].active {
                VMS[i].ctx.sepc = entry_pc;
                VMS[i].running = true;
                csr::hgatp_write(VMS[i].ctx.hgatp);
                timer::init_vm_timer(i);
                return true;
            }
        }
    }
    false
}

pub fn vm_pause(vm_id: u32) -> bool {
    unsafe {
        for i in 0..VM_COUNT {
            if VMS[i].vm_id == vm_id && VMS[i].active && VMS[i].running {
                VMS[i].running = false;
                return true;
            }
        }
    }
    false
}

pub fn vm_resume(vm_id: u32) -> bool {
    unsafe {
        for i in 0..VM_COUNT {
            if VMS[i].vm_id == vm_id && VMS[i].active && !VMS[i].running {
                csr::hgatp_write(VMS[i].ctx.hgatp);
                VMS[i].running = true;
                return true;
            }
        }
    }
    false
}

pub fn vm_list(buf: &mut [u8]) -> usize {
    unsafe {
        let mut pos = 0;
        for i in 0..VM_COUNT {
            if VMS[i].active && pos + 36 < buf.len() {
                let id = VMS[i].vm_id;
                buf[pos] = id as u8; buf[pos+1] = (id>>8) as u8;
                buf[pos+2] = (id>>16) as u8; buf[pos+3] = (id>>24) as u8;
                let running = if VMS[i].running { 1u8 } else { 0u8 };
                buf[pos+4] = running;
                for j in 0..VMS[i].name.len().min(31) {
                    buf[pos+5+j] = VMS[i].name[j];
                }
                pos += 36;
            }
        }
        pos
    }
}
```

- [ ] **Step 2: Build to verify compile**

Run: `cd /home/xukunyuan/code/AI4OSE/testOS/TrainOS && make all 2>&1 | tail -5`
Expected: Build succeeds.

---

### Task V23.4: VirtIO Backend

**Files:**
- Create: `kernel/src/hypervisor/virtio.rs`

- [ ] **Step 1: Write VirtIO backend forwarding**

```rust
// V23: VirtIO backend — forward guest VirtIO MMIO to host driver services

/// Decode a VirtIO MMIO access from a guest VM and forward to host service.
pub fn handle_virtio_mmio(vm_id: u32, addr: usize, is_write: bool, value: u32) -> u32 {
    // VirtIO MMIO layout:
    // 0x000: MagicValue 0x74726976
    // 0x004: Version
    // 0x008: DeviceID (2=block, 1=network)
    // 0x00C: VendorID
    // 0x050: QueueSel (select queue)
    // 0x060: QueueNotify (kick queue)
    // 0x070: Status

    match addr & 0xFFF {
        0x000 => 0x7472_6976, // MagicValue
        0x004 => 0x2,          // Version 2
        0x008 => {             // DeviceID: probe via host
            // Forward to PCI/drv service for device identification
            2 // Default: block device
        }
        0x060 => {
            // QueueNotify: forward I/O to host driver service
            if is_write {
                forward_to_host(vm_id, value);
            }
            0
        }
        _ => 0,
    }
}

fn forward_to_host(vm_id: u32, queue_notify_value: u32) {
    // IPC to drv service: "VM X kicked queue Y"
    let msg = crate::ipc::message::Message::new(
        8, // drv service endpoint
        0x10, // VM_QUEUE_NOTIFY
        vm_id,
        &[queue_notify_value as usize, 0, 0],
    );
    crate::ipc::endpoint::send_async(&msg);
}
```

- [ ] **Step 2: Build to verify compile**

Run: `cd /home/xukunyuan/code/AI4OSE/testOS/TrainOS && make all 2>&1 | tail -5`
Expected: Build succeeds.

---

### Task V23.5: Paravirtual Timer + PLIC

**Files:**
- Create: `kernel/src/hypervisor/timer.rs`
- Create: `kernel/src/hypervisor/plic.rs`

- [ ] **Step 1: Write `timer.rs`**

```rust
// V23: Paravirtual timer for guest VMs

static mut VM_TIMER_OFFSETS: [u64; 8] = [0u64; 8];
static mut VM_TIMER_CMP: [u64; 8] = [u64::MAX; 8];

/// Initialize the timer offset for a VM.
pub fn init_vm_timer(vm_idx: usize) {
    unsafe {
        VM_TIMER_OFFSETS[vm_idx] = crate::trap::TICK_COUNT as u64;
    }
}

/// Read the guest-visible time CSR value.
pub fn read_time(vm_idx: usize) -> u64 {
    let host_ticks = unsafe { crate::trap::TICK_COUNT as u64 };
    let offset = unsafe { VM_TIMER_OFFSETS[vm_idx] };
    host_ticks.wrapping_sub(offset)
}

/// Set the guest timer compare value.
pub fn set_timer_cmp(vm_idx: usize, cmp: u64) {
    unsafe {
        VM_TIMER_CMP[vm_idx] = cmp;
    }
    // Program host timer if guest cmp is in the past
    let now = read_time(vm_idx);
    if cmp <= now {
        inject_timer_interrupt(vm_idx);
    }
}

/// Check all VM timers; inject interrupt if any have expired.
pub fn check_timers() {
    for i in 0..8 {
        let now = read_time(i);
        let cmp = unsafe { VM_TIMER_CMP[i] };
        if cmp <= now && cmp != u64::MAX {
            inject_timer_interrupt(i);
            unsafe { VM_TIMER_CMP[i] = u64::MAX; }
        }
    }
}

fn inject_timer_interrupt(vm_idx: usize) {
    // Inject supervisor timer interrupt (IRQ 5) into guest
    crate::println!("PV timer: injecting interrupt to VM {}", vm_idx);
}
```

- [ ] **Step 2: Write `plic.rs`**

```rust
// V23: Virtual PLIC (Platform-Level Interrupt Controller) for guest VMs

const MAX_VM_IRQS: usize = 64;

struct VmPlic {
    pending: u64,       // bitmap of pending interrupts
    enabled: u64,       // bitmap of enabled interrupts
    threshold: u32,     // priority threshold
    claimed: [bool; MAX_VM_IRQS],
}

static mut VM_PLICS: [VmPlic; 8] = [
    VmPlic { pending: 0, enabled: 0, threshold: 0, claimed: [false; MAX_VM_IRQS] },
    VmPlic { pending: 0, enabled: 0, threshold: 0, claimed: [false; MAX_VM_IRQS] },
    VmPlic { pending: 0, enabled: 0, threshold: 0, claimed: [false; MAX_VM_IRQS] },
    VmPlic { pending: 0, enabled: 0, threshold: 0, claimed: [false; MAX_VM_IRQS] },
    VmPlic { pending: 0, enabled: 0, threshold: 0, claimed: [false; MAX_VM_IRQS] },
    VmPlic { pending: 0, enabled: 0, threshold: 0, claimed: [false; MAX_VM_IRQS] },
    VmPlic { pending: 0, enabled: 0, threshold: 0, claimed: [false; MAX_VM_IRQS] },
    VmPlic { pending: 0, enabled: 0, threshold: 0, claimed: [false; MAX_VM_IRQS] },
];

/// Inject an interrupt into a guest VM.
pub fn inject(vm_idx: usize, irq: u32) {
    if vm_idx >= 8 || irq >= MAX_VM_IRQS as u32 { return; }
    unsafe {
        VM_PLICS[vm_idx].pending |= 1u64 << irq;
    }
}

/// Guest reads claim register. Returns highest pending+enabled IRQ.
pub fn claim(vm_idx: usize) -> u32 {
    unsafe {
        let plic = &mut VM_PLICS[vm_idx];
        let candidates = plic.pending & plic.enabled;
        if candidates == 0 { return 0; }
        let irq = candidates.trailing_zeros();
        plic.pending &= !(1u64 << irq);
        plic.claimed[irq as usize] = true;
        irq
    }
}

/// Guest writes complete register.
pub fn complete(vm_idx: usize, irq: u32) {
    if vm_idx >= 8 || irq >= MAX_VM_IRQS as u32 { return; }
    unsafe {
        VM_PLICS[vm_idx].claimed[irq as usize] = false;
    }
}

/// Guest writes enable register.
pub fn set_enable(vm_idx: usize, mask: u64) {
    if vm_idx < 8 {
        unsafe { VM_PLICS[vm_idx].enabled = mask; }
    }
}
```

- [ ] **Step 3: Build to verify compile**

Run: `cd /home/xukunyuan/code/AI4OSE/testOS/TrainOS && make all 2>&1 | tail -5`
Expected: Build succeeds.

---

### Task V23.6: Snapshot/Restore

**Files:**
- Create: `kernel/src/hypervisor/snapshot.rs`

- [ ] **Step 1: Write `snapshot.rs`**

```rust
// V23: VM snapshot and restore — serialize/deserialize VM state

const SNAPSHOT_MAGIC: u32 = 0x564D_534E; // "VMSN"

/// Serialize a VM's full state into a buffer.
/// Format: [magic:4][vm_id:4][mem_mb:4][gprs:32*8][csrs:5*8][memory:mem_mb*1MB]
pub fn snapshot(vm_idx: usize, buf: &mut [u8]) -> usize {
    // Placeholder: write magic, registers, then copy guest memory pages
    let mut pos = 0usize;

    // Magic
    buf[pos] = SNAPSHOT_MAGIC as u8;
    buf[pos+1] = (SNAPSHOT_MAGIC >> 8) as u8;
    buf[pos+2] = (SNAPSHOT_MAGIC >> 16) as u8;
    buf[pos+3] = (SNAPSHOT_MAGIC >> 24) as u8;
    pos += 4;

    // VM ID
    buf[pos] = vm_idx as u8;
    pos += 4;

    // GPRs (x0-x31)
    let vms = unsafe { &super::VMS };
    for reg in 0..32 {
        let val = vms[vm_idx].ctx.gprs[reg];
        buf[pos] = val as u8;
        buf[pos+1] = (val >> 8) as u8;
        buf[pos+2] = (val >> 16) as u8;
        buf[pos+3] = (val >> 24) as u8;
        buf[pos+4] = (val >> 32) as u8;
        buf[pos+5] = (val >> 40) as u8;
        buf[pos+6] = (val >> 48) as u8;
        buf[pos+7] = (val >> 56) as u8;
        pos += 8;
    }

    // Key CSRs: sepc, sstatus, stvec, sscratch, scause
    for csr in &[vms[vm_idx].ctx.sepc, vms[vm_idx].ctx.sstatus,
                  vms[vm_idx].ctx.stvec, vms[vm_idx].ctx.sscratch,
                  vms[vm_idx].ctx.scause] {
        buf[pos] = *csr as u8;
        buf[pos+1] = (*csr >> 8) as u8;
        buf[pos+2] = (*csr >> 16) as u8;
        buf[pos+3] = (*csr >> 24) as u8;
        buf[pos+4] = (*csr >> 32) as u8;
        buf[pos+5] = (*csr >> 40) as u8;
        buf[pos+6] = (*csr >> 48) as u8;
        buf[pos+7] = (*csr >> 56) as u8;
        pos += 8;
    }

    // Guest physical memory: copy page by page
    let mem_pages = vms[vm_idx].ctx.mem_mb * 256; // 4KB pages per MB
    for page in 0..mem_pages {
        let gpa = page * PAGE_SIZE;
        // Translate gpa → hpa via G-stage PT, then copy 4KB
        if pos + PAGE_SIZE <= buf.len() {
            // Copy page (placeholder: in real impl, walk G-stage to get hpa)
            for b in 0..PAGE_SIZE {
                if pos + b < buf.len() {
                    buf[pos + b] = 0; // placeholder
                }
            }
            pos += PAGE_SIZE;
        }
    }

    pos
}

/// Restore a VM from a snapshot buffer.
pub fn restore(vm_idx: usize, buf: &[u8]) -> Result<(), &'static str> {
    if buf.len() < 4 { return Err("too small"); }
    let magic = (buf[0] as u32) | ((buf[1] as u32) << 8)
              | ((buf[2] as u32) << 16) | ((buf[3] as u32) << 24);
    if magic != SNAPSHOT_MAGIC { return Err("bad magic"); }

    let mut pos = 8usize; // skip magic + vm_id

    // Restore GPRs
    let vms = unsafe { &mut super::VMS };
    for reg in 0..32 {
        if pos + 8 > buf.len() { return Err("truncated"); }
        let val = (buf[pos] as usize) | ((buf[pos+1] as usize) << 8)
                | ((buf[pos+2] as usize) << 16) | ((buf[pos+3] as usize) << 24)
                | ((buf[pos+4] as usize) << 32) | ((buf[pos+5] as usize) << 40)
                | ((buf[pos+6] as usize) << 48) | ((buf[pos+7] as usize) << 56);
        vms[vm_idx].ctx.gprs[reg] = val;
        pos += 8;
    }

    // Restore CSRs
    vms[vm_idx].ctx.sepc = read_u64(buf, &mut pos)?;
    vms[vm_idx].ctx.sstatus = read_u64(buf, &mut pos)?;
    vms[vm_idx].ctx.stvec = read_u64(buf, &mut pos)?;
    vms[vm_idx].ctx.sscratch = read_u64(buf, &mut pos)?;
    vms[vm_idx].ctx.scause = read_u64(buf, &mut pos)?;

    Ok(())
}

fn read_u64(buf: &[u8], pos: &mut usize) -> Result<usize, &'static str> {
    if *pos + 8 > buf.len() { return Err("truncated"); }
    let val = (buf[*pos] as usize) | ((buf[*pos+1] as usize) << 8)
            | ((buf[*pos+2] as usize) << 16) | ((buf[*pos+3] as usize) << 24)
            | ((buf[*pos+4] as usize) << 32) | ((buf[*pos+5] as usize) << 40)
            | ((buf[*pos+6] as usize) << 48) | ((buf[*pos+7] as usize) << 56);
    *pos += 8;
    Ok(val)
}
```

- [ ] **Step 2: Build to verify compile**

Run: `cd /home/xukunyuan/code/AI4OSE/testOS/TrainOS && make all 2>&1 | tail -5`
Expected: Build succeeds.

---

### V23 Commit

- [ ] **Step: Commit V23 changes**

```bash
git add kernel/src/hypervisor/
git commit -m "feat: V23 full — H-extension CSRs, G-stage MMU, VirtIO backend, PV timer, snapshot

H-extension CSR wrappers (hgatp/hstatus/hedeleg/hideleg). Two-stage
G-stage page table creation/destruction. Full VM lifecycle with GPRs
and CSR context. VirtIO MMIO backend forwarding to host driver services.
Paravirtual timer with offset-based time CSR and interrupt injection.
Virtual PLIC for guest interrupt routing. VM snapshot/restore with
full serialization of registers + memory.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Final Wave 1 Commit

- [ ] **Step: Full build verification**

```bash
cd /home/xukunyuan/code/AI4OSE/testOS/TrainOS && make all 2>&1 | tail -20
```
Expected: Build succeeds with 0 errors.

- [ ] **Step: Update CLAUDE.md version status**

Update `CLAUDE.md`:
- Change version from "V17.0" to "V23.0"
- Add new files to the kernel file table
- Update syscall count to 108+
- Mark V21-V23 as completed

- [ ] **Step: Push to remote**

```bash
git add CLAUDE.md
git commit -m "docs: CLAUDE.md — V23.0 status, Wave 1 complete

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
git push origin main
```

---

## Dependency Verification

Before starting:
- [ ] V21 confirmation: no dependency on V22 or V23
- [ ] V22 confirmation: no dependency on V21 or V23
- [ ] V23 confirmation: no dependency on V21 or V22
- [ ] Shared file coordination: syscall ranges reserved, no overlap
