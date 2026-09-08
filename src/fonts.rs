//! 跨平台 CJK 字体发现与安装。

use std::sync::Arc;

#[cfg(not(target_arch = "wasm32"))]
pub(super) fn install_cjk_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let candidates = [
        "C:/Windows/Fonts/msyh.ttc",
        "C:/Windows/Fonts/msyh.ttf",
        "C:/Windows/Fonts/simhei.ttf",
        "C:/Windows/Fonts/simsun.ttc",
        "/System/Library/Fonts/PingFang.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
        "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
    ];
    for path in candidates {
        if let Ok(bytes) = std::fs::read(path) {
            fonts
                .font_data
                .insert("cjk".into(), Arc::new(egui::FontData::from_owned(bytes)));
            for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                if let Some(list) = fonts.families.get_mut(&family) {
                    list.insert(0, "cjk".into());
                }
            }
            break;
        }
    }
    for (name, path, family) in [
        (
            "ui-latin",
            "C:/Windows/Fonts/segoeui.ttf",
            egui::FontFamily::Proportional,
        ),
        (
            "code-latin",
            "C:/Windows/Fonts/consola.ttf",
            egui::FontFamily::Monospace,
        ),
    ] {
        if let Ok(bytes) = std::fs::read(path) {
            fonts
                .font_data
                .insert(name.into(), Arc::new(egui::FontData::from_owned(bytes)));
            fonts
                .families
                .entry(family)
                .or_default()
                .insert(0, name.into());
        }
    }
    ctx.set_fonts(fonts);
}

#[cfg(target_arch = "wasm32")]
pub(super) fn install_cjk_fonts(ctx: &egui::Context) {
    mod bundled {
        include!(concat!(env!("OUT_DIR"), "/web_fonts.rs"));
    }
    use bundled::{CJK, CODE, UI};
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "cjk".into(),
        Arc::new(egui::FontData::from_static(CJK.expect("内置中文字体"))),
    );
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        fonts
            .families
            .entry(family)
            .or_default()
            .insert(0, "cjk".into());
    }
    for (name, bytes, family) in [
        ("ui-latin", UI, egui::FontFamily::Proportional),
        ("code-latin", CODE, egui::FontFamily::Monospace),
    ] {
        if let Some(bytes) = bytes {
            fonts
                .font_data
                .insert(name.into(), Arc::new(egui::FontData::from_static(bytes)));
            fonts
                .families
                .entry(family)
                .or_default()
                .insert(0, name.into());
        }
    }
    ctx.set_fonts(fonts);
}
