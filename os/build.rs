//! Build script for trainOS
//!
//! This links the custom linker script

use std::path::Path;

fn main() {
    let target = std::env::var("TARGET").unwrap();
    if target.contains("riscv") {
        // Tell cargo to rerun this build script if linker.ld changes
        println!("cargo:rerun-if-changed=linker.ld");

        // Get the path to linker.ld
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let linker_path = Path::new(&manifest_dir).join("linker.ld");
        println!("cargo:rustc-link-arg=-T{}", linker_path.to_string_lossy());
    }
}
