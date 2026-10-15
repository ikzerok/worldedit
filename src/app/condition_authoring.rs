//! 条件作者表单只消费 core DTO；不解析表达式，不生成 DSL 或来源补丁。
mod candidates;
pub(super) mod focus;
pub(super) mod input;
mod navigation;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;
mod tree;
use crate::theme;
use worldline_core::{condition_authoring::*, project::Project, CompileResult, TargetRef};

pub(super) struct Context<'a> {
    pub project: &'a Project,
    pub compiled: &'a CompileResult,
    pub version: u64,
    pub baseline: &'a str,
    pub current_baseline: String,
    pub draft_signature: String,
    pub composing: bool,
}
#[derive(Clone, Default)]
pub(super) struct ConditionForms {
    entries: Vec<(ConditionOwner, ConditionSlot, Buffer)>,
    sequence: u64,
    navigation: Option<Navigation>,
    pub(super) choice_cache: Option<(String, Vec<worldline_core::authoring::ChoiceDraft>)>,
    pub(super) choice_error: Option<String>,
}
#[derive(Clone)]
pub(super) enum Navigation {
    Definition(worldline_core::state_inspection_source::DeclarationSource),
    Owner(TargetRef),
}
impl ConditionForms {
    pub(super) fn take_navigation(&mut self) -> Option<Navigation> {
        self.navigation.take()
    }
    pub(super) fn has_pending(&self) -> bool {
        self.choice_error.is_some() || self.entries.iter().any(|(_, _, buffer)| buffer.dirty())
    }
    pub(super) fn clear(&mut self) {
        self.entries.clear();
        self.choice_cache = None;
        self.choice_error = None;
        self.navigation = None;
    }
    pub(super) fn signature(&self) -> String {
        let trees = self
            .entries
            .iter()
            .filter(|(_, _, buffer)| buffer.dirty())
            .map(|(owner, slot, buffer)| (owner, slot, &buffer.input, &buffer.tree))
            .collect::<Vec<_>>();
        serde_json::to_string(&(
            trees,
            self.choice_error
                .as_ref()
                .map(|_| format!("{:?}", self.choice_cache)),
        ))
        .unwrap_or_default()
    }
}
#[derive(Clone)]
struct Buffer {
    input: ConditionDraftInput,
    baseline: String,
    draft_signature: String,
    projection: ConditionProjection,
    tree: Option<ConditionNode>,
    preview: Option<ConditionDraftPreview>,
    error: Option<String>,
}
impl Buffer {
    fn dirty(&self) -> bool {
        self.tree != self.projection.tree
    }
    fn stale(&self, source: &str, context: &Context<'_>) -> bool {
        self.input.source != source
            || self.baseline != context.baseline
            || self.baseline != context.current_baseline
            || self.draft_signature != context.draft_signature
    }
    fn preview(&mut self, context: &Context<'_>, source: &str) -> Result<(), String> {
        self.preview = None;
        if self.stale(source, context) {
            return Err("条件原稿或工程基线已变化；输入保留，请取消此条件输入后重新读取".into());
        }
        self.preview = Some(context.project.preview_condition_draft(
            context.compiled,
            &self.input,
            &self.tree,
        )?);
        Ok(())
    }
    fn apply(&self, context: &Context<'_>, source: &str) -> Result<String, String> {
        if self.stale(source, context) {
            return Err("预览后稿源已变化；没有覆盖当前稿，条件输入保留".into());
        }
        let preview = self.preview.as_ref().ok_or("请先预览这个条件的改动")?;
        if preview.replacement != self.tree {
            return Err("条件输入已变化，请重新预览".into());
        }
        context
            .project
            .apply_condition_draft(context.compiled, &self.input, preview)
    }
}

