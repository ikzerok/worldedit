//! 事件与世界观的结构化表单;源码操作统一提交给 core。
use super::WorldeditApp;
use crate::theme::{self, *};
use egui::RichText;
use std::path::Path;
use worldline_core::ast::{EffectWhen, PropertyValue};
use worldline_core::authoring::{EffectDraft, WorldDraft};
use worldline_core::catalog::Catalog;

pub(super) fn field(ui: &mut egui::Ui, label: &str, value: &mut String) {
    ui.label(theme::muted(label));
    ui.add(egui::TextEdit::singleline(value).desired_width(f32::INFINITY));
}

fn effect_label(when: EffectWhen) -> &'static str {
    match when {
        EffectWhen::Enter => "前置 · enter",
        EffectWhen::Exit => "后置 · exit",
        EffectWhen::Done => "自然完成 · done",
    }
}

fn effect_cards(ui: &mut egui::Ui, effects: &mut Vec<EffectDraft>, catalog: Option<&Catalog>) {
    ui.label(RichText::new("事件效果").strong());
    ui.label(theme::muted("前置在准入通过后执行；后置在完成或离开时执行。自然完成效果仅在正文走完时执行，早于后置效果。"));
    let mut remove = None;
    for (i, effect) in effects.iter_mut().enumerate() {
        ui.push_id(("effect", i), |ui| {
            theme::card().show(ui, |ui| {
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_salt("when")
                        .selected_text(effect_label(effect.when))
                        .show_ui(ui, |ui| {
                            for when in [EffectWhen::Enter, EffectWhen::Exit, EffectWhen::Done] {
                                ui.selectable_value(&mut effect.when, when, effect_label(when));
                            }
                        });
                    if ui.small_button("删除").clicked() {
                        remove = Some(i);
                    }
                });
                field(
                    ui,
                    "执行条件（可选，留空则始终执行）",
                    &mut effect.condition,
                );
                ui.label(theme::muted(
                    "例如 has(mood, calm)；同一时机按卡片顺序执行。",
                ));
                ui.label(theme::muted("动作代码"));
                ui.add(
                    egui::TextEdit::multiline(&mut effect.actions)
                        .code_editor()
                        .desired_rows(3)
                        .desired_width(f32::INFINITY)
                        .hint_text("become mood with alert"),
                );
                if let Some(catalog) = catalog {
                    super::tags::condition(ui, catalog, &mut effect.condition);
                    super::tags::actions(ui, catalog, &mut effect.actions);
                }
            });
        });
        ui.add_space(6.0);
    }
    if let Some(i) = remove {
        effects.remove(i);
    }
    if ui.button("＋ 添加效果").clicked() {
        effects.push(EffectDraft {
            when: EffectWhen::Enter,
            condition: String::new(),
            actions: String::new(),
        });
    }
}

pub(super) fn properties(ui: &mut egui::Ui, values: &mut Vec<(String, PropertyValue)>) {
    let mut remove = None;
    for (i, (name, value)) in values.iter_mut().enumerate() {
        ui.push_id(i, |ui| {
            ui.label(super::reading::property_label(name));
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(name)
                        .hint_text("属性 ID")
                        .desired_width((ui.available_width() - 38.0).max(90.0)),
                );
                if ui.small_button("×").clicked() {
                    remove = Some(i);
                }
            });
            ui.horizontal(|ui| {
                let current = match value {
                    PropertyValue::Str(_) => 0,
                    PropertyValue::Num(_) => 1,
                    PropertyValue::Bool(_) => 2,
                };
                let mut kind = current;
                egui::ComboBox::from_id_salt("type")
                    .width(65.0)
                    .selected_text(["文本", "数值", "布尔"][kind])
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut kind, 0, "文本");
                        ui.selectable_value(&mut kind, 1, "数值");
                        ui.selectable_value(&mut kind, 2, "布尔");
                    });
                if kind != current {
                    *value = match kind {
                        1 => PropertyValue::Num(0.0),
                        2 => PropertyValue::Bool(false),
                        _ => PropertyValue::Str(String::new()),
                    };
                }
                match value {
                    PropertyValue::Str(text) => {
                        ui.add(
                            egui::TextEdit::multiline(text)
                                .desired_rows(3)
                                .desired_width(ui.available_width())
                                .hint_text("属性值"),
                        );
                    }
                    PropertyValue::Num(number) => {
                        ui.add(egui::DragValue::new(number).speed(1.0));
                    }
                    PropertyValue::Bool(value) => {
                        ui.checkbox(value, "是 / 否");
                    }
                }
            });
            ui.add_space(6.0);
        });
    }
    if let Some(index) = remove {
        values.remove(index);
    }
    if ui.button("＋ 添加属性").clicked() {
        values.push((String::new(), PropertyValue::Str(String::new())));
    }
}

