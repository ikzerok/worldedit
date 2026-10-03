//! 源码高亮只控制显示；LayoutJob文本必须逐字保留输入。
pub(super) fn job(
    source: &str,
    body_size: f32,
    line_height: f32,
    version: worldline_core::LanguageVersion,
    wrap: bool,
    width: f32,
) -> egui::text::LayoutJob {
    let mut job = crate::highlight::layout_job(source, body_size, version);
    job.wrap.max_width = if wrap { width.max(1.0) } else { f32::INFINITY };
    for section in &mut job.sections {
        section.format.line_height = Some(line_height);
    }
    job
}
