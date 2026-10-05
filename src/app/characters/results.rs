use super::{state, WorldeditApp};
use crate::theme;
use worldline_core::world_context::WorldContextLimit;
impl WorldeditApp {
    pub(super) fn character_context_results(&mut self, ui: &mut egui::Ui) {
        let Some(result) = self.character_focus.result.clone() else {
            return;
        };
        let entry = ui.interact(
            egui::Rect::from_min_size(ui.cursor().min, egui::vec2(ui.available_width(), 20.0)),
            egui::Id::new("character-context-results-entry"),
            egui::Sense::focusable_noninteractive(),
        );
        if entry.has_focus() {
            ui.colored_label(theme::ACCENT(), "关系结果 · 键盘焦点");
        }
        ui.label(theme::muted(format!(
            "同一快照 · 一跳范围 · 返回 {} / {} 条 · {}",
            result.returned,
            result
                .total
                .map(|n| n.to_string())
                .unwrap_or_else(|| "总量未知".into()),
            if result.complete {
                "范围完整"
            } else {
                "不完整，不能作为完整审计"
            }
        )));
        for reason in &result.reasons {
            ui.colored_label(
                theme::WARNING(),
                match reason {
                    WorldContextLimit::ExecutableIndexBudget => {
                        "静态使用处索引预算已耗尽，当前结果不完整"
                    }
                    WorldContextLimit::SourceUnavailable => "部分来源无法确认，未提供猜测位置",
                    WorldContextLimit::InvalidSource => {
                        "稿件有错误：只读结果可能不完整，请先处理诊断"
                    }
                    WorldContextLimit::CandidateBudget => {
                        "候选预算已耗尽：请关闭文字提及或选择更接近的对象"
                    }
                    WorldContextLimit::NodeLimit => "邻域节点达到上限：选择相邻对象继续查看",
                    WorldContextLimit::RecordLimit => "关系条数达到上限：选择相邻对象继续查看",
                    WorldContextLimit::SourceConflict => {
                        "当前缓冲与磁盘有未决冲突：先保留并核对两个版本"
                    }
                },
            );
        }
        egui::ScrollArea::vertical()
            .id_salt("character-context-results")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for (kind, label, _) in state::KINDS {
                    if !self.character_focus.visible(&kind) {
                        continue;
                    }
                    let rows = result
                        .records
                        .iter()
                        .filter(|r| r.kind == kind)
                        .collect::<Vec<_>>();
                    if rows.is_empty() {
                        continue;
                    }
                    egui::CollapsingHeader::new(format!("{label} · {} 条", rows.len()))
                        .default_open(true)
                        .show(ui, |ui| {
                            for row in rows {
                                ui.push_id(&row.id, |ui| {
                                    let other = if row.from_ref == result.target {
                                        &row.to_ref
                                    } else {
                                        &row.from_ref
                                    };
                                    let node = result.nodes.iter().find(|n| &n.target == other);
                                    let name =
                                        node.map(|n| n.display.as_str()).unwrap_or(&other.id);
                                    ui.horizontal_wrapped(|ui| {
                                        if ui
                                            .add_enabled(
                                                node.is_some_and(|n| n.exists),
                                                egui::Button::new(crate::visual::truncated(
                                                    name, 24,
                                                )),
                                            )
                                            .on_hover_text(format!("{name}\n{}", state::key(other)))
                                            .clicked()
                                        {
                                            self.activate_character_context_target(other.clone());
                                        }
                                        ui.add(
                                            egui::Label::new(format!(
                                                "{} · {}",
                                                if row.direction
                                                    == worldline_core::RelationDirection::Undirected
                                                {
                                                    "无向 ↔"
                                                } else if row.from_ref == result.target {
                                                    "向外 →"
                                                } else {
                                                    "向内 ←"
                                                },
                                                row.role
                                            ))
                                            .wrap(),
                                        );
                                        if ui.small_button("定位来源").clicked() {
                                            self.jump_to_file(
                                                &row.source.file,
                                                row.source.line,
                                                row.source.column.unwrap_or(1),
                                            );
                                        }
                                    });
                                    if let worldline_core::world_context::WorldContextProvenance::FormalRelation { scope_refs, .. } = &row.provenance {
                                        if !scope_refs.is_empty() {
                                            ui.horizontal_wrapped(|ui| {
                                                ui.label(theme::muted("作者限定范围："));
                                                for scope in scope_refs {
                                                    let object = self.snapshot.as_ref().and_then(|s|s.result.analysis.catalog.object(scope)).cloned();
                                                    let name = object.as_ref().map(|o|o.display.as_str()).unwrap_or(&scope.id);
                                                    if ui.add_enabled(object.is_some(),egui::Button::new(crate::visual::truncated(name,20)).small()).on_hover_text(state::key(scope)).clicked() {
                                                        if let Some(object) = &object { self.jump_to_file(&object.file,object.line,1); }
                                                    }
                                                }
                                            });
                                        }
                                    }
                                    let source = format!(
                                        "{}:{} · {}",
                                        theme::relative_source(
                                            &self.project.root,
                                            std::path::Path::new(&row.source.file)
                                        ),
                                        row.source.line,
                                        state::precision_label(row.source.precision)
                                    );
                                    ui.add(egui::Label::new(theme::muted(&source)).truncate())
                                        .on_hover_text(format!(
                                            "{}\n{} → {}\n{}\n{}",
                                            row.source.file,
                                            state::key(&row.from_ref),
                                            state::key(&row.to_ref),
                                            state::provenance_label(&row.provenance),
                                            row.id
                                        ));
                                });
                            }
                        });
                }
                if result.records.is_empty() && result.complete {
                    ui.label("此一跳范围内没有所含类型的关系或引用");
                }
            });
    }
}
