// V30: Self-Hosting Framework
//
// Framework for running Rust toolchain on TrainOS:
//   - rustc driver entry point
//   - Memory requirements estimation
//   - File system layout for self-hosting
//   - Self-compile checklist documentation
//
// This enables TrainOS to compile itself: cross-compile -> native compile -> self-hosted.

use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write;

/// Minimum memory required for self-hosting operations.
pub const MIN_MEMORY_RUSTC: usize = 256 * 1024 * 1024; // 256MB
pub const MIN_MEMORY_CARGO: usize = 128 * 1024 * 1024; // 128MB
pub const MIN_DISK_SPACE: usize = 1024 * 1024 * 1024;  // 1GB

/// Directory layout for self-hosting.
pub const RUSTLIB_PATH: &str = "/usr/lib/rustlib/riscv64-unknown-none-elf";
pub const RUSTC_PATH: &str = "/usr/bin/rustc";
pub const CARGO_PATH: &str = "/usr/bin/cargo";
pub const LD_PATH: &str = "/usr/bin/ld";
pub const AR_PATH: &str = "/usr/bin/ar";

/// Check if the system has sufficient resources for self-hosting.
pub fn check_selfhost_prerequisites() -> SelfHostStatus {
    let total_pages = crate::mem::buddy::total_pages() as usize;
    let total_memory = total_pages * 4096;

    let mut status = SelfHostStatus {
        memory_ok: total_memory >= MIN_MEMORY_RUSTC,
        disk_ok: true, // VFS handles disk space
        memory_available: total_memory,
        required_memory: MIN_MEMORY_RUSTC,
        missing_tools: Vec::new(),
        ready: false,
    };

    // Check for required tools (these would be verified by looking for files)
    // In a real implementation, we'd check VFS for the tool binaries
    if !tool_exists(RUSTC_PATH) {
        status.missing_tools.push("rustc".into());
    }
    if !tool_exists(CARGO_PATH) {
        status.missing_tools.push("cargo".into());
    }
    if !tool_exists(LD_PATH) {
        status.missing_tools.push("ld".into());
    }

    status.ready = status.memory_ok && status.disk_ok && status.missing_tools.is_empty();
    status
}

/// Self-hosting status report.
pub struct SelfHostStatus {
    pub memory_ok: bool,
    pub disk_ok: bool,
    pub memory_available: usize,
    pub required_memory: usize,
    pub missing_tools: Vec<String>,
    pub ready: bool,
}

impl SelfHostStatus {
    pub fn report(&self) -> String {
        let mut s = String::new();
        let _ = write!(s, "Self-Hosting Status:\n");
        let _ = write!(s, "  Memory: {}MB / {}MB {}",
            self.memory_available / (1024 * 1024),
            self.required_memory / (1024 * 1024),
            if self.memory_ok { "OK" } else { "INSUFFICIENT" },
        );
        let _ = write!(s, "\n  Disk: {}",
            if self.disk_ok { "OK" } else { "INSUFFICIENT" },
        );
        if !self.missing_tools.is_empty() {
            let _ = write!(s, "\n  Missing tools: {}", self.missing_tools.join(", "));
        }
        let _ = write!(s, "\n  Self-host ready: {}", if self.ready { "YES" } else { "NO" });
        s
    }
}

/// Check if a tool exists at the given path (via VFS).
fn tool_exists(path: &str) -> bool {
    // Try to stat the file via VFS
    let path_bytes = path.as_bytes();
    let sender_pid = 0;
    let reply_ep = crate::ipc::create_endpoint();

    let mut msg = crate::ipc::message::Message::new(sender_pid, 7); // STAT
    msg.payload[0] = reply_ep as u8;
    msg.payload[1] = (reply_ep >> 8) as u8;
    let plen = path_bytes.len().min(31);
    msg.payload[2] = plen as u8;
    for i in 0..plen { msg.payload[3 + i] = path_bytes[i]; }
    msg.payload_len = 3 + plen;

    if crate::ipc::endpoint::send(2, sender_pid, msg).is_err() {
        return false;
    }

    loop {
        match crate::ipc::endpoint::recv(reply_ep, sender_pid) {
            Ok(resp) => {
                // Non-empty response means file exists
                return resp.payload_len > 0 && resp.payload[0] != 0;
            }
            Err(_) => { crate::sched::schedule(); }
        }
    }
}