pub(super) fn show(
    ui: &mut egui::Ui,
    context: &Context<'_>,
    forms: &mut ConditionForms,
    owner: ConditionOwner,
    slot: ConditionSlot,
    source: &mut String,
) {
    let label = match slot {
        ConditionSlot::ChoiceDisplay => "显示条件",
        ConditionSlot::ChoiceEnable => "可选条件",
        ConditionSlot::EventAfter => "前置条件 after",
    };
    ui.push_id(
        (
            "condition-author",
            owner.target.id.clone(),
            owner.choice_line,
            label,
        ),
        |ui| {
            ui.label(egui::RichText::new(label).strong());
            let index = forms
                .entries
                .iter()
                .position(|(candidate, candidate_slot, _)| {
                    candidate == &owner && *candidate_slot == slot
                });
            let mut buffer = index.map(|index| forms.entries.remove(index).2);
            let needs_read = buffer.as_ref().is_none_or(|buffer| {
                !buffer.dirty()
                    && (buffer.input.source != *source
                        || buffer.baseline != context.baseline
                        || buffer.draft_signature != context.draft_signature)
            });
            if needs_read {
                forms.sequence = forms.sequence.wrapping_add(1);
                let input = ConditionDraftInput {
                    schema_version: 1,
                    owner: owner.clone(),
                    slot,
                    generation: forms.sequence,
                    source: source.clone(),
                };
                match context
                    .project
                    .condition_draft_from_compile(context.compiled, &input)
                {
                    Ok(projection) => {
                        buffer = Some(Buffer {
                            input,
                            baseline: context.baseline.into(),
                            draft_signature: context.draft_signature.clone(),
                            tree: projection.tree.clone(),
                            projection,
                            preview: None,
                            error: None,
                        })
                    }
                    Err(error) => {
                        ui.colored_label(theme::ERROR(), error);
                        buffer = None;
                    }
                }
            }
            let Some(mut buffer) = buffer else {
                source_editor(ui, source, true, context);
                return;
            };
            let supported = matches!(
                buffer.projection.support,
                ConditionSupport::Supported | ConditionSupport::Empty
            );
            let stale = buffer.stale(source, context);
            let enabled = supported && !stale && buffer.projection.writable;
            if stale {
                ui.colored_label(
                    theme::WARNING(),
                    "此条件输入基于旧稿；原输入保留，应用被阻止",
                );
            }
            for reason in buffer
                .projection
                .reasons
                .iter()
                .chain(&buffer.projection.read_only_reasons)
            {
                ui.label(theme::muted(reason));
            }
            for diagnostic in &buffer.projection.diagnostics {
                ui.colored_label(theme::WARNING(), &diagnostic.message);
            }
            let mut discard = false;
            let mut applied = false;
            if supported {
                ui.label(theme::muted(
                    "正在编辑：事件表单草稿；确认条件后仍需应用事件，再保存工程",
                ));
                let before = buffer.tree.clone();
                // 外部基线变化不能撤走正在组合的 TextEdit；仅保留接收，应用仍受完整 guard。
                theme::add_enabled_ui(ui, enabled || context.composing, |ui| {
                    if let Some(node) = &mut buffer.tree {
                        tree::show(ui, context, node, &mut forms.navigation);
                        if theme::add_enabled(
                            ui,
                            !context.composing,
                            egui::Button::new("删除整个条件"),
                        )
                        .clicked()
                        {
                            buffer.tree = None;
                        }
                    } else {
                        ui.label("没有条件限制");
                        if theme::add_enabled(ui, !context.composing, egui::Button::new("添加条件"))
                            .clicked()
                        {
                            buffer.tree = Some(ConditionNode::Bool { value: true });
                        }
                    }
                });
                if buffer.tree != before {
                    buffer.preview = None;
                    buffer.error = None;
                }
                ui.horizontal_wrapped(|ui| {
                    let preview = theme::add_enabled(
                        ui,
                        enabled && !context.composing,
                        egui::Button::new("预览条件改动"),
                    );
                    crate::chrome::action_layout::record(
                        ui,
                        &format!("condition/{label}/preview"),
                        &preview,
                    );
                    if preview.clicked() {
                        if let Err(error) = buffer.preview(context, source) {
                            buffer.error = Some(error);
                        }
                    }
                    if theme::add_enabled(ui, !context.composing, egui::Button::new("取消条件输入"))
                        .clicked()
                    {
                        discard = true;
                    }
                });
                if let Some(preview) = buffer.preview.clone() {
                    ui.group(|ui| {
                        ui.label(if preview.changed {
                            "预览：仅替换这个条件，尚未写入事件草稿"
                        } else {
                            "预览：无变化，保留原始字节"
                        });
                        ui.label("原条件");
                        ui.add(
                            egui::Label::new(if preview.before.source.is_empty() {
                                "（空条件）"
                            } else {
                                &preview.before.source
                            })
                            .wrap(),
                        );
                        ui.label("修改后");
                        ui.add(
                            egui::Label::new(if preview.after_source.is_empty() {
                                "（删除条件限制）"
                            } else {
                                &preview.after_source
                            })
                            .wrap(),
                        );
                        let apply = theme::add_enabled(
                            ui,
                            enabled && !context.composing,
                            theme::primary("应用到事件草稿"),
                        );
                        crate::chrome::action_layout::record(
                            ui,
                            &format!("condition/{label}/apply"),
                            &apply,
                        );
                        if apply.clicked() {
                            match buffer.apply(context, source) {
                                Ok(replacement) => {
                                    *source = replacement;
                                    applied = true;
                                }
                                Err(error) => buffer.error = Some(error),
                            }
                        }
                    });
                }
                ui.label(theme::muted(
                    "组内按原顺序从左到右求值；预览不运行条件，不消耗随机数",
                ));
            } else {
                ui.label(match buffer.projection.support {
                    ConditionSupport::Invalid => "高级源码 · 当前条件无效；完整原文保留",
                    ConditionSupport::BudgetExceeded => "高级源码 · 超出有界表单预算；完整原文保留",
                    ConditionSupport::VersionUnsupported => {
                        "高级源码 · 当前语言版本不支持此条件槽；不会自动升级"
                    }
                    _ => "高级源码 · 此表达式不在表单子集；完整原文保留",
                });
            }
            if let Some(error) = &buffer.error {
                ui.colored_label(theme::ERROR(), error);
            }
            source_editor(ui, source, !buffer.dirty(), context);
            if !discard && !applied {
                forms.entries.push((owner, slot, buffer));
            }
        },
    );
}

