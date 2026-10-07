//! 组合稿与失败保留稿分开；失败稿不能作为新工程正文再次隐式提交。
use super::{prose, Action, Mode, Typography, ViewState};
use worldline_core::{catalog::TargetRef, manuscript::WritingBuffer};

#[derive(Clone)]
pub(super) struct PendingInput {
    pub text: String,
    pub generation: u64,
    pub mode: Mode,
}

fn editor_id(key: &prose::RetainedKey, mode: Mode) -> egui::Id {
    match mode {
        Mode::Prose => egui::Id::new((
            "writing-prose",
            &key.path,
            &key.target.kind,
            &key.target.id,
            key.offset,
        )),
        Mode::Structure => egui::Id::new((
            "writing-structure",
            &key.path,
            &key.target.kind,
            &key.target.id,
        )),
        Mode::Source => egui::Id::new((
            "writing-source",
            &key.path,
            &key.target.kind,
            &key.target.id,
        )),
    }
}

impl ViewState {
    pub(in crate::app) fn needs_input_rescue(
        &self,
        target: Option<&TargetRef>,
        read_only: bool,
        hidden: bool,
    ) -> bool {
        self.composing_inputs.iter().any(|(key, input)| {
            read_only || hidden || target != Some(&key.target) || input.mode != self.mode
        })
    }

    pub(in crate::app) fn has_pending_for(
        &self,
        path: &std::path::Path,
        target: &TargetRef,
    ) -> bool {
        self.composing_inputs
            .keys()
            .any(|key| key.path == path && key.target == *target)
    }

    pub(in crate::app) fn draw_orphaned_compositions(
        &mut self,
        ui: &mut egui::Ui,
        buffers: &std::collections::HashMap<std::path::PathBuf, WritingBuffer>,
        typography: Typography,
    ) {
        let owners: std::collections::BTreeSet<_> = self
            .composing_inputs
            .iter()
            .filter(|(key, input)| {
                !super::input_registry::drawn_this_frame(ui.ctx(), editor_id(key, input.mode))
            })
            .map(|(key, _)| (key.path.clone(), key.target.clone()))
            .collect();
        for (path, target) in owners {
            if let Some(buffer) = buffers.get(&path) {
                let error = "章节或权限已变化；请完成组合，完整输入会保留，不写入旧来源";
                ui.colored_label(crate::theme::WARNING(), error);
                ui.label(format!("原来源 · {}:{}", target.kind, target.id));
                ui.add(egui::Label::new(crate::theme::muted(path.to_string_lossy())).wrap());
                rescue(
                    ui,
                    buffer,
                    &target,
                    self,
                    typography,
                    error,
                    &mut Action::default(),
                );
            }
        }
    }

    pub(in crate::app) fn prose_text(
        &self,
        path: &std::path::Path,
        target: &TargetRef,
        offset: usize,
        fallback: &str,
    ) -> String {
        self.composing_inputs
            .get(&prose::RetainedKey::new(path, target, offset))
            .map(|input| input.text.clone())
            .unwrap_or_else(|| fallback.to_owned())
    }

    pub(super) fn pending_outside_projection(
        &self,
        buffer: &WritingBuffer,
        target: &TargetRef,
        projection: &worldline_core::manuscript::WritingProjection,
    ) -> bool {
        self.composing_inputs.iter().any(|(key, input)| {
            key.path == buffer.path()
                && key.target == *target
                && match input.mode {
                    Mode::Structure => key.offset != projection.range.start,
                    Mode::Source => false,
                    Mode::Prose => {
                        !projection
                            .empty_prose_slot
                            .as_ref()
                            .is_some_and(|slot| slot.offset() == key.offset)
                            && !projection.blocks.iter().any(|block| {
                                block.kind == worldline_core::manuscript::WritingBlockKind::Prose
                                    && block.range.start == key.offset
                            })
                    }
                }
        })
    }

    pub(super) fn pending_prose(&self, key: &prose::RetainedKey) -> bool {
        self.composing_inputs.contains_key(key)
    }
}

fn retain_error(view: &mut ViewState, mut key: prose::RetainedKey, text: &str, error: &str) {
    // 不同编辑尝试可在同一 core 范围相继失败；不能让新失败覆盖旧保留稿。
    while view.retained_inputs.contains_key(&key) {
        key.attempt += 1;
    }
    view.retained_inputs.insert(
        key,
        prose::RetainedInput {
            text: text.to_owned(),
            error: error.to_owned(),
        },
    );
}

pub(super) fn finish(
    view: &mut ViewState,
    key: prose::RetainedKey,
    text: &str,
    generation: u64,
    changed: bool,
    write: impl FnOnce() -> Result<(), String>,
) -> Option<String> {
    let pending = view
        .composing_inputs
        .get(&key)
        .map(|input| input.generation);
    if view.composing() {
        if changed {
            view.composing_inputs.insert(
                key,
                PendingInput {
                    text: text.to_owned(),
                    generation: pending.unwrap_or(generation),
                    mode: view.mode,
                },
            );
        }
        return None;
    }
    if !changed && pending.is_none() {
        return None;
    }
    let result = if pending.is_some() && !changed && !view.committed() {
        Err("组合已结束但未收到提交；临时文字已保留，请复制核对后处理".into())
    } else if pending.is_some_and(|previous| previous != generation) {
        Err("输入法组合期间正文已变化；完整提交已保留，请核对新稿后合并".into())
    } else {
        write()
    };
    view.composing_inputs.remove(&key);
    match result {
        Ok(()) => None,
        Err(error) => {
            retain_error(view, key, text, &error);
            Some(error)
        }
    }
}

/// 只接收文本，不写源。使用组合开始时的模式和身份，不借用当前被切换的章节。
pub(super) fn rescue(
    ui: &mut egui::Ui,
    buffer: &WritingBuffer,
    target: &TargetRef,
    view: &mut ViewState,
    typography: Typography,
    error: &str,
    action: &mut Action,
) {
    let pending: Vec<_> = view
        .composing_inputs
        .iter()
        .filter(|(key, input)| {
            key.path == buffer.path()
                && key.target == *target
                && !super::input_registry::drawn_this_frame(ui.ctx(), editor_id(key, input.mode))
        })
        .map(|(key, input)| (key.clone(), input.clone()))
        .collect();
    for (key, mut input) in pending {
        let output = if input.mode == Mode::Prose {
            prose::edit(
                ui,
                buffer,
                target,
                view,
                typography,
                key.offset,
                &mut input.text,
                false,
            )
        } else {
            egui::TextEdit::multiline(&mut input.text)
                .id(editor_id(&key, input.mode))
                .code_editor()
                .font(crate::theme::source_font(typography.source_size))
                .desired_width(f32::INFINITY)
                .desired_rows(12)
                .show(ui)
        };
        super::register_input(&output.response);
        super::remember_text_undo(ui.ctx(), output.response.id, &input.text);
        if view.composing() {
            view.composing_inputs.insert(key, input);
        } else {
            view.composing_inputs.remove(&key);
            let error = format!("{error}；完整输入已保留，尚未写入正文");
            retain_error(view, key, &input.text, &error);
            action.error = Some(error);
        }
    }
}
