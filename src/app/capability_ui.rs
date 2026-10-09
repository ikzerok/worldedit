//! 既有语言能力的显式决定点；目录、清单候选和校验全部来自 core。
use super::WorldeditApp;
use crate::theme;
use std::collections::BTreeSet;
use worldline_core::capabilities::{
    feature_capabilities, language_capabilities, CapabilityEnablePlan, CapabilityEnableRequest,
};
use worldline_core::LanguageVersion;
mod layout;

pub(super) struct CapabilityState {
    target: LanguageVersion,
    features: BTreeSet<String>,
    plan: Option<CapabilityEnablePlan>,
    acknowledged: bool,
    error: Option<String>,
    focus_language: bool,
}

impl WorldeditApp {
    pub(super) fn open_capabilities(&mut self) {
        if self.capability_ui.is_none() {
            self.capability_ui = Some(CapabilityState {
                target: self.project.language_version_kind(),
                features: self
                    .project
                    .required_features()
                    .into_iter()
                    .filter(|id| feature_capabilities().iter().any(|item| item.id == id))
                    .collect(),
                plan: None,
                acknowledged: false,
                error: None,
                focus_language: true,
            });
        }
    }

    fn capability_blockers(&self) -> Vec<String> {
        let mut blockers: Vec<String> = self
            .unapplied_export_inputs()
            .into_iter()
            .filter(|input| !matches!(input.kind, "世界资料导入" | "本地化草稿"))
            .map(|input| format!("{} · {}", input.kind, input.source))
            .collect();
        if self.stale_form {
            blockers.push("外部刷新后仍有旧表单；请保留输入并重新打开合并".into());
        }
        if !self.project.authoring_diagnostics().is_empty() {
            blockers.push("工程清单不受支持或无效；按原始字节只读保留".into());
        }
        blockers
    }

