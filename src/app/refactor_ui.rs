//! 跨视图稳定 ID 重命名：先预览，再按 core 基线整批应用。
use super::authoring_forms::RenameForm;
use super::WorldeditApp;
use crate::theme;
use worldline_core::refactor::RenamePlan;
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
        let mut edit_display = None;
        let title = egui::RichText::new("跨视图更改稳定 ID").heading();
        let style = ctx.style();
        let frame = egui::Frame::window(&style);
        let title_height = ctx
            .fonts(|fonts| title.font_height(fonts, &style))
            .max(style.spacing.interact_size.y)
            + frame.inner_margin.sum().y;
        let chrome_height = title_height + frame.stroke.width + frame.total_margin().sum().y;
        egui::Window::new(title)
            .id(egui::Id::new("target-rename"))
            .open(&mut open)
            .frame(frame)
            .constrain_to(ctx.available_rect())
            .default_width(680.0)
            .default_height(620.0)
            .max_height((ctx.available_rect().height() - chrome_height - 8.0).max(120.0))
            .resizable(true)
            .vscroll(true)
            .show(ctx, |ui| {
                ui.heading(format!("{}:{}", form.target.kind, form.target.id));
                ui.label("显示名修改不会断链；这里改变的是稳定 ID，会同时更新明确的源码、地图和共享网络引用。");
                ui.label(theme::muted("完整计划整批应用；普通文字保持不变，失败时所有文件保持原样。"));
                if form.target.kind == "entity"
                    && ui.button("改显示名（保留 ID）").clicked()
                {
                    edit_display = Some(form.target.id.clone());
                }
                ui.label("新的稳定 ID");
                if ui
                    .add(
                        egui::TextEdit::singleline(&mut form.new_id)
                            .desired_width(f32::INFINITY),
                    )
                    .changed()
                {
                    form.plan = None;
                    self.io_error = None;
                }
                let current = form.guard.is_current(&self.project, self.version);
                if !current {
                    ui.colored_label(
                        theme::GOLD(),
                        "工程已变化，旧输入/预览已失效；请关闭后重新打开。",
                    );
                }
                if crate::theme::add_enabled(ui, current, egui::Button::new("预览重命名"))
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
                        "确认到 {} 个明确身份引用，涉及 {} 个文件：",
                        plan.explicit_references,
                        plan.changes.len()
                    ));
                    rename_preview(ui, plan, &self.project.root);
                    ui.colored_label(
                        theme::GOLD(),
                        "这不是改显示名。应用后旧 ID 将不存在；可用应用级撤销恢复。",
                    );
                    if crate::theme::add_enabled(ui, current, theme::primary("应用跨视图重命名"))
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
                    ui.colored_label(theme::ERROR(), error);
                }
            });
        if open && !applied {
            self.rename_form = Some(form);
        }
        if let Some(id) = edit_display {
            if !self.prevent_replacing_draft("实体资料") {
                self.edit_entity(Some(&id));
                if self
                    .entity_editor
                    .as_ref()
                    .is_some_and(|form| form.draft.id == id)
                {
                    self.rename_form = None;
                    self.io_error = None;
                }
            }
        }
    }
}

fn rename_preview(ui: &mut egui::Ui, plan: &RenamePlan, root: &std::path::Path) {
    ui.label(format!("内容基线：{}", plan.content_baseline));
    let fingerprint = if plan.runtime_fingerprint_before == plan.runtime_fingerprint_after {
        format!("运行指纹保持不变：{:016x}", plan.runtime_fingerprint_before)
    } else {
        format!(
            "运行指纹变化：{:016x} → {:016x}；旧存档与检查点不匹配，入口重放须重新验证",
            plan.runtime_fingerprint_before, plan.runtime_fingerprint_after
        )
    };
    ui.label(fingerprint);
    ui.label(theme::muted(
        "以下是同一计划的真实前后内容；所有明确引用一起提交，不能单独取消某一处。",
    ));
    // 使用窗口唯一滚动区，避免小视口的内外滚动区截住滚轮，使末尾确认动作不可达。
    for change in &plan.changes {
        ui.separator();
        ui.label(
            egui::RichText::new(format!(
                "{} · {} 处 · {}",
                change
                    .path
                    .strip_prefix(root)
                    .unwrap_or(&change.path)
                    .display(),
                change.reference_count,
                if change.kind == "source" {
                    "源码"
                } else {
                    "展示文档"
                }
            ))
            .strong(),
        );
        for (index, occurrence) in change.occurrences.iter().enumerate() {
            ui.push_id((change.path.clone(), index), |ui| {
                if occurrence.field.as_deref() == Some("source.syntax") {
                    ui.label(format!(
                        "第 {} 行 · 语法行整体改写（可包含多个身份引用）",
                        occurrence.line
                    ));
                } else {
                    ui.label(format!(
                        "第 {} 行 · {} · {} → {}",
                        occurrence.line,
                        occurrence.field.as_deref().unwrap_or("身份引用"),
                        occurrence.before_token,
                        occurrence.after_token
                    ));
                }
                ui.label(theme::muted(format!(
                    "UTF-8 字节 {}..{} → {}..{}",
                    occurrence.before_range.start,
                    occurrence.before_range.end,
                    occurrence.after_range.start,
                    occurrence.after_range.end
                )));
                ui.label("修改前");
                ui.add(egui::Label::new(&occurrence.before_context).wrap());
                ui.label("修改后");
                ui.add(egui::Label::new(&occurrence.after_context).wrap());
            });
        }
    }
}
