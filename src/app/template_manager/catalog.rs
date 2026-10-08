use super::*;

pub(super) fn show(
    ui: &mut egui::Ui,
    state: &mut ManagerState,
    index: &ProjectTemplateIndex,
) -> Option<Action> {
    let mut action = None;
    ui.heading(format!("内置模板 · {} 项", index.builtins.len()));
    egui::ScrollArea::vertical()
        .id_salt("template-catalog-builtins")
        .max_height(220.0)
        .show(ui, |ui| {
            for template in &index.builtins {
                if ui
                    .selectable_label(
                        state.selected_id.as_deref() == Some(&template.id),
                        format!("{} · {}", template.title, template.id),
                    )
                    .clicked()
                {
                    action = Some(Action::Select(template.id.clone()));
                }
            }
        });
    ui.separator();
    ui.heading(format!("工程模板 · {} 项", index.projects.len()));
    egui::ScrollArea::vertical()
        .id_salt("template-catalog-projects")
        .max_height(280.0)
        .show(ui, |ui| {
            for (id, document) in &index.projects {
                let title = document
                    .template
                    .as_ref()
                    .map(|template| template.title.as_str())
                    .unwrap_or("无法解析的模板");
                let suffix = if document.read_only { " · 只读" } else { "" };
                if ui
                    .selectable_label(
                        state.selected_id.as_deref() == Some(id),
                        format!("{title} · {id}{suffix}"),
                    )
                    .clicked()
                {
                    action = Some(Action::Select(id.clone()));
                }
            }
        });
    ui.add_space(8.0);
    ui.label(theme::muted(
        "切换模板会重置试填值。未应用定义或属性输入会先询问。",
    ));
    action
}
pub(super) fn details(ui: &mut egui::Ui, state: &ManagerState, index: &ProjectTemplateIndex) {
    let Some(id) = state.selected_id.as_deref() else {
        return;
    };
    if let Some(document) = index.projects.get(id) {
        ui.label(
            egui::RichText::new(
                document
                    .template
                    .as_ref()
                    .map(|template| template.title.as_str())
                    .unwrap_or(id),
            )
            .strong(),
        );
        let version = document
            .source_document
            .as_ref()
            .and_then(|value| value.get("schema_version"))
            .and_then(serde_json::Value::as_u64);
        ui.label(theme::muted(format!(
            "来源：工程 · {}",
            version
                .map(|version| format!("schema v{version}"))
                .unwrap_or_else(|| "版本未知".into())
        )));
        if document.read_only {
            ui.colored_label(
                theme::GOLD(),
                "只读模板：版本、必需能力或原文不受支持。原文保持在工程中。",
            );
        }
        for diagnostic in &document.diagnostics {
            ui.colored_label(
                theme::GOLD(),
                format!("{} · {}", diagnostic.code, diagnostic.message),
            );
        }
    } else if let Some(template) = index.builtins.iter().find(|template| template.id == id) {
        ui.label(egui::RichText::new(&template.title).strong());
        ui.label(theme::muted(format!(
            "来源：内置 · schema v{}",
            template.schema_version
        )));
        ui.label(theme::muted(format!(
            "适用对象：{}{} · {} 个提示字段",
            template.applies_to.kind,
            template
                .applies_to
                .entity_type
                .as_ref()
                .map(|kind| format!(" · {kind}"))
                .unwrap_or_default(),
            template.fields.len()
        )));
        ui.label("内置模板只读；复制后可以可视编辑。");
    }
}
