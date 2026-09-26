//! 用户态程序链接脚本注入（audiod/evsrcdemo 同款，S28）。
//!
//! 通过 `CARGO_MANIFEST_DIR` 得到绝对路径，把 `linker.ld` 传给链接器，
//! 将 `.text` 等段定位到用户态地址（0x400000 起），`ENTRY(_start)`。
//! `-no-pie`：强制生成 ET_EXEC——内核 ELF 加载器只接受 ET_EXEC。
//! 仅在裸机目标生效，宿主 `cargo test` 不受影响。

fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "none" {
        let dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
        println!("cargo:rustc-link-arg=-T{}/linker.ld", dir);
        println!("cargo:rustc-link-arg=-no-pie");
    }
}
