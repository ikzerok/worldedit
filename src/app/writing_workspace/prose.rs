use super::*;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(super) struct RetainedKey {
    pub path: std::path::PathBuf,
    pub target: TargetRef,
    pub offset: usize,
    pub attempt: u64,
}
impl RetainedKey {
    pub fn new(path: &std::path::Path, target: &TargetRef, offset: usize) -> Self {
        Self {
            path: path.to_owned(),
            target: target.clone(),
            offset,
            attempt: 0,
        }
    }
}
pub(super) struct RetainedInput {
    pub text: String,
    pub error: String,
}

impl ViewState {
    pub(in crate::app) fn has_retained_input(&self) -> bool {
        self.retained_inputs
            .values()
            .any(|input| !input.text.is_empty())
            || self
                .composing_inputs
                .values()
                .any(|input| !input.text.is_empty())
    }
    pub(in crate::app) fn has_retained_for(&self, path: &std::path::Path) -> bool {
        self.retained_inputs
            .iter()
            .any(|(key, input)| key.path == path && !input.text.is_empty())
            || self
                .composing_inputs
                .iter()
                .any(|(key, input)| key.path == path && !input.text.is_empty())
    }
    pub(in crate::app) fn retained_runtime_drafts(
        &self,
        root: &std::path::Path,
    ) -> std::collections::BTreeMap<String, String> {
        self.retained_inputs
            .iter()
            .map(|(key, input)| (key, &input.text, "未插入正文", "retained_prose"))
            .chain(
                self.composing_inputs
                    .iter()
                    .map(|(key, input)| (key, &input.text, "组合中正文", "composing_prose")),
            )
            .filter(|(_, text, _, _)| !text.is_empty())
            .map(|(key, text, label, kind)| {
                let path = key.path.strip_prefix(root).unwrap_or(&key.path);
                (
                    format!(
                        "{label} · {}:{} · {} · 位置{} · 保留{}",
                        key.target.kind,
                        key.target.id,
                        path.display(),
                        key.offset,
                        key.attempt
                    ),
                    serde_json::json!([kind, key.path, key.target, key.offset, text, key.attempt])
                        .to_string(),
                )
            })
            .collect()
    }
    pub(in crate::app) fn draw_retained_input(&mut self, ui: &mut egui::Ui) {
        super::editors::retained_notice(ui, self);
    }
    pub(in crate::app) fn discard_retained_for(&mut self, path: &std::path::Path) {
        self.retained_inputs.retain(|key, _| key.path != path);
        self.composing_inputs.retain(|key, _| key.path != path);
        if self
            .retained_clear_confirm
            .as_ref()
            .is_some_and(|key| key.path == path)
        {
            self.retained_clear_confirm = None;
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn edit(
    ui: &mut egui::Ui,
    buffer: &WritingBuffer,
    target: &TargetRef,
    view: &mut ViewState,
    typography: Typography,
    offset: usize,
    text: &mut String,
    empty: bool,
) -> egui::text_edit::TextEditOutput {
    let font = theme::body_font(typography.size);
    let mut layouter = |ui: &egui::Ui, text: &dyn egui::TextBuffer, width: f32| {
        let mut job = egui::text::LayoutJob::simple(
            text.as_str().to_owned(),
            font.clone(),
            theme::TEXT(),
            width,
        );
        for section in &mut job.sections {
            section.format.line_height = Some(typography.size * typography.spacing);
        }
        ui.fonts(|fonts| fonts.layout_job(job))
    };
    let id = egui::Id::new((
        "writing-prose",
        buffer.path(),
        &target.kind,
        &target.id,
        offset,
    ));
    let key = RetainedKey::new(buffer.path(), target, offset);
    if !view.pending_prose(&key) {
        view.restore_editor(ui, id, buffer, offset, text);
    }
    egui::TextEdit::multiline(text)
        .id(id)
        .font(font.clone())
        .layouter(&mut layouter)
        .frame(false)
        .hint_text(if empty {
            "从这里写下第一段……"
        } else {
            ""
        })
        .desired_width(f32::INFINITY)
        .desired_rows(if empty { 12 } else { 2 })
        .show(ui)
}
