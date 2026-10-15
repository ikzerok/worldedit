use super::*;
#[cfg(all(test, not(target_arch = "wasm32")))]
#[path = "candidate_cache_tests.rs"]
mod tests;

#[derive(Clone, PartialEq, Eq)]
struct CacheKey {
    root: std::path::PathBuf,
    version: u64,
    // 仅是 UI 缓存失效提示；正式来源仍由 core 校验，绝不作为来源权限。
    compiled_instance: usize,
    baseline: String,
    kind: ConditionCandidateKind,
    text: String,
    offset: usize,
}

#[derive(Clone, Default)]
struct Picker {
    text: String,
    offset: usize,
    cache: Option<(CacheKey, ConditionCandidates)>,
}
impl Picker {
    fn page(
        &mut self,
        context: &Context<'_>,
        kind: ConditionCandidateKind,
    ) -> Result<ConditionCandidates, String> {
        let key = CacheKey {
            root: context.project.root.clone(),
            version: context.version,
            compiled_instance: std::ptr::from_ref(context.compiled) as usize,
            baseline: context.current_baseline.clone(),
            kind,
            text: self.text.clone(),
            offset: self.offset,
        };
        if let Some((_, page)) = self.cache.as_ref().filter(|(cached, _)| cached == &key) {
            return Ok(page.clone());
        }
        self.cache = None;
        let page = context.project.condition_candidates_from_compile(
            context.compiled,
            &ConditionCandidateQuery {
                schema_version: 1,
                kind,
                text: self.text.clone(),
                offset: self.offset,
                limit: 20,
            },
        )?;
        self.cache = Some((key, page.clone()));
        Ok(page)
    }
}

pub(super) fn select(
    ui: &mut egui::Ui,
    context: &Context<'_>,
    kind: ConditionCandidateKind,
    current: &mut String,
    bool_only: bool,
) -> Option<ConditionCandidate> {
    let label = match kind {
        ConditionCandidateKind::State => "状态",
        ConditionCandidateKind::Tag => "标签",
        ConditionCandidateKind::Variable => "全局变量",
    };
    let mut selected = None;
    let combo = egui::ComboBox::from_id_salt(label)
        .width(ui.available_width().max(80.0))
        .truncate()
        .selected_text(if current.is_empty() {
            format!("选择真实{label}")
        } else {
            current.clone()
        })
        .show_ui(ui, |ui| {
            ui.set_max_width(ui.ctx().screen_rect().width().min(420.0) - 24.0);
            let key = ui.id().with("condition-candidate-page");
            let mut picker = ui
                .data_mut(|data| data.get_temp::<Picker>(key))
                .unwrap_or_default();
            let search = ui.add(
                egui::TextEdit::singleline(&mut picker.text)
                    .hint_text("搜索名称 / 完整 ID")
                    .char_limit(128)
                    .desired_width(f32::INFINITY),
            );
            input::register(&search, &context.project.root);
            if search.changed() {
                picker.offset = 0;
            }
            match picker.page(context, kind) {
                Ok(page) => {
                    for candidate in &page.items {
                        let enabled = !context.composing
                            && (!bool_only
                                || candidate.value_type == Some(ConditionValueType::Bool));
                        let response = theme::add_enabled(
                            ui,
                            enabled,
                            egui::Button::new(format!(
                                "{} · {}",
                                candidate.display, candidate.target.id
                            ))
                            .selected(*current == candidate.target.id),
                        );
                        crate::chrome::action_layout::record(
                            ui,
                            &format!(
                                "condition/candidate/{}:{}",
                                candidate.target.kind, candidate.target.id
                            ),
                            &response,
                        );
                        if response.clicked() {
                            *current = candidate.target.id.clone();
                            selected = Some(candidate.clone());
                            ui.close();
                        }
                        response.on_hover_text(format!(
                            "{}:{}\n{}:{}{}",
                            candidate.target.kind,
                            candidate.target.id,
                            candidate.file,
                            candidate.line,
                            candidate
                                .owner
                                .as_ref()
                                .map(|owner| format!("\n所属 {}:{}", owner.kind, owner.id))
                                .unwrap_or_default()
                        ));
                    }
                    ui.horizontal_wrapped(|ui| {
                        if theme::add_enabled(
                            ui,
                            !context.composing && picker.offset > 0,
                            egui::Button::new("上一页候选"),
                        )
                        .clicked()
                        {
                            picker.offset = picker.offset.saturating_sub(20);
                        }
                        ui.label(format!("共 {} 项", page.total));
                        if theme::add_enabled(
                            ui,
                            !context.composing && page.has_more,
                            egui::Button::new("下一页候选"),
                        )
                        .clicked()
                        {
                            picker.offset += 20;
                        }
                    });
                    if page.total == 0 {
                        ui.label("没有符合检索的已声明对象");
                    }
                }
                Err(error) => {
                    ui.colored_label(theme::ERROR(), error);
                }
            }
            ui.data_mut(|data| data.insert_temp(key, picker));
        });
    let role = match kind {
        ConditionCandidateKind::State => "condition/state-picker",
        ConditionCandidateKind::Tag => "condition/tag-picker",
        ConditionCandidateKind::Variable => "condition/variable-picker",
    };
    if !context.composing {
        crate::chrome::action_layout::record(ui, role, &combo.response);
    }
    selected
}
