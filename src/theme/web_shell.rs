//! Canvas 外壳只使用解析后的白名单角色，不注入用户 CSS。
use super::ResolvedTheme;
fn hex(color: egui::Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", color.r(), color.g(), color.b())
}
pub(super) fn synchronize(theme: &ResolvedTheme) {
    if let Some(root) = web_sys::window()
        .and_then(|w| w.document())
        .and_then(|d| d.document_element())
    {
        let style = format!(
            "--workspace:{};--text:{};--accent:{};color-scheme:{}",
            hex(theme.colors.workspace),
            hex(theme.colors.text),
            hex(theme.colors.accent),
            if theme.light { "light" } else { "dark" }
        );
        let _ = root.set_attribute("style", &style);
    }
}
