use super::filters::relative_path;
use super::*;
use egui::{RichText, Ui};
use worldline_core::queries::{TodoItem, TodoKind};
pub(super) fn render_todo(ui: &mut Ui, app: &WorldeditApp, item: &TodoItem, action: &mut Action) {
    ui.group(|ui| {
        ui.label(RichText::new(&item.reason).strong());
        ui.horizontal_wrapped(|ui| {
            if ui
                .button(format!("定位{}来源", todo_kind_label(item.kind)))
                .clicked()
            {
                *action = Action::Jump(
                    item.source.file.clone(),
                    item.source.line,
                    item.column.unwrap_or(1),
                );
            }
            if let Some(target) = &item.related_target {
                if ui.button("查看引用对象").clicked() {
                    *action = Action::Navigate(target.clone());
                }
            }
            ui.label(format!(
                "{}:{}{}",
                relative_path(&app.project.root, &item.source.file),
                item.source.line,
                item.column
                    .map_or(String::new(), |column| format!(" · 列 {column}"))
            ));
        });
    });
}
pub(super) fn todo_kind_label(kind: TodoKind) -> &'static str {
    match kind {
        TodoKind::BrokenLink => "断链",
        TodoKind::EntryToCreate => "待建资料",
        TodoKind::DetachedComment => "失锚批注",
        TodoKind::OpenProposal => "待审提案",
    }
}