fn source_editor(ui: &mut egui::Ui, source: &mut String, enabled: bool, context: &Context<'_>) {
    let composing = context.composing;
    let key = ui.id().with("condition-source-focus");
    let focus = ui.data_mut(|data| data.get_temp::<egui::Id>(key));
    let keep_open = composing && focus.is_some_and(|id| ui.memory(|memory| memory.has_focus(id)));
    let header = egui::CollapsingHeader::new("高级源码（完整保留）");
    let header = if keep_open {
        header.open(Some(true))
    } else {
        header
    };
    header.show(ui, |ui| {
        let response = theme::add_enabled(
            ui,
            enabled,
            egui::TextEdit::multiline(source)
                .id_salt("condition-source")
                .desired_rows(2)
                .desired_width(f32::INFINITY),
        );
        input::register(&response, &context.project.root);
        ui.data_mut(|data| data.insert_temp(key, response.id));
        if !enabled {
            ui.label("先预览并应用或取消条件表单输入，再编辑高级源码");
        }
    });
}

pub(super) fn owner(event: &str, line: Option<u32>) -> ConditionOwner {
    ConditionOwner {
        target: TargetRef::new("event", event),
        choice_line: line,
    }
}

pub(super) fn draft_signature(draft: &worldline_core::authoring::EventDraft) -> String {
    super::writing_workspace::fingerprint(&format!(
        "{}\u{0}{}\u{0}{}",
        draft.id, draft.after, draft.body
    ))
}