    pub(super) fn capability_window(&mut self, ctx: &egui::Context) {
        let Some(mut state) = self.capability_ui.take() else {
            return;
        };
        let blockers = self.capability_blockers();
        let mut open = true;
        let mut cancel = false;
        let mut preview = false;
        let mut apply = false;
        let mut changed = false;
        let budget = layout::Budget::new(ctx);
        let opening = state.focus_language;
        let escape = ctx.input(|input| {
            input.raw.events.iter().any(|event| {
                matches!(
                    event,
                    egui::Event::Key {
                        key: egui::Key::Escape,
                        pressed: true,
                        ..
                    }
                )
            })
        });
        let restore_escaped_focus = escape
            && self.edit_layer_is_top("guard-capabilities")
            && !egui::Popup::is_any_open(ctx)
            && !self.ime_composing
            && !self.command_palette.ime
            && !self.command_palette.ime_frame;
        let focus = layout::FocusReveal::for_frame(ctx, opening, restore_escaped_focus);
        egui::Window::new(layout::TITLE)
            .id(egui::Id::new("language-capabilities"))
            .open(&mut open).resizable(true).frame(budget.frame)
            .constrain_to(budget.bounds).max_size(budget.inner)
            .default_size(egui::vec2(660.0, 640.0).min(budget.inner)).show(ctx, |ui| {
                let mut contents = |ui: &mut egui::Ui| {
                {
                let body = |ui: &mut egui::Ui| {
                ui.heading(format!("当前语言 {}", self.project.language_version()));
                if self.catalog_import.has_unsubmitted_work() {
                    ui.label("世界资料导入快照与列映射会保留；启用能力后须重新预览，不能沿用旧确认。");
                }
                if self.localization_ui.has_unsubmitted_work() {
                    ui.label("本地化输入会保留；启用能力后须按新源文基线重新预览。");
                }
                ui.label("保留当前设置即可继续传统作品。启用能力须先检查全文，再明确确认；打开旧工程不会自动升级。");
                let before = state.target;
                let mut selected_language = false;
                let language_id = ui.make_persistent_id(egui::Id::new("capability-language"));
                let language_was_open = egui::ComboBox::is_open(ctx, language_id);
                let language = egui::ComboBox::from_id_salt("capability-language")
                    .selected_text(format!("语言 {}", state.target.as_str()))
                    .show_ui(ui, |ui| {
                        for capability in language_capabilities() {
                            let response = ui.selectable_value(&mut state.target, capability.version,
                                capability.title);
                            focus.reveal(ui, &response);
                            if response.clicked() {
                                selected_language = true;
                                // 键盘 Enter 与鼠标选择使用相同的关闭边界。
                                ui.close();
                            }
                        }
                    });
                // 只在进入窗口或完成版本选择时移交焦点，不能每帧抢回 Tab 焦点。
                let dismissed_language = escape && language_was_open
                    && !egui::ComboBox::is_open(ctx, language.response.id);
                if opening || selected_language || dismissed_language {
                    state.focus_language = false;
                    focus.request(ui, &language.response);
                } else {
                    focus.reveal(ui, &language.response);
                }
                changed |= before != state.target;
                if let Some(capability) = language_capabilities().iter().find(|c| c.version == state.target) {
                    ui.label(capability.description);
                    ui.label(format!("此版本所需能力：{}", if capability.required_features.is_empty() { "无新增 required_features".into() } else { capability.required_features.join("、") }));
                }
                let extra = ui.collapsing("额外资料能力与兼容要求", |ui| {
                    let required = self.project.required_features();
                    for capability in feature_capabilities() {
                        let mut selected = state.features.contains(capability.id);
                        let response = crate::theme::add_enabled(ui, !required.iter().any(|id| id == capability.id),
                            egui::Checkbox::new(&mut selected, format!("{} · {}", capability.title, capability.id)));
                        focus.reveal(ui, &response);
                        if response.changed() {
                            changed = true;
                            if selected { state.features.insert(capability.id.into()); }
                            else { state.features.remove(capability.id); }
                        }
                        ui.label(theme::muted(format!("{}；最低语言 {}；依赖：{}", capability.description,
                            capability.minimum_language.as_str(), capability.dependencies.join("、"))));
                    }
                });
                focus.reveal(ui, &extra.header_response);
                if changed { state.plan = None; state.acknowledged = false; state.error = None; }
                if !blockers.is_empty() {
                    ui.colored_label(theme::WARNING(), "以下输入尚待处理，升级不会自动应用或丢弃它们：");
                    for blocker in &blockers { ui.label(blocker); }
                    ui.label("先关闭此窗口处理原输入。若旧版本不能应用表单，请先复制所需输入并明确关闭该表单，再启用能力；不会自动清稿。已应用未保存内容无需先落盘。");
                }
                if let Some(plan) = &state.plan {
                    ui.separator();
                    ui.heading(format!("语言 {} → {}", plan.current_language.as_str(), plan.target_language.as_str()));
                    ui.label(format!("原 required_features：{}", plan.required_features_before.join("、")));
                    ui.label(format!("启用后 required_features：{}", plan.required_features_after.join("、")));
                    for note in &plan.compatibility_notes { ui.label(note); }
                    if plan.fingerprint_comparison_reliable {
                        ui.label(format!("运行指纹：{:016x} → {:016x}", plan.runtime_fingerprint_before, plan.runtime_fingerprint_after));
                    } else {
                        ui.colored_label(theme::WARNING(), "存在编译错误，不能据此比较运行兼容性。");
                    }
                    ui.label(format!("关键字解释变化 {} 处", plan.keyword_changes.len()));
                    for change in &plan.keyword_changes {
                        ui.label(format!("{}:{} · {} → {}", change.file, change.line, change.before_kind, change.after_kind));
                        ui.monospace(&change.source);
                    }
                    ui.label(format!("全文候选诊断 {} 项；新增 {} 项", plan.diagnostics_after.len(), plan.new_diagnostics.len()));
                    for diagnostic in &plan.diagnostics_after {
                        ui.colored_label(if diagnostic.severity == worldline_core::Severity::Error { theme::ERROR() } else { theme::WARNING() },
                            format!("{} · {}:{} · {}", diagnostic.code, diagnostic.file, diagnostic.span.line, diagnostic.message));
                    }
                    if !plan.manifest_changed {
                        ui.label("没有需要启用的变化；保持当前设置即可。");
                    } else if !plan.can_apply {
                        ui.colored_label(theme::ERROR(), "候选未通过全文检查，不能启用或运行；原稿保持不变。");
                    }
                }
                };
                if budget.compact {
                    // 预览会增加正文控件；固定其父级序位，不能把 footer 焦点变成正文 ID。
                    ui.scope(body);
                } else {
                    egui::ScrollArea::vertical().id_salt("capability-preview-scroll")
                        .max_height((ctx.available_rect().height() - 270.0).clamp(140.0, 420.0))
                        .show(ui, body);
                }
                }
                ui.separator();
                let response = crate::theme::add_enabled(ui, blockers.is_empty(), egui::Button::new("预览全稿兼容影响"));
                focus.reveal(ui, &response);
                preview = response.clicked();
                if let Some(plan) = &state.plan {
                    let response = ui.checkbox(&mut state.acknowledged, "我已查看兼容影响；旧存档/检查点须匹配指纹，入口轨迹须重新严格验证");
                    focus.reveal(ui, &response);
                    let response = crate::theme::add_enabled(ui, plan.can_apply && state.acknowledged && blockers.is_empty(),
                        theme::primary("确认启用（不自动保存）"));
                    focus.reveal(ui, &response);
                    apply = response.clicked();
                } else {
                    // 两个可选控件各占一个 auto-ID；取消的身份不随计划出现而移动。
                    ui.skip_ahead_auto_ids(2);
                }
                if let Some(error) = &state.error {
                    ui.colored_label(theme::ERROR(), error);
                } else {
                    ui.skip_ahead_auto_ids(1);
                }
                let response = ui.button("取消 / 保留当前设置");
                focus.reveal(ui, &response);
                cancel = response.clicked();
                };
                if budget.compact {
                    egui::ScrollArea::vertical().id_salt("capability-complete-scroll")
                        .max_height(budget.inner.y).min_scrolled_height(1.0)
                        .auto_shrink([false, true]).show(ui, contents);
                } else {
                    contents(ui);
                }
            });
        if preview {
            state.acknowledged = false;
            let request = CapabilityEnableRequest {
                target_language: state.target,
                enable_features: state.features.iter().cloned().collect(),
                expected_baseline: self.project.content_baseline(),
            };
            match self.project.plan_capability_enable(&request) {
                Ok(plan) => {
                    state.plan = Some(plan);
                    state.error = None;
                }
                Err(error) => {
                    state.plan = None;
                    state.error = Some(error);
                }
            }
        }
        if apply {
            let before = self.project.clone();
            let result = if self.capability_blockers().is_empty() {
                state
                    .plan
                    .as_ref()
                    .ok_or_else(|| "请重新预览兼容影响".to_string())
                    .and_then(|plan| self.project.apply_capability_enable(plan))
            } else {
                Err("作者输入已变化，请先处理输入并重新预览；未启用能力".into())
            };
            match result {
                Ok(()) => {
                    let localization_enabled = !before.compile_options().localization_ids
                        && self.project.compile_options().localization_ids;
                    self.remember(before);
                    self.recompile();
                    self.manuscript.rebase_clean(&self.project);
                    if localization_enabled {
                        self.localization_ui.clear_operation_status();
                    }
                    self.message =
                        Some("语言与资料能力已显式启用，全文检查通过；尚未保存，可撤销。".into());
                    self.io_error = None;
                    return;
                }
                Err(error) => {
                    state.error = Some(error);
                    state.plan = None;
                    state.acknowledged = false;
                }
            }
        }
        if open && !cancel {
            self.capability_ui = Some(state);
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;
