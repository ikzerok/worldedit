//! 文件路径整理只消费 core 的完整计划；不在界面识别或替换引用。
use super::{Tab, WorldeditApp};
use crate::theme;
use std::path::{Path, PathBuf};
use worldline_core::source_lifecycle::{SourceLifecyclePlan, SourceLifecycleRequest};

pub(super) struct SourceMoveForm {
    pub root: PathBuf,
    pub source: PathBuf,
    pub destination: String,
    pub plan: Option<SourceLifecyclePlan>,
    error: Option<String>,
}

impl SourceMoveForm {
    fn request(&self) -> SourceLifecycleRequest {
        SourceLifecycleRequest::Move {
            from: self
                .source
                .strip_prefix(&self.root)
                .unwrap_or(&self.source)
                .into(),
            to: PathBuf::from(self.destination.trim()),
        }
    }
    pub(super) fn changed(&self) -> bool {
        Path::new(self.destination.trim())
            != self.source.strip_prefix(&self.root).unwrap_or(&self.source)
    }
}

impl WorldeditApp {
    fn source_move_blocker(&self) -> Option<String> {
        let inputs: Vec<_> = self
            .unapplied_export_inputs()
            .into_iter()
            .filter(|input| input.kind != "源码路径")
            .map(|input| format!("{} · {}", input.kind, input.source))
            .collect();
        (!inputs.is_empty()).then(|| {
            format!(
                "请先处理未应用输入，再整理源码路径；当前输入全部保留：{}",
                inputs.join("；")
            )
        })
    }

    pub(super) fn begin_source_move(&mut self, source: PathBuf) {
        if self.prevent_replacing_draft("源码路径") {
            return;
        }
        if let Some(message) = self.source_move_blocker() {
            self.message = Some(message);
            return;
        }
        if source == self.project.entry || !self.project.documents.contains_key(&source) {
            self.message =
                Some("只支持已跟踪的非入口 .wl 源码；入口、目录和普通附件不能移动。".into());
            return;
        }
        self.source_move_form = Some(SourceMoveForm {
            root: self.project.root.clone(),
            destination: source
                .strip_prefix(&self.project.root)
                .unwrap_or(&source)
                .to_string_lossy()
                .into_owned(),
            source,
            plan: None,
            error: None,
        });
    }

    fn preview_source_move(&self, form: &mut SourceMoveForm) {
        form.plan = None;
        form.error = if form.root != self.project.root {
            Some("工作区已切换，旧路径输入保留；请取消后在当前工作区重新打开。".into())
        } else {
            self.source_move_blocker()
        };
        if form.error.is_some() {
            return;
        }
        match self.project.preview_source_lifecycle(&form.request()) {
            Ok(plan) => form.plan = Some(plan),
            Err(error) => form.error = Some(error),
        }
    }

    fn apply_source_move(&mut self, form: &mut SourceMoveForm) -> bool {
        if form.root != self.project.root {
            form.error = Some("工作区已切换，未应用旧计划。".into());
            return false;
        }
        if let Some(message) = self.source_move_blocker() {
            form.error = Some(message);
            return false;
        }
        let Some(plan) = &form.plan else {
            return false;
        };
        let before = self.project.clone();
        let result = self.project.apply_source_lifecycle_plan(plan);
        match result {
            Ok(applied) => {
                self.remember(before);
                if let Some(destination) = applied.destination_path {
                    for location in &mut self.personal.history {
                        if location.file == form.source {
                            location.file = destination.clone();
                        }
                    }
                    if let Some((path, _)) = &mut self.reading_return {
                        if *path == form.source {
                            *path = destination.clone();
                        }
                    }
                    self.active_file = destination;
                }
                self.manuscript.rebase_clean(&self.project);
                self.recompile();
                self.tab = Tab::Edit;
                self.io_error = None;
                self.message = Some("源码路径与正式引用已整批更新，语义与运行指纹不变；可一次撤销，保存全部后写入磁盘。".into());
                true
            }
            Err(error) => {
                form.error = Some(error);
                false
            }
        }
    }

