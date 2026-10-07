//! 实际字体族：Latin 保留 Sans/Mono 主字体，CJK 逐字形回退；不下载字体。
use std::sync::{Arc, OnceLock};
const BUNDLED_CJK: &[u8] = include_bytes!("../assets/fonts/NotoSansSC.ttf");
const CJK_PATHS: &[&str] = &[
    "C:/Windows/Fonts/msyh.ttc",
    "C:/Windows/Fonts/msyh.ttf",
    "C:/Windows/Fonts/simhei.ttf",
    "C:/Windows/Fonts/simsun.ttc",
    "/System/Library/Fonts/PingFang.ttc",
    "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
];
fn usable_font(bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return false;
    }
    let mut db = resvg::usvg::fontdb::Database::new();
    db.load_font_data(bytes.to_vec());
    let valid = db.faces().next().is_some();
    valid
}
fn definitions(mut read: impl FnMut(&str) -> Option<Vec<u8>>) -> egui::FontDefinitions {
    let mut fonts = egui::FontDefinitions::default();
    let native = CJK_PATHS
        .iter()
        .find_map(|path| read(path).filter(|bytes| usable_font(bytes)));
    if let Some(bytes) = native {
        fonts.font_data.insert(
            "system-cjk".into(),
            Arc::new(egui::FontData::from_owned(bytes)),
        );
    }
    fonts.font_data.insert(
        "bundled-cjk".into(),
        Arc::new(egui::FontData::from_static(BUNDLED_CJK)),
    );
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        let list = fonts.families.entry(family).or_default();
        if fonts.font_data.contains_key("system-cjk") {
            list.push("system-cjk".into());
        }
        list.push("bundled-cjk".into());
    }
    // An actual Latin font is optional; egui's bundled Sans and Hack already differ.
    for (name, paths, family) in [
        (
            "system-sans",
            [
                "C:/Windows/Fonts/segoeui.ttf",
                "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            ],
            egui::FontFamily::Proportional,
        ),
        (
            "system-mono",
            [
                "C:/Windows/Fonts/consola.ttf",
                "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
            ],
            egui::FontFamily::Monospace,
        ),
    ] {
        if let Some(bytes) = paths
            .iter()
            .find_map(|path| read(path).filter(|b| usable_font(b)))
        {
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
    fonts
}
pub(super) fn install_cjk_fonts(ctx: &egui::Context) {
    let key = egui::Id::new("worldedit.fonts.v2");
    if ctx.data(|data| data.get_temp::<bool>(key)) == Some(true) {
        return;
    }
    static FONTS: OnceLock<egui::FontDefinitions> = OnceLock::new();
    let fonts = FONTS.get_or_init(|| {
        #[cfg(not(target_arch = "wasm32"))]
        {
            definitions(|path| std::fs::read(path).ok())
        }
        #[cfg(target_arch = "wasm32")]
        {
            definitions(|_| None)
        }
    });
    ctx.set_fonts(fonts.clone());
    ctx.data_mut(|data| data.insert_temp(key, true));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_or_broken_system_fonts_have_real_bundled_cjk_and_distinct_latin_families() {
        for corrupt in [false, true] {
            let fonts = definitions(|_| corrupt.then(|| b"corrupt font".to_vec()));
            assert!(!fonts.font_data.contains_key("system-cjk"));
            assert_eq!(fonts.font_data["bundled-cjk"].font.len(), BUNDLED_CJK.len());
            let sans = &fonts.families[&egui::FontFamily::Proportional];
            let mono = &fonts.families[&egui::FontFamily::Monospace];
            assert_ne!(sans[0], mono[0]);
            assert_eq!(sans.last().unwrap(), "bundled-cjk");
            assert_eq!(mono.last().unwrap(), "bundled-cjk");
            let ctx = egui::Context::default();
            ctx.set_fonts(fonts);
            let _ = ctx.run(Default::default(), |ctx| {
                ctx.fonts(|fonts| {
                    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
                        assert!(fonts.has_glyph(&egui::FontId::new(16.0, family), '世'));
                    }
                });
            });
        }
    }
    #[test]
    fn font_validator_rejects_broken_bytes_and_accepts_bundled_font() {
        assert!(!usable_font(b"not a font"));
        assert!(usable_font(BUNDLED_CJK));
    }
}
