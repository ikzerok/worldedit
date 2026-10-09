use super::*;
use egui::RichText;
use worldline_core::project::reconciliation::ReconciliationFile;

impl ConflictView {
    pub(super) fn render(&mut self, ctx: &egui::Context, drafts: &[&str]) -> Option<Action> {
        let _composition = self.composition.scope(ctx);
        let ime = self.composition.blocks_actions();
        let mut action = None;
        let viewport = ctx.screen_rect().shrink(12.0);
        let frame = egui::Frame::popup(&ctx.style());
        let margin = frame.total_margin().sum();
        let width = (viewport.width() - margin.x).clamp(120.0, 1080.0);
        let height = (viewport.height().min(760.0) - margin.y).max(100.0);
        egui::Modal::new(egui::Id::new("external-conflicts")).frame(frame).show(ctx, |ui| {
            ui.set_width(width);
            // Modal默认可用尺寸很小；只设ScrollArea上限不会为材料保留空间。
            // 先给面板分配视口内高度，再扣掉真实标题、换行动作和分隔线。
            ui.set_height(height);
            let top = ui.cursor().top();
            ui.heading("外部改稿 · 核对后续写");
            ui.horizontal_wrapped(|ui| {
                let close = ui.button("关闭并保留");
                if self.request_focus && !ime { close.request_focus(); self.request_focus = false; }
                if close.clicked() { action = Some(Action::Close); }
                if ui.add_enabled(self.job.is_none() && jobs::Job::available() && self.session.is_some(), egui::Button::new("预览候选")).clicked() {
                    action = Some(Action::Preview);
                }
                let valid = self.preview.as_ref().is_some_and(|plan| plan.can_apply && plan.request == self.request());
                if ui.add_enabled(self.job.is_none() && jobs::Job::available() && valid && drafts.is_empty(), theme::primary("采纳到内存")).clicked() {
                    action = Some(Action::Apply);
                }
                if self.job.is_some() && ui.button("停止检查").clicked() { action = Some(Action::CancelJob); }
            });
            ui.separator();
            let controls_height = ui.cursor().top() - top;
            let material_height = (height - controls_height - ui.spacing().item_spacing.y).max(32.0);
            egui::ScrollArea::vertical().id_salt("reconciliation-body")
                .min_scrolled_height(material_height)
                .max_height(material_height)
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.set_max_width(width);
                    ui.label(theme::muted("三方原文 → 明确候选 → 完整预览 → 采纳。保存另行执行。"));
                    ui.label(theme::muted("采纳将旧代次撤销历史作废，并提供一个可撤销的采纳步骤。"));
                    if !drafts.is_empty() { ui.colored_label(GOLD(), format!("采纳前需处理未应用输入：{}；当前输入保留", drafts.join("、"))); }
                    if let Some(error) = &self.error { ui.colored_label(ERROR(), error); }
                    if let Some(notice) = &self.notice { ui.colored_label(GOLD(), notice); }
                    if self.job.is_some() { ui.horizontal(|ui| { ui.spinner(); ui.label("正在完整核验候选与磁盘…"); }); }
                    if ui.add_enabled(self.job.is_none() && jobs::Job::available(), egui::Button::new("重新捕获外改…")).clicked() {
                        self.recapture_confirm = true;
                    }
                    if self.recapture_confirm {
                        ui.label("保留手工原文，清除旧侧选择及预览；请按新三方材料重新核对");
                        ui.horizontal_wrapped(|ui| {
                            if ui.button("确认重新捕获").clicked() { action = Some(Action::Recapture); }
                            if ui.button("继续当前候选").clicked() { self.recapture_confirm = false; }
                        });
                    }
                    let Some(session) = &self.session else { return; };
                    for blocker in &session.blockers { ui.colored_label(ERROR(), blocker); }
                    if session.files.is_empty() { ui.label("当前没有普通外改冲突"); return; }
                    self.selected = self.selected.min(session.files.len() - 1);
                    egui::ComboBox::from_id_salt("reconciliation-file")
                        .selected_text(format!("文件 {} / {}", self.selected + 1, session.files.len()))
                        .width(width.min(440.0)).show_ui(ui, |ui| {
                            ui.set_max_width(width - 20.0);
                            for (index, file) in session.files.iter().enumerate() {
                                ui.selectable_value(&mut self.selected, index, file.path.display().to_string());
                            }
                        });
                    let file = &session.files[self.selected];
                    ui.add(egui::Label::new(RichText::new(file.path.display().to_string()).strong()).wrap());
                    if let Some(reason) = &file.protected_reason {
                        ui.colored_label(ERROR(), format!("受保护：{reason}"));
                        ui.label("可选择复制完整文本或原始十六进制；关闭后另存工程副本。保存事务恢复请使用检查点历史的救援入口。");
                    }
                    if width >= 900.0 {
                        ui.columns(3, |columns| {
                            show_side(&mut columns[0], "保存基线", file.baseline.as_deref());
                            show_side(&mut columns[1], "本地缓冲", file.local.as_deref());
                            show_side(&mut columns[2], "捕获磁盘", file.disk.as_deref());
                        });
                    } else {
                        ui.horizontal_wrapped(|ui| {
                            for (index, label) in ["保存基线", "本地缓冲", "捕获磁盘"].iter().enumerate() {
                                ui.selectable_value(&mut self.side, index, *label);
                            }
                        });
                        let bytes = [file.baseline.as_deref(), file.local.as_deref(), file.disk.as_deref()][self.side.min(2)];
                        show_side(ui, ["保存基线", "本地缓冲", "捕获磁盘"][self.side.min(2)], bytes);
                    }
                    let draft = self.drafts.entry(file.path.clone()).or_default();
                    if edit_candidate(ui, file, draft, self.job.is_none(), &self.workspace_root) { self.preview = None; }
                    if ui.checkbox(&mut self.allow_incomplete_source, "明确保留为未完成源码（完整显示错误，不可运行）").changed() {
                        self.preview = None;
                    }
                    for (path, retained) in &self.drafts {
                        if !retained.manual.is_empty() && !session.files.iter().any(|file| &file.path == path) {
                            ui.collapsing(format!("保留的旧手工稿 · {}", path.display()), |ui| {
                                ui.label("该路径已不在本次冲突中；原输入保留，可选择复制");
                                show_side(ui, "原手工输入", Some(retained.manual.as_bytes()));
                            });
                        }
                    }
                    if let Some(plan) = &self.preview {
                        ui.separator();
                        ui.heading("完整候选核对");
                        ui.label(format!("{} 文件 · {} 未解决 · {} 条问题 · {}", plan.files.len(), plan.unresolved, plan.problems.entries.len(), if plan.can_apply { "可采纳到内存" } else { "尚不能采纳" }));
                        if plan.source_has_errors { ui.colored_label(GOLD(), "源码未完成：候选不能当作可运行版本"); }
                        for blocker in &plan.blockers { ui.colored_label(ERROR(), blocker); }
                        if !plan.problems.complete { ui.label(format!("诊断覆盖限制：{}", plan.problems.reasons.join("、"))); }
                        for entry in &plan.problems.entries {
                            ui.label(format!("{:?} · {} · {} · {}", entry.severity, entry.code, entry.primary.path.as_deref().unwrap_or("工程"), entry.message));
                        }
                        for item in &plan.files {
                            ui.collapsing(format!("候选全文 · {}", item.path.display()), |ui| show_side(ui, "已审阅结果", item.result.as_deref()));
                        }
                    }
                });
        });
        if ime {
            None
        } else {
            action
        }
    }
}