impl WorldeditApp {
    pub(super) fn event_inspector(&mut self, ctx: &egui::Context) {
        egui::SidePanel::right("event-inspector")
            .default_width(300.0)
            .width_range(270.0..=410.0)
            .frame(theme::panel())
            .show(ctx, |ui| {
                let Some(mut editor) = self.event_editor.take() else {
                    ui.label(RichText::new("事件详情").strong().size(17.0));
                    ui.add_space(18.0);
                    theme::card().show(ui, |ui| {
                        ui.label(RichText::new("开始编织你的世界").color(ACCENT));
                        ui.label(theme::muted(
                            "点击卡片编辑内容与人物。时段内用连线添加先后约束,未连接的事件保持自由。",
                        ));
                    });
                    ui.add_space(10.0);
                    if ui.add(theme::primary("＋ 新建事件")).clicked() {
                        self.new_event(None);
                    }
                    return;
                };
                let mut close = false;
                let mut apply = false;
                let mut delete = false;
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(if editor.original.is_some() {
                            "事件详情"
                        } else {
                            "创建事件"
                        })
                        .strong()
                        .size(17.0),
                    );
                    if ui.add(theme::primary("应用更改")).clicked() { apply = true; }
                    if ui.small_button("×").clicked() {
                        close = true;
                    }
                });
                ui.label(theme::muted("应用更改后同步到源文件与全部视图"));
                ui.separator();
                egui::ScrollArea::vertical()
                    .id_salt("event-form")
                    .show(ui, |ui| {
                        ui.label(theme::muted("事件 ID"));
                        ui.add_enabled(
                            editor.original.is_none(),
                            egui::TextEdit::singleline(&mut editor.draft.id)
                                .desired_width(f32::INFINITY),
                        );
                        field(ui, "事件名称 / 简述", &mut editor.draft.summary);
                        if let Err(error) = super::choices::choice_cards(ui, &mut editor.draft, self.snapshot.as_ref().map(|s| &s.result.analysis.graph), self.snapshot.as_ref().map(|s| &s.result.analysis.catalog)) {
                            self.io_error = Some(error);
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
                                    ui.add_enabled_ui(editor.original.is_none(), |ui| {
                                        ui.selectable_value(
                                            &mut editor.path,
                                            path.clone(),
                                            path.strip_prefix(&self.project.root)
                                                .unwrap_or(path)
                                                .display()
                                                .to_string(),
                                        );
                                    });
                                }
                            });
                        field(
                            ui,
                            "故事线 ID（可创建新故事线）",
                            &mut editor.draft.storyline,
                        );
                        ui.label(theme::muted("所属时段"));
                        let previous_period = editor.draft.period.clone();
                        egui::ComboBox::from_id_salt("event-period").selected_text(editor.draft.period.as_deref().unwrap_or("未分配时段")).show_ui(ui, |ui| {
                            ui.selectable_value(&mut editor.draft.period, None, "未分配时段");
                            if let Some(snapshot) = &self.snapshot {
                                for period in &snapshot.result.analysis.timeline.periods {
                                    ui.selectable_value(&mut editor.draft.period, Some(period.id.clone()), &period.display);
                                }
                            }
                        });
                        if editor.draft.period != previous_period { editor.draft.predecessors.clear(); }
                        if let Some(period) = &editor.draft.period {
                            ui.label(theme::muted("明确晚于以下事件（可不选）"));
                            if let Some(snapshot) = &self.snapshot {
                                for event in snapshot.result.program.events.iter().filter(|e| e.period.as_ref() == Some(period) && e.name != editor.draft.id) {
                                    let mut selected = editor.draft.predecessors.contains(&event.name);
                                    if ui.checkbox(&mut selected, event.summary.as_deref().unwrap_or(&event.name)).changed() {
                                        if selected { editor.draft.predecessors.push(event.name.clone()); }
                                        else { editor.draft.predecessors.retain(|id| id != &event.name); }
                                    }
                                }
                            }
                            ui.label(theme::muted("未连接的事件可无序发生,也可与有序事件链并列。"));
                        }
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
                            field(ui, "前置条件 after（可选）", &mut editor.draft.after);
                            ui.label(theme::muted("例如 has(mood, calm)，可用 and、or、not 组合多个状态要求。准入通过后才执行前置效果。"));
                            if let Some(snapshot) = &self.snapshot {
                                super::tags::condition(ui, &snapshot.result.analysis.catalog, &mut editor.draft.after);
                            }
                        });
                        ui.add_space(6.0);
                        effect_cards(ui, &mut editor.draft.effects,
                            self.snapshot.as_ref().map(|s| &s.result.analysis.catalog));
                        ui.add_space(6.0);
                        ui.label(RichText::new("完整事件正文（高级编辑）").strong());
                        let body_edit =
                            egui::TextEdit::multiline(&mut editor.draft.body)
                                .code_editor()
                                .desired_rows(8)
                                .desired_width(f32::INFINITY).show(ui);
                        ui.menu_button("插入对象链接（名称或别名）", |ui| {
                            ui.text_edit_singleline(&mut self.link_query);
                            let candidates = self.snapshot.as_ref().map(|s| s.result.analysis.catalog.search_objects(&self.link_query)).unwrap_or_default();
                            egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                                for object in candidates {
                                    if ui.button(format!("{} · {} ({})", super::catalog::kind_label(&object.target.kind), object.display, object.target.id)).clicked() {
                                        let file = editor.path.to_string_lossy();
                                        let source = worldline_core::navigation::link_source(&object.target, &object.display, &file)
                                            .or_else(|_| worldline_core::navigation::link_source(&object.target, &object.target.id, &file));
                                        match source {
                                            Ok(source) => {
                                                let count = editor.draft.body.chars().count();
                                                let range = body_edit.state.cursor.char_range();
                                                let (start, end) = range.map(|r| (r.primary.index.min(r.secondary.index), r.primary.index.max(r.secondary.index))).unwrap_or((count, count));
                                                let byte = |i| editor.draft.body.char_indices().nth(i).map(|(p, _)| p).unwrap_or(editor.draft.body.len());
                                                editor.draft.body.replace_range(byte(start)..byte(end), &source);
                                            }
                                            Err(error) => self.io_error = Some(error),
                                        }
                                        ui.close();
                                    }
                                }
                            });
                        });
                        apply |= ui
                            .add_sized(
                                [ui.available_width(), 36.0],
                                theme::primary(if editor.original.is_some() {
                                    "应用更改"
                                } else {
                                    "创建并加入时间线"
                                }),
                            )
                            .clicked();
                        if let Some(id) = &editor.original {
                            self.object_links(ui, &worldline_core::catalog::TargetRef::new("event", id));
                            ui.horizontal(|ui| {
                                if ui.button(if editor.draft.period.is_some() { "添加后续约束" } else { "连到另一事件" }).clicked() {
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
                                .small_button(RichText::new("删除事件").color(ERROR))
                                .on_hover_text("仍有引用时阻止删除;删除后可撤销")
                                .clicked();
                        }
                    });
                if apply {
                    let draft = editor.draft.clone();
                    let original = editor.original.clone();
                    let path = editor.path.clone();
                    if self.commit("事件已更新", |p| {
                        p.write_event(&path, original.as_deref(), &draft)
                    }) {
                        editor.original = Some(draft.id);
                    }
                }
                if delete {
                    if let Some(id) = editor.original.clone() {
                        close = self.commit("事件已删除,可撤销", |p| p.remove_event(&id));
                    }
                }
                if !close {
                    self.event_editor = Some(editor);
                }
            });
    }

    pub(super) fn world_tab(&mut self, ctx: &egui::Context) {
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
        egui::CentralPanel::default().frame(theme::panel().fill(BG)).show(ctx, |ui| {
            self.page_heading(ui, "世界观", "整个工程的共同设定 · 所有引用文件共享这一份世界观");
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.set_max_width(860.0);
                theme::card().show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("◎  世界档案").size(20.0).color(ACCENT));
                        ui.label(theme::muted(format!("{} 份文件共享", self.project.documents.len())));
                    });
                    field(ui, "世界 ID", &mut draft.id); field(ui, "世界名称", &mut draft.display);
                    ui.label(theme::muted("世界描述 / 创作共识"));
                    ui.add(egui::TextEdit::multiline(&mut draft.description).desired_rows(7).desired_width(f32::INFINITY).hint_text("时代、地域、规则、历史与叙事基调…"));
                });
                ui.add_space(12.0);
                theme::card().show(ui, |ui| { ui.label(RichText::new("共同属性").strong().size(16.0)); properties(ui, &mut draft.properties); });
                ui.add_space(12.0);
                if ui.add(theme::primary("应用世界观")).clicked() { self.commit("世界观已更新", |p| p.write_world(&draft)); }
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
