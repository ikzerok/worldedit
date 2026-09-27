use super::filters::relative_path;
use super::*;
use egui::{RichText, Ui};
use worldline_core::queries::{CatalogQueryMatch, TodoItem, TodoKind};
pub(super) fn render_match(
    ui: &mut Ui,
    app: &WorldeditApp,
    item: &CatalogQueryMatch,
    action: &mut Action,
) {
    let object = app
        .snapshot
        .as_ref()
        .and_then(|snapshot| snapshot.result.analysis.catalog.object(&item.target));
    let label = object.map_or_else(
        || format!("{} · {}", item.target.kind, item.target.id),
        |object| {
            format!(
                "{} · {} · {}",
                super::super::catalog::kind_label(&item.target.kind),
                object.display,
                item.target.id
            )
        },
    );
    ui.group(|ui| {
        ui.horizontal_wrapped(|ui| {
            if ui.button(label).clicked() {
                *action = Action::Navigate(item.target.clone());
            }
            if ui.button("定位来源").clicked() {
                *action = Action::Jump(item.source.file.clone(), item.source.line, 1);
            }
            ui.label(format!(
                "{}:{}",
                relative_path(&app.project.root, &item.source.file),
                item.source.line
            ));
        });
        for reason in &item.reasons {
            ui.label(RichText::new(reason).small().weak());
        }
    });
}

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
