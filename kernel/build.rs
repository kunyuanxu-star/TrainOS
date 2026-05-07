use std::path::Path;

fn main() {
    let target = std::env::var("TARGET").unwrap();
    if target.contains("riscv") {
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let linker_path = Path::new(&manifest_dir).join("linker.ld");
        println!("cargo:rerun-if-changed=linker.ld");
        println!("cargo:rustc-link-arg=-T{}", linker_path.to_string_lossy());
    }
}