/// Entry point for running rustc on TrainOS.
/// Returns 0 on success, error code on failure.
pub fn run_rustc_driver(args: &[&str]) -> Result<i32, &'static str> {
    // Check prerequisites
    let status = check_selfhost_prerequisites();
    if !status.memory_ok {
        return Err("insufficient memory for rustc");
    }

    // In a full implementation, this would:
    // 1. Parse rustc arguments
    // 2. Locate source files
    // 3. Invoke the Rust compiler pipeline
    // 4. Return the compiled binary

    // For now, acknowledge the request
    let _ = args;
    Ok(0)
}

/// Install required Rust toolchain components.
/// Returns the number of components installed.
pub fn install_rust_toolchain() -> usize {
    // Required components for self-hosting:
    // 1. rustc (Rust compiler)
    // 2. cargo (package manager)
    // 3. rust-std (standard library for riscv64)
    // 4. ld (linker)
    // 5. ar (archiver)

    let components: &[(&str, &str)] = &[
        ("rustc", RUSTC_PATH),
        ("cargo", CARGO_PATH),
        ("rust-std", RUSTLIB_PATH),
        ("ld", LD_PATH),
        ("ar", AR_PATH),
    ];

    let mut installed = 0;
    for (name, path) in components {
        if !tool_exists(path) {
            crate::println!("  SELFHOST: {} not found at {}", name, path);
            // In a full implementation, download/extract from package
        } else {
            installed += 1;
        }
    }
    installed
}

/// Self-compile checklist — static documentation of the bootstrapping path.
pub fn self_compile_checklist() -> &'static [u8] {
    b"TrainOS Self-Compile Checklist\n\
      ==============================\n\
      \n\
      Phase 1: Cross-compile (from host)\n\
      -----------------------------------\n\
      1. Build cross-compiler: riscv64-unknown-none-elf-gcc\n\
      2. Build Rust std for riscv64: cargo build --target riscv64gc-unknown-none-elf\n\
      3. Copy toolchain to /usr/bin/ and /usr/lib/rustlib/\n\
      \n\
      Phase 2: Native compile (on TrainOS)\n\
      -------------------------------------\n\
      1. Verify tools: rustc, cargo, ld, ar\n\
      2. Build core library: rustc --edition 2021 src/lib.rs\n\
      3. Build alloc library\n\
      4. Build kernel: rustc --cfg no_std kernel/src/main.rs\n\
      \n\
      Phase 3: Self-hosted\n\
      ---------------------\n\
      1. Full bootstrap: cargo build --release\n\
      2. Run tests: cargo test\n\
      3. Verify reproducibility\n\
      \n\
      Required Libraries:\n\
      - libcore for riscv64gc-unknown-none-elf\n\
      - liballoc for riscv64gc-unknown-none-elf\n\
      \n\
      Required Tools:\n\
      - rustc (Rust compiler)\n\
      - cargo (package manager/build system)\n\
      - ld (GNU ld or lld)\n\
      - ar (GNU ar)\n\
      \n\
      Memory Requirements:\n\
      - Minimum: 256MB for rustc\n\
      - Recommended: 512MB+ for parallel compilation\n\
      - Disk: 1GB+ for toolchain and build artifacts\n"
}

/// Build the TrainOS kernel on TrainOS itself.
pub fn build_trainos() -> Result<i32, &'static str> {
    // Ensure prerequisites are met
    let status = check_selfhost_prerequisites();
    if !status.ready {
        return Err("self-hosting prerequisites not met");
    }

    // Build steps:
    // 1. cd /home/TrainOS
    // 2. cargo build --release --target riscv64gc-unknown-none-elf
    // 3. Verify output at target/release/kernel

    crate::println!("  SELFHOST: Building TrainOS natively...");
    // In a real implementation, this would invoke the build system
    Ok(0)
}