fn edit_candidate(
    ui: &mut egui::Ui,
    file: &ReconciliationFile,
    draft: &mut Draft,
    enabled: bool,
    root: &std::path::Path,
) -> bool {
    let mut changed = false;
    ui.separator();
    ui.label(RichText::new("本文件候选 · 不会自动选择").strong());
    ui.add_enabled_ui(enabled && file.protected_reason.is_none(), |ui| {
        ui.horizontal_wrapped(|ui| {
            for (label, choice) in [
                ("选基线", ReconciliationChoice::Baseline),
                ("选本地", ReconciliationChoice::Local),
                ("选磁盘", ReconciliationChoice::Disk),
                ("明确删除", ReconciliationChoice::Delete),
            ] {
                if ui
                    .selectable_label(draft.choice.as_ref() == Some(&choice), label)
                    .clicked()
                {
                    draft.choice = Some(choice);
                    changed = true;
                }
            }
            if ui
                .selectable_label(
                    matches!(draft.choice, Some(ReconciliationChoice::Manual { .. })),
                    "手工全文",
                )
                .clicked()
            {
                draft.choice = Some(ReconciliationChoice::Manual {
                    text: String::new(),
                });
                changed = true;
            }
        });
        if matches!(draft.choice, Some(ReconciliationChoice::Manual { .. })) {
            ui.horizontal_wrapped(|ui| {
                for (label, bytes) in [
                    ("载入本地", file.local.as_deref()),
                    ("载入磁盘", file.disk.as_deref()),
                    ("载入基线", file.baseline.as_deref()),
                ] {
                    if ui
                        .add_enabled(draft.manual.is_empty(), egui::Button::new(label))
                        .on_hover_text("只填入空手工候选；已有输入不会被此按钮替换")
                        .clicked()
                    {
                        if let Some(text) = bytes.and_then(|b| std::str::from_utf8(b).ok()) {
                            draft.manual = text.into();
                            changed = true;
                        }
                    }
                }
            });
            changed |= ui
                .add(
                    egui::TextEdit::multiline(&mut draft.manual)
                        .id(egui::Id::new(("reconciliation-manual", root, &file.path)))
                        .font(egui::TextStyle::Monospace)
                        .desired_width(f32::INFINITY)
                        .desired_rows(8),
                )
                .changed();
        } else {
            let result = match draft.choice {
                Some(ReconciliationChoice::Baseline) => file.baseline.as_deref(),
                Some(ReconciliationChoice::Local) => file.local.as_deref(),
                Some(ReconciliationChoice::Disk) => file.disk.as_deref(),
                Some(ReconciliationChoice::Delete) => None,
                _ => {
                    ui.label(theme::muted("尚未选择"));
                    return;
                }
            };
            show_side(ui, "当前明确候选", result);
        }
    });
    changed
}

