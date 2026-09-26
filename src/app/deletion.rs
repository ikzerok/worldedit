//! 内容删除确认；影响计划和写入许可都来自 core。
use super::authoring_forms::DeleteForm;
use super::WorldeditApp;
use crate::theme;
use worldline_core::project::Project;
use worldline_core::TargetRef;

impl WorldeditApp {
    pub(super) fn finish_content_command(
        &mut self,
        before: Project,
        result: Result<(), String>,
        label: &str,
    ) -> bool {
        match result {
            Ok(()) => {
                self.remember(before);
                self.recompile();
                self.io_error = None;
                self.message = Some(label.into());
                true
            }
            Err(error) => {
                self.io_error = Some(error);
                false
            }
        }
    }
    pub(super) fn plan_content_deletion(&mut self, target: TargetRef) {
        self.delete_form = Some(DeleteForm::open(&self.project, self.version, target));
    }
    pub(super) fn content_deletion_window(&mut self, ctx: &egui::Context) {
        let Some(mut form) = self.delete_form.take() else {
            return;
        };
        let mut open = true;
        let mut applied = false;
        egui::Window::new("删除内容前检查引用")
            .id(egui::Id::new("content-delete-confirm"))
            .open(&mut open)
            .default_width(640.0)
            .resizable(true)
            .vscroll(true)
            .show(ctx, |ui| {
                ui.heading(format!(
                    "{}:{}",
                    form.impact.target.kind, form.impact.target.id
                ));
                ui.label("这会删除内容声明，不是仅隐藏图形。存在引用或检查不完整时不会执行删除。");
                if !form.impact.complete {
                    ui.colored_label(theme::ERROR, "引用检查不完整，请先修复诊断。");
                    for diagnostic in &form.impact.diagnostics {
                        ui.label(format!(
                            "{} · {} · {}",
                            diagnostic.code, diagnostic.file, diagnostic.message
                        ));
                    }
                }
                for reference in &form.impact.content_references {
                    ui.label(format!(
                        "正文引用 · {}:{} · {}",
                        reference.file, reference.line, reference.kind
                    ));
                }
                for reference in &form.impact.map_placements {
                    ui.label(format!(
                        "地图标记 · {} / {}",
                        reference.map_id, reference.placement_id
                    ));
                }
                for reference in &form.impact.map_scopes {
                    ui.label(format!(
                        "地图范围 · {} / {}",
                        reference.map_id, reference.placement_id
                    ));
                }
                for reference in &form.impact.graph_views {
                    ui.label(format!(
                        "共享布局引用 · {} / {}",
                        reference.view_id, reference.field
                    ));
                    ui.label(theme::muted(&reference.file));
                }
                for reference in &form.impact.comments {
                    ui.label(format!(
                        "批注引用 · {} / {}",
                        reference.comment_id, reference.anchor
                    ));
                    ui.label(theme::muted(&reference.file));
                }
                if form.impact.can_delete() {
                    ui.label("已完成引用检查；没有需要解除的引用。");
                }
                let current = form.guard.is_current(&self.project, self.version);
                if !current {
                    ui.colored_label(
                        theme::GOLD,
                        "工程已变化，旧影响计划失效。请关闭后重新检查。",
                    );
                }
                ui.checkbox(&mut form.confirmed, "我确认删除此内容，而不是仅隐藏显示");
                if ui
                    .add_enabled(
                        current && form.confirmed && form.impact.can_delete(),
                        egui::Button::new("确认删除内容"),
                    )
                    .clicked()
                {
                    let before = self.project.clone();
                    let result = form.apply(&mut self.project, self.version);
                    applied = self.finish_content_command(
                        before,
                        result,
                        "内容已删除；可撤销，保存全部后写入磁盘",
                    );
                    if applied {
                        self.entity_editor = None;
                        self.relation_editor = None;
                    }
                }
                if let Some(error) = &self.io_error {
                    ui.colored_label(theme::ERROR, error);
                }
            });
        if open && !applied {
            self.delete_form = Some(form);
        }
    }
}
