//! Puts `memory.x` where the linker can find it via `link.x`'s own
//! `INCLUDE memory.x` (cortex-m-rt's linker script pulls it in itself -
//! we must NOT also pass `-Tmemory.x` directly, or the `MEMORY` block gets
//! defined twice and the link fails with "region 'BOOT2' already defined").
use std::env;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

fn main() {
    let out = &PathBuf::from(env::var_os("OUT_DIR").unwrap());
    File::create(out.join("memory.x"))
        .unwrap()
        .write_all(include_bytes!("memory.x"))
        .unwrap();
    println!("cargo:rustc-link-search={}", out.display());

    println!("cargo:rerun-if-changed=memory.x");
    println!("cargo:rerun-if-changed=build.rs");
}
