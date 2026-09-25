//! 跨视图稳定 ID 重命名：先预览，再按 core 基线整批应用。
use super::authoring_forms::RenameForm;
use super::WorldeditApp;
use crate::theme;
use worldline_core::TargetRef;

impl WorldeditApp {
    pub(super) fn plan_target_rename(&mut self, target: TargetRef) {
        self.rename_form = Some(RenameForm::open(&self.project, self.version, target));
    }

    pub(super) fn target_rename_window(&mut self, ctx: &egui::Context) {
        let Some(mut form) = self.rename_form.take() else {
            return;
        };
        let mut open = true;
        let mut applied = false;
        egui::Window::new("跨视图更改稳定 ID")
            .id(egui::Id::new("target-rename"))
            .open(&mut open)
            .default_width(680.0)
            .resizable(true)
            .vscroll(true)
            .show(ctx, |ui| {
                ui.heading(format!("{}:{}", form.target.kind, form.target.id));
                ui.label("显示名修改不会断链；这里改变的是稳定 ID，会同时更新明确的源码、地图和共享网络引用。");
                ui.label(theme::muted("首版只对 entity / relation 开放；失败时所有文件保持原样。"));
                ui.label("新的稳定 ID");
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut form.new_id)
                            .desired_width(f32::INFINITY),
                    )
                    .changed()
                {
                    form.plan = None;
                }
                let current = form.guard.is_current(&self.project, self.version);
                if !current {
                    ui.colored_label(
                        theme::GOLD,
                        "工程已变化，旧输入/预览已失效；请关闭后重新打开。",
                    );
                }
                if ui
                    .add_enabled(current, egui::Button::new("预览重命名"))
                    .clicked()
                {
                    if let Err(error) = form.preview(&self.project, self.version) {
                        self.io_error = Some(error);
                    } else {
                        self.io_error = None;
                    }
                }
                if let Some(plan) = &form.plan {
                    ui.separator();
                    ui.label(format!(
                        "确认到 {} 个明确替换点，涉及 {} 个文件：",
                        plan.explicit_references,
                        plan.changes.len()
                    ));
                    for change in &plan.changes {
                        ui.label(format!(
                            "{} · {} 处 · {}",
                            change.kind,
                            change.reference_count,
                            change
                                .path
                                .strip_prefix(&self.project.root)
                                .unwrap_or(&change.path)
                                .display()
                        ));
                    }
                    ui.colored_label(
                        theme::GOLD,
                        "这不是改显示名。应用后旧 ID 将不存在；可用应用级撤销恢复。",
                    );
                    if ui
                        .add_enabled(current, theme::primary("应用跨视图重命名"))
                        .clicked()
                    {
                        let new_target = TargetRef::new(&form.target.kind, form.new_id.trim());
                        let before = self.project.clone();
                        let result = form.apply(&mut self.project, self.version);
                        applied = self.finish_content_command(
                            before,
                            result,
                            "稳定 ID 与明确引用已整批更新；保存全部后写入磁盘",
                        );
                        if applied {
                            self.catalog_target = Some(new_target.clone());
                            self.network_selected = Some(new_target);
                        }
                    }
                }
                if let Some(error) = &self.io_error {
                    ui.separator();
                    ui.colored_label(theme::ERROR, error);
                }
            });
        if open && !applied {
            self.rename_form = Some(form);
        }
    }
}
