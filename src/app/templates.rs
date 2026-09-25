//! 16 类可选内容模板的共享 UI；定义与匹配来自 worldline-core。
use crate::theme;
use egui::RichText;
use worldline_core::ast::PropertyValue;

fn add_optional_text(
    ui: &mut egui::Ui,
    values: &mut Vec<(String, PropertyValue)>,
    key: &str,
    label: &str,
) {
    if let Some((_, value)) = values.iter_mut().find(|(name, _)| name == key) {
        ui.label(RichText::new(label).strong());
        match value {
            PropertyValue::Str(text) => {
                ui.add(
                    egui::TextEdit::multiline(text)
                        .desired_rows(3)
                        .desired_width(f32::INFINITY),
                );
            }
            _ => {
                ui.label(theme::muted("此字段已有非文本自定义值；模板不会覆盖它。"));
            }
        }
    } else if ui.button(format!("＋ {label}")).clicked() {
        values.push((key.into(), PropertyValue::Str(String::new())));
    }
}
pub(super) fn template_panel(
    ui: &mut egui::Ui,
    kind: &str,
    entity_type: Option<&str>,
    values: &mut Vec<(String, PropertyValue)>,
) -> Option<String> {
    let Some(template) = worldline_core::content_templates::matching_template(kind, entity_type)
    else {
        ui.label(theme::muted(
            "当前分类没有专用模板；仍可使用正文和自定义属性。",
        ));
        return None;
    };

    let mut suggestion = None;
    egui::CollapsingHeader::new(format!("创作模板 · {}", template.title))
        .default_open(false)
        .show(ui, |ui| {
            ui.label(theme::muted(
                "模板字段全部可选；切换模板不会删除正文、既有属性或未知字段。",
            ));
            for prompt in &template.prompts {
                ui.label(format!("思考：{prompt}"));
            }
            ui.separator();
            ui.label(RichText::new("关系建议").strong());
            ui.label(theme::muted("点击只会打开关系草稿，不会自动生成事实。"));
            ui.horizontal_wrapped(|ui| {
                for label in &template.suggested_relations {
                    if ui.button(label).clicked() {
                        suggestion = Some(label.clone());
                    }
                }
            });
            ui.separator();
            for field in &template.fields {
                add_optional_text(ui, values, &field.key, &field.label);
            }
        });

    egui::CollapsingHeader::new("来源、陈述性质与创作状态").show(ui, |ui| {
        ui.label(theme::muted(
            "这些是作者资料，不把“已接受”自动解释为“客观事实”。",
        ));
        add_optional_text(ui, values, "source_note", "来源 / 依据");
        add_optional_text(
            ui,
            values,
            "statement_nature",
            "陈述性质（事实 / 传闻 / 观点等）",
        );
        add_optional_text(
            ui,
            values,
            "authoring_status",
            "创作状态（草稿 / 已接受等）",
        );
    });
    suggestion
}
