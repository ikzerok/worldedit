//! 事件与世界观的结构化表单;源码操作统一提交给 core。
use super::WorldeditApp;
use crate::theme::{self, *};
use egui::RichText;
use std::path::Path;
#[cfg(test)]
use worldline_core::ast::PropertyValue;
use worldline_core::authoring::WorldDraft;
mod effects;
mod properties;
mod temporal_constraints;
#[cfg(test)]
mod temporal_tests;
#[cfg(test)]
use properties::properties;
pub(super) use properties::{properties_excluding, properties_with_references};

pub(super) fn field(ui: &mut egui::Ui, label: &str, value: &mut String) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        ui.add(egui::Label::new(theme::muted(label)).wrap());
        ui.add(egui::TextEdit::singleline(value).desired_width(f32::INFINITY));
    });
}

impl WorldeditApp {
    pub(super) fn event_inspector(&mut self, ctx: &egui::Context) {
        self.capture_new_draft_baselines();
        let event_composing = super::condition_authoring::input::event_composing(
            ctx,
            &self.project.root,
            self.event_editor.is_some(),
        );
        let composing = self.ime_composing || event_composing;
        super::condition_authoring::focus::begin(ctx, !composing);
        egui::SidePanel::right("event-inspector")
            .default_width(300.0)
            .width_range(270.0..=410.0)
            .frame(theme::panel())
            .show(ctx, |ui| {
                let Some(mut editor) = self.event_editor.take() else {
                    theme::panel_header(ui, "事件详情", "尚未选择事件");
                    ui.label("选择画布中的事件，查看正文、人物与先后要求。");
                    ui.add_space(theme::metrics().section_gap);
                    if ui.add(theme::primary("＋ 新建事件")).clicked() {
                        self.new_event(None);
                    }
                    return;
                };
                let condition_baseline = self.project.content_baseline();
                let condition_draft_signature =
                    super::condition_authoring::draft_signature(&editor.draft);
                let options = temporal_constraints::options(&self.project, &mut editor);
                let temporal_ready = temporal_constraints::can_apply(&options)
                    && !self.stale_form
                    && !editor.condition_forms.has_pending()
                    && !composing;
                let mut close = false;
                let mut apply = false;
                let mut delete = false;
                // 动态待确认/过期提示只占父布局的一个固定子域，避免下方自动 ID 漂移。
                ui.scope(|ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(
                            RichText::new(if editor.original.is_some() {
                                "事件详情"
                            } else {
                                "创建事件"
                            })
                            .strong()
                            .size(17.0),
                        );
                        let commit = crate::theme::add_enabled(
                            ui,
                            temporal_ready,
                            theme::primary("应用更改"),
                        );
                        crate::chrome::action_layout::record(ui, "condition/event-apply", &commit);
                        if commit.clicked() {
                            apply = true;
                        }
                        if crate::theme::add_enabled(ui, !composing, egui::Button::new("取消编辑"))
                            .clicked()
                        {
                            close = true;
                        }
                    });
                    ui.add(egui::Label::new(format!("当前事件：{}", editor.draft.id)).truncate())
                        .on_hover_text(&editor.draft.id);
                    let pending = editor.condition_forms.has_pending();
                    let status_height = ui.spacing().interact_size.y.max(
                        ui.text_style_height(&egui::TextStyle::Button)
                            + 2.0 * ui.spacing().button_padding.y,
                    );
                    // 复用原保存说明行；首次输入不再把正在编辑的控件推出视口。
                    ui.allocate_ui_with_layout(
                        egui::vec2(ui.available_width(), status_height),
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            ui.set_min_height(status_height);
                            if pending && theme::add_enabled(ui, !composing, egui::Button::new("取消全部"))
                                .on_hover_text("取消本事件所有待确认条件输入；保留已应用到事件草稿和工程的内容。")
                                .clicked()
                            {
                                editor.condition_forms.clear();
                            }
                            let status = if pending {
                                RichText::new("条件输入待确认").color(WARNING())
                            } else {
                                theme::muted("应用进入工程缓冲；保存后写入磁盘")
                            };
                            ui.add(egui::Label::new(status).truncate()).on_hover_text(if pending {
                                "先预览并应用到事件草稿，或取消所有待确认条件输入。应用事件后进入工程缓冲；保存后写入磁盘。"
                            } else {
                                "应用进入工程缓冲；保存后写入磁盘"
                            });
                        },
                    );
                    temporal_constraints::application_status(ui, &options);
                    if self.stale_form {
                        ui.colored_label(
                            ERROR(),
                            "外部内容已改变，输入已保留；请重开表单合并后应用。",
                        );
                    }
                });
                ui.separator();
                egui::ScrollArea::vertical()
                    .id_salt("event-form")
                    .show(ui, |ui| {
                        ui.label(theme::muted("事件 ID"));
                        crate::theme::add_enabled(
                            ui,
                            editor.original.is_none(),
                            egui::TextEdit::singleline(&mut editor.draft.id)
                                .desired_width(f32::INFINITY),
                        );
                        field(ui, "事件名称 / 简述", &mut editor.draft.summary);
                        temporal_constraints::show(
                            ui,
                            &self.project,
                            &mut editor,
                            self.snapshot
                                .as_ref()
                                .map(|snapshot| {
                                    snapshot.result.analysis.timeline.periods.as_slice()
                                })
                                .unwrap_or_default(),
                            self.stale_form,
                        );
                        if let Some(snapshot) = &self.snapshot {
                            let context = super::condition_authoring::Context {
                                project: &self.project,
                                compiled: &snapshot.result,
                                version: self.version,
                                baseline: &editor.baseline,
                                current_baseline: condition_baseline.clone(),
                                draft_signature: condition_draft_signature.clone(),
                                composing,
                            };
                            if let Err(error) = super::choices::choice_cards(
                                ui,
                                &mut editor.draft,
                                &context,
                                &mut editor.condition_forms,
                            ) {
                                self.io_error = Some(error);
                            }
                        }
                        ui.add_space(12.0);

                        ui.label(theme::muted("源文件"));
                        egui::ComboBox::from_id_salt("event-file")
                            .width(ui.available_width() - 20.0)
                            .selected_text(
                                editor
                                    .path
                                    .strip_prefix(&self.project.root)
                                    .unwrap_or(&editor.path)
                                    .display()
                                    .to_string(),
                            )
                            .show_ui(ui, |ui| {
                                for path in self.project.documents.keys() {
                                    crate::theme::add_enabled_ui(
                                        ui,
                                        editor.original.is_none(),
                                        |ui| {
                                            ui.selectable_value(
                                                &mut editor.path,
                                                path.clone(),
                                                path.strip_prefix(&self.project.root)
                                                    .unwrap_or(path)
                                                    .display()
                                                    .to_string(),
                                            );
                                        },
                                    );
                                }
                            });
                        field(
                            ui,
                            "故事线 ID（可创建新故事线）",
                            &mut editor.draft.storyline,
                        );
                        ui.horizontal(|ui| {
                            ui.label(theme::muted("编排序号"));
                            let mut order = editor.draft.order.unwrap_or(0);
                            if ui
                                .add(
                                    egui::DragValue::new(&mut order)
                                        .range(0..=u32::MAX)
                                        .speed(10),
                                )
                                .changed()
                            {
                                editor.draft.order = (order > 0).then_some(order);
                            }
                            ui.label(theme::muted("0 = 自动"));
                        });
                        ui.add_space(6.0);
                        ui.label(RichText::new("关联人物").strong());
                        let characters: Vec<_> = self
                            .snapshot
                            .as_ref()
                            .map(|s| {
                                s.result
                                    .analysis
                                    .symbols
                                    .character_order
                                    .iter()
                                    .map(|id| {
                                        (
                                            id.clone(),
                                            s.result.analysis.symbols.characters[id]
                                                .display
                                                .clone(),
                                        )
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();
                        if characters.is_empty() {
                            ui.label(theme::muted("先到人物页创建角色"));
                        }
                        ui.horizontal_wrapped(|ui| {
                            for (id, display) in characters {
                                let mut selected = editor.draft.characters.contains(&id);
                                if ui
                                    .checkbox(&mut selected, display)
                                    .on_hover_text(&id)
                                    .changed()
                                {
                                    if selected {
                                        editor.draft.characters.push(id);
                                    } else {
                                        editor.draft.characters.retain(|c| c != &id);
                                    }
                                }
                            }
                        });
                        ui.add_space(6.0);
                        theme::card().show(ui, |ui| {
                            ui.label(RichText::new("前置要求 / 准入条件").strong());
                            if let Some(snapshot) = &self.snapshot {
                                let context = super::condition_authoring::Context {
                                    project: &self.project,
                                    compiled: &snapshot.result,
                                    version: self.version,
                                    baseline: &editor.baseline,
                                    current_baseline: condition_baseline.clone(),
                                    draft_signature: condition_draft_signature.clone(),
                                    composing,
                                };
                                super::condition_authoring::show(
                                    ui,
                                    &context,
                                    &mut editor.condition_forms,
                                    super::condition_authoring::owner(&editor.draft.id, None),
                                    worldline_core::condition_authoring::ConditionSlot::EventAfter,
                                    &mut editor.draft.after,
                                );
                            } else {
                                field(ui, "前置条件 after（可选）", &mut editor.draft.after);
                            }
                            ui.label(theme::muted("准入通过后才执行前置效果。"));
                        });
                        ui.add_space(6.0);
                        effects::effect_cards(
                            ui,
                            &mut editor.draft.effects,
                            self.snapshot.as_ref().map(|s| &s.result.analysis.catalog),
                        );
                        ui.add_space(6.0);
                        ui.label(RichText::new("完整事件正文（高级编辑）").strong());
                        let body_edit = egui::TextEdit::multiline(&mut editor.draft.body)
                            .code_editor()
                            .desired_rows(8)
                            .desired_width(f32::INFINITY)
                            .show(ui);
                        ui.menu_button("插入对象链接（名称或别名）", |ui| {
                            ui.text_edit_singleline(&mut self.link_query);
                            let candidates = self
                                .snapshot
                                .as_ref()
                                .map(|s| {
                                    self.applied_object_candidates(
                                        ui,
                                        &s.result.analysis.catalog,
                                        "event-body-link",
                                        &self.link_query,
                                        &[],
                                    )
                                })
                                .unwrap_or_default();
                            egui::ScrollArea::vertical()
                                .max_height(220.0)
                                .show(ui, |ui| {
                                    for object in candidates {
                                        if super::object_picker::candidate_row_at_revision(
                                            ui,
                                            &object,
                                            Some(&self.project.root),
                                            false,
                                            (self.version, &self.link_query),
                                        )
                                        .clicked()
                                        {
                                            let file = editor.path.to_string_lossy();
                                            let source = worldline_core::navigation::link_source(
                                                &object.target,
                                                &object.display,
                                                &file,
                                            )
                                            .or_else(|_| {
                                                worldline_core::navigation::link_source(
                                                    &object.target,
                                                    &object.target.id,
                                                    &file,
                                                )
                                            });
                                            match source {
                                                Ok(source) => {
                                                    let count = editor.draft.body.chars().count();
                                                    let range = body_edit.state.cursor.char_range();
                                                    let (start, end) = range
                                                        .map(|r| {
                                                            (
                                                                r.primary
                                                                    .index
                                                                    .min(r.secondary.index),
                                                                r.primary
                                                                    .index
                                                                    .max(r.secondary.index),
                                                            )
                                                        })
                                                        .unwrap_or((count, count));
                                                    let byte = |i| {
                                                        editor
                                                            .draft
                                                            .body
                                                            .char_indices()
                                                            .nth(i)
                                                            .map(|(p, _)| p)
                                                            .unwrap_or(editor.draft.body.len())
                                                    };
                                                    editor.draft.body.replace_range(
                                                        byte(start)..byte(end),
                                                        &source,
                                                    );
                                                }
                                                Err(error) => self.io_error = Some(error),
                                            }
                                            ui.close();
                                        }
                                    }
                                });
                        });
                        apply |= crate::theme::add_enabled(
                            ui,
                            temporal_ready,
                            theme::primary(if editor.original.is_some() {
                                "应用更改"
                            } else {
                                "创建并加入时间线"
                            }),
                        )
                        .clicked();
                        if let Some(id) = &editor.original {
                            self.object_links(
                                ui,
                                &worldline_core::catalog::TargetRef::new("event", id),
                            );
                            ui.horizontal(|ui| {
                                if ui
                                    .button(if editor.draft.period.is_some() {
                                        "添加后续约束"
                                    } else {
                                        "连到另一事件"
                                    })
                                    .clicked()
                                {
                                    self.link_from = Some(id.clone());
                                }
                                if ui.button("查看源码").clicked() {
                                    let node = self
                                        .snapshot
                                        .as_ref()
                                        .and_then(|s| {
                                            s.result
                                                .analysis
                                                .graph
                                                .nodes
                                                .iter()
                                                .find(|n| &n.name == id)
                                        })
                                        .cloned();
                                    if let Some(node) = node {
                                        self.jump_to_file(&node.file, node.line, 1);
                                    }
                                }
                            });
                            ui.add_space(8.0);
                            delete = ui
                                .small_button(RichText::new("删除事件").color(ERROR()))
                                .on_hover_text("仍有引用时阻止删除;删除后可撤销")
                                .clicked();
                        }
                        // 在所有子 TextEdit/ScrollArea 结束后，由本次 Tab 意图请求显露。
                        super::condition_authoring::focus::finish(ui, !composing);
                    });
                if apply && !composing && !editor.condition_forms.has_pending() {
                    let draft = editor.draft.clone();
                    let original = editor.original.clone();
                    let path = editor.path.clone();
                    if self.commit("事件已更新", |p| {
                        p.write_event_at_baseline(
                            &path,
                            original.as_deref(),
                            &draft,
                            &editor.baseline,
                        )
                    }) {
                        editor.original = Some(draft.id);
                        editor.baseline = self.project.content_baseline();
                        editor.condition_forms.clear();
                    }
                }
                if delete && !composing {
                    if let Some(id) = editor.original.clone() {
                        close = self.commit("事件已删除,可撤销", |p| p.remove_event(&id));
                    }
                }
                let condition_navigation = editor.condition_forms.take_navigation();
                if !close || composing {
                    self.event_editor = Some(editor);
                    if let Some(request) = condition_navigation {
                        self.navigate_condition_authoring(ctx, request);
                    }
                }
            });
    }

    pub(super) fn world_tab(&mut self, ctx: &egui::Context) {
        let protected = self.world_draft_dirty();
        let template_index = self
            .snapshot
            .as_ref()
            .map(|snapshot| snapshot.template_index.clone())
            .unwrap_or_else(|| self.project.template_index());
        let catalog = self
            .snapshot
            .as_ref()
            .map(|snapshot| snapshot.result.analysis.catalog.clone())
            .unwrap_or_default();
        let world = self
            .snapshot
            .as_ref()
            .and_then(|s| s.result.analysis.world.clone());
        let mut draft = self.world_editor.take().unwrap_or_else(|| {
            world
                .as_ref()
                .map(|w| WorldDraft {
                    id: w.id.clone(),
                    display: w.display.clone(),
                    description: w.description.clone(),
                    properties: w.properties.clone().into_iter().collect(),
                })
                .unwrap_or_else(|| WorldDraft {
                    id: "my_world".into(),
                    display: "新的世界".into(),
                    ..Default::default()
                })
        });
        egui::CentralPanel::default().frame(theme::panel().fill(BG())).show(ctx, |ui| {
            self.page_heading(ui, "世界观", "整个工程的共同设定 · 所有引用文件共享这一份世界观");
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.set_max_width(860.0);
                theme::card().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("◎  世界档案").size(20.0).color(ACCENT()));
                        ui.label(theme::muted(format!("{} 份文件共享", self.project.documents.len())));
                    });
                    field(ui, "世界 ID", &mut draft.id); field(ui, "世界名称", &mut draft.display);
                    ui.label(theme::muted("世界描述 / 创作共识"));
                    ui.add(egui::TextEdit::multiline(&mut draft.description).desired_rows(7).desired_width(f32::INFINITY).hint_text("时代、地域、规则、历史与叙事基调…"));
                });
                ui.add_space(12.0);
                theme::card().show(ui, |ui| {
                    ui.label(RichText::new("共同属性").strong().size(16.0));
                    let schema_keys = world.as_ref().map(|world| {
                        self.schema_authoring_entry(ui, &worldline_core::TargetRef::new("world", &world.id), protected)
                    }).unwrap_or_default();
                    properties_excluding(ui, &mut draft.properties, &catalog, self.project.compile_options(), &schema_keys);
                    if let Some(suggestion) = super::templates::template_panel_excluding(
                        ui,
                        "world",
                        None,
                        &template_index,
                        &catalog,
                        &mut draft.properties,
                        &schema_keys.keys,
                    )
                    {
                        if let Some(world) = &world {
                            self.edit_relation(
                                None,
                                Some(worldline_core::TargetRef::new("world", &world.id)),
                            );
                            self.message = Some(format!(
                                "已按“{suggestion}”打开关系草稿；仍需明确关系类型和另一端"
                            ));
                        } else {
                            self.message =
                                Some("请先保存世界观，再从模板建议打开关系草稿".into());
                        }
                    }
                });
                ui.add_space(12.0);
                if ui.add(theme::primary("应用世界观")).clicked() { self.commit("世界观已更新", |p| p.write_world(&draft)); }
                if ui.button("恢复为已应用内容").clicked() { draft = self.current_world_draft(); }
                ui.label(theme::muted("浏览不会产生草稿；改动后先应用，再保存工程。"));
                if let Some(world) = &world { self.object_links(ui, &worldline_core::catalog::TargetRef::new("world", &world.id)); }
                ui.add_space(18.0);
                theme::card().show(ui, |ui| {
                    ui.label(RichText::new("工程总入口").strong());
                    ui.label(theme::muted(self.project.entry.display().to_string()));
                    ui.label(theme::muted("工作区递归索引全部源码。导出保留完整目录，其他作者可直接打开继续编辑。"));
                    if ui.button("编辑总入口").clicked() { let path = self.project.entry.to_string_lossy().into_owned(); self.jump_to_file(&path, 1, 1); }
                    if let Some(w) = &world {
                        if Path::new(&w.file) != self.project.entry && ui.button("查看世界观源文件").clicked() { self.jump_to_file(&w.file, w.line, 1); }
                    }
                });
            });
        });
        self.world_editor = Some(draft);
    }
}

#[cfg(test)]
mod object_reference_tests {
    use super::*;
    use worldline_core::catalog::TargetRef;

    #[test]
    fn generic_property_form_keeps_a_typed_reference_without_casting_it_to_text() {
        let context = egui::Context::default();
        let mut values = vec![(
            "home".into(),
            PropertyValue::Ref(TargetRef::new("entity", "tower")),
        )];
        let _ = context.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| properties(ui, &mut values));
        });
        assert!(matches!(
            &values[0].1,
            PropertyValue::Ref(target) if target.kind == "entity" && target.id == "tower"
        ));
    }
}
