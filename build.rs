//! Web 包仅嵌入随仓库分发的开放字体。
use std::{env, fs, path::Path};
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=assets/fonts/NotoSansSC.ttf");
    if env::var("CARGO_CFG_TARGET_ARCH").as_deref() != Ok("wasm32") {
        return;
    }
    let font =
        Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap()).join("assets/fonts/NotoSansSC.ttf");
    let source = format!("pub const CJK: Option<&[u8]> = Some(include_bytes!({:?}));\npub const UI: Option<&[u8]> = None;\npub const CODE: Option<&[u8]> = None;\n", font);
    fs::write(
        Path::new(&env::var("OUT_DIR").unwrap()).join("web_fonts.rs"),
        source,
    )
    .unwrap();
}