    pub(super) fn source_move_window(&mut self, ctx: &egui::Context) {
        let Some(mut form) = self.source_move_form.take() else {
            return;
        };
        let mut open = true;
        let mut cancel = false;
        let mut apply = false;
        let mut preview = false;
        let viewport = ctx.available_rect().shrink(8.0);
        let width = (viewport.width() - 32.0).clamp(260.0, 720.0);
        egui::Window::new("安全整理源码路径")
            .id(egui::Id::new("source-move"))
            .open(&mut open).collapsible(false).resizable(true)
            .default_width(width).max_width(width)
            .max_height((viewport.height() - 50.0).max(200.0))
            .constrain_to(viewport).show(ctx, |ui| {
                ui.label(format!("原路径：{}", theme::relative_source(&form.root, &form.source)));
                ui.label("新路径（相对工作区，保留 .wl）");
                if ui.add(egui::TextEdit::singleline(&mut form.destination).desired_width(f32::INFINITY)).changed() {
                    form.plan = None;
                    form.error = None;
                }
                ui.label(theme::muted("只移动一个非入口源码。归档不自动启用；无法证明引用、资源和加载顺序等价时拒绝。"));
                ui.separator();
                egui::ScrollArea::vertical().id_salt("source-move-preview")
                    .max_height((viewport.height() - 285.0).max(100.0))
                    .auto_shrink([false, false]).show(ui, |ui| {
                        if let Some(error) = &form.error { ui.colored_label(theme::ERROR(), error); }
                        if let Some(plan) = &form.plan { draw_plan(ui, plan, &form.root); }
                        else { ui.label("预览将列出每处正式路径修改、附件解析与运行指纹证明。"); }
                    });
                ui.separator();
                if form.error.is_some() { ui.colored_label(theme::ERROR(), "未应用；完整原因见上方，输入已保留。"); }
                let current = form.plan.as_ref().is_some_and(|p| p.content_baseline == self.project.content_baseline()) && form.root == self.project.root;
                if form.plan.is_some() && !current { ui.colored_label(theme::WARNING(), "工程已变化；输入保留，请重新预览。" ); }
                ui.horizontal_wrapped(|ui| {
                    preview = ui.add_enabled(form.root == self.project.root, egui::Button::new("预览完整移动")).clicked();
                    apply = ui.add_enabled(current, theme::primary("应用完整计划")).clicked();
                    cancel = ui.button("取消路径整理").clicked();
                });
            });
        if preview {
            self.preview_source_move(&mut form);
        }
        let applied = apply && self.apply_source_move(&mut form);
        if open && !cancel && !applied {
            self.source_move_form = Some(form);
        }
    }
}

fn draw_plan(ui: &mut egui::Ui, plan: &SourceLifecyclePlan, root: &Path) {
    ui.label(format!(
        "成员身份：{} · {} 个文件变更",
        plan.membership,
        plan.changes.len()
    ));
    ui.label(format!(
        "运行指纹：{:016x} → {:016x}",
        plan.runtime_fingerprint_before, plan.runtime_fingerprint_after
    ));
    ui.label(format!(
        "默认入口：{} → {}",
        plan.entry_before, plan.entry_after
    ));
    egui::CollapsingHeader::new("语义加载顺序证明").show(ui, |ui| {
        for (before, after) in plan.load_order_before.iter().zip(&plan.load_order_after) {
            ui.label(format!("{before} → {after}"));
        }
    });
    for change in &plan.changes {
        egui::CollapsingHeader::new(format!(
            "{} → {} · {}",
            theme::relative_source(root, &change.path),
            theme::relative_source(root, &change.after_path),
            change.kind
        ))
        .default_open(true)
        .show(ui, |ui| {
            for occurrence in &change.occurrences {
                ui.label(format!(
                    "第 {} 行 · {}",
                    occurrence.line,
                    occurrence.field.as_deref().unwrap_or("正式路径")
                ));
                ui.label(format!(
                    "{} → {}",
                    occurrence.before_token, occurrence.after_token
                ));
                ui.label(theme::muted(&occurrence.before_context));
                ui.label(theme::muted(&occurrence.after_context));
            }
        });
    }
    egui::CollapsingHeader::new(format!("{} 项资源解析证明", plan.resources.len())).show(
        ui,
        |ui| {
            for item in &plan.resources {
                ui.label(format!(
                    "{} · {} → {}",
                    item.field, item.before_path, item.after_path
                ));
                ui.label(theme::muted(format!(
                    "解析到 {} · 摘要 {}",
                    theme::relative_source(root, &item.resolved_after),
                    item.content_digest
                )));
            }
        },
    );
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;