pub(super) fn show_side(ui: &mut egui::Ui, title: &str, bytes: Option<&[u8]>) {
    ui.label(RichText::new(title).strong());
    match bytes {
        None => {
            ui.colored_label(GOLD(), "缺失（不是空文件）");
        }
        Some(bytes) => {
            ui.label(theme::muted(format!(
                "{} 字节{}",
                bytes.len(),
                if bytes.is_empty() {
                    " · 空文件"
                } else {
                    ""
                }
            )));
            let invalid;
            let mut text = match std::str::from_utf8(bytes) {
                Ok(text) => text,
                Err(_) => {
                    ui.colored_label(ERROR(), "非UTF-8，原字节全部保留；分段查看十六进制");
                    let pages = bytes.len().div_ceil(512).max(1);
                    let id = ui.make_persistent_id(("reconciliation-hex-page", title));
                    let mut page = ui
                        .data(|data| data.get_temp::<usize>(id).unwrap_or(0))
                        .min(pages - 1);
                    ui.horizontal_wrapped(|ui| {
                        if ui
                            .add_enabled(page > 0, egui::Button::new("上一段"))
                            .clicked()
                        {
                            page -= 1;
                        }
                        ui.label(format!("{} / {} 段 · 每段512字节", page + 1, pages));
                        if ui
                            .add_enabled(page + 1 < pages, egui::Button::new("下一段"))
                            .clicked()
                        {
                            page += 1;
                        }
                    });
                    ui.data_mut(|data| data.insert_temp(id, page));
                    let start = page * 512;
                    invalid = bytes[start..bytes.len().min(start + 512)]
                        .iter()
                        .map(|byte| format!("{byte:02x} "))
                        .collect::<String>();
                    &invalid
                }
            };
            egui::ScrollArea::vertical()
                .id_salt(("reconciliation-side", title))
                .max_height(220.0)
                .show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut text)
                            .font(egui::TextStyle::Monospace)
                            .desired_rows(6)
                            .desired_width(f32::INFINITY),
                    );
                });
        }
    }
}
