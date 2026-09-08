//! 人物档案、可编辑关系图与反向事件索引。
use super::inspector::{field, properties};
use super::{CharacterEditor, Tab, WorldeditApp};
use crate::theme::{self, *};
use crate::visual::{draw_arrow, truncated};
use egui::{Pos2, Rect, RichText, Sense, Stroke, Vec2};
use std::collections::HashMap;
use std::path::PathBuf;
use worldline_core::authoring::CharacterDraft;

impl WorldeditApp {
    pub(super) fn select_character(&mut self, id: &str) {
        let info = self
            .snapshot
            .as_ref()
            .and_then(|s| s.result.analysis.symbols.characters.get(id))
            .cloned();
        if let Some(info) = info {
            self.character_editor = Some(CharacterEditor {
                path: PathBuf::from(info.decl_file),
                original: Some(id.into()),
                draft: CharacterDraft {
                    id: id.into(),
                    display: info.display,
                    properties: info.properties.into_iter().collect(),
                    relations: info
                        .relations
                        .into_iter()
                        .map(|r| (r.target, r.label))
                        .collect(),
                },
            });
        }
    }
    fn new_character(&mut self) {
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        let characters = &snapshot.result.analysis.symbols.characters;
        let mut index = 1;
        while characters.contains_key(&format!("character_{index}")) {
            index += 1;
        }
        let path = snapshot
            .result
            .analysis
            .symbols
            .character_order
            .first()
            .and_then(|id| characters.get(id))
            .map(|c| PathBuf::from(&c.decl_file))
            .unwrap_or_else(|| self.project.entry.clone());
        self.character_editor = Some(CharacterEditor {
            path,
            original: None,
            draft: CharacterDraft {
                id: format!("character_{index}"),
                display: "新人物".into(),
                ..Default::default()
            },
        });
    }
    pub(super) fn characters_tab(&mut self, ctx: &egui::Context) {
        self.character_inspector(ctx);
        let Some(snapshot) = &self.snapshot else {
            return;
        };
        let symbols = snapshot.result.analysis.symbols.clone();
        let ids = &symbols.character_order;
        egui::CentralPanel::default()
            .frame(theme::panel().fill(BG))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.heading("人物");
                        ui.label(theme::muted(format!(
                            "{} 位人物 · 全局 ID 与反向事件索引",
                            ids.len()
                        )));
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(theme::primary("＋ 新建人物")).clicked() {
                            self.new_character();
                        }
                    });
                });
                ui.add_space(18.0);
                egui::ScrollArea::horizontal()
                    .id_salt("character-cards")
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            for id in ids {
                                let info = &symbols.characters[id];
                                let selected = self
                                    .character_editor
                                    .as_ref()
                                    .is_some_and(|c| c.draft.id == *id);
                                if ui
                                    .add_sized(
                                        [164.0, 82.0],
                                        egui::Button::selectable(
                                            selected,
                                            RichText::new(format!(
                                                "{}\n{} · {} 个事件",
                                                info.display,
                                                id,
                                                info.events.len()
                                            ))
                                            .size(13.0),
                                        ),
                                    )
                                    .clicked()
                                {
                                    self.select_character(id);
                                }
                            }
                        });
                    });
                ui.add_space(18.0);
                ui.label(RichText::new("人物关系图").strong().size(17.0));
                ui.label(theme::muted(if self.character_link.is_some() {
                    "点击目标人物建立关系,随后在档案中编辑关系名称"
                } else {
                    "点击人物编辑档案 · 拖动卡片整理布局 · 从圆点拖到另一人物建立关系"
                }));
                ui.add_space(12.0);
                let mut select = None;
                let mut connect = None;
                egui::ScrollArea::both()
                    .id_salt("character-graph")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let radius = (ids.len() as f32 * 25.0).max(175.0);
                        let total = Vec2::new(
                            (radius * 2.0 + 260.0).max(ui.available_width()),
                            (radius * 2.0 + 120.0).max(ui.available_height()),
                        );
                        let (canvas, _) = ui.allocate_exact_size(total, Sense::click());
                        let painter = ui.painter_at(canvas);
                        painter.rect_filled(canvas, 12, PANEL);
                        let center = Pos2::new(total.x * 0.5, total.y * 0.5);
                        painter.circle_stroke(
                            canvas.min + center.to_vec2(),
                            radius,
                            Stroke::new(1.0_f32, BORDER),
                        );
                        let rects: HashMap<String, Rect> = ids
                            .iter()
                            .enumerate()
                            .map(|(i, id)| {
                                let angle =
                                    i as f32 / ids.len().max(1) as f32 * std::f32::consts::TAU;
                                let default = center
                                    + Vec2::new(
                                        angle.cos() * radius - 84.0,
                                        angle.sin() * radius - 40.0,
                                    );
                                let pos = *self.character_positions.get(id).unwrap_or(&default);
                                (
                                    id.clone(),
                                    Rect::from_min_size(
                                        canvas.min + pos.to_vec2(),
                                        Vec2::new(168.0, 80.0),
                                    ),
                                )
                            })
                            .collect();
                        for id in ids {
                            for relation in &symbols.characters[id].relations {
                                let (Some(from), Some(to)) =
                                    (rects.get(id), rects.get(&relation.target))
                                else {
                                    continue;
                                };
                                let direction = (to.center() - from.center()).normalized();
                                if !direction.is_finite() {
                                    continue;
                                }
                                let start = from.center() + direction * 48.0;
                                let end = to.center() - direction * 86.0;
                                painter.line_segment(
                                    [start, end],
                                    Stroke::new(1.5_f32, ACCENT.gamma_multiply(0.65)),
                                );
                                draw_arrow(&painter, start, end, ACCENT);
                                painter.text(
                                    start.lerp(end, 0.5) + Vec2::new(0.0, -12.0),
                                    egui::Align2::CENTER_BOTTOM,
                                    truncated(&relation.label, 18),
                                    egui::FontId::proportional(12.0),
                                    ACCENT,
                                );
                            }
                        }
                        for (id, rect) in &rects {
                            let selected = self
                                .character_editor
                                .as_ref()
                                .is_some_and(|c| c.draft.id == *id);
                            painter.rect_filled(*rect, 12, CARD);
                            painter.rect_stroke(
                                *rect,
                                12,
                                Stroke::new(
                                    if selected { 2.0_f32 } else { 1.0_f32 },
                                    if selected { ACCENT } else { BORDER },
                                ),
                                egui::StrokeKind::Inside,
                            );
                            let info = &symbols.characters[id];
                            painter.text(
                                rect.center() - Vec2::new(0.0, 13.0),
                                egui::Align2::CENTER_CENTER,
                                truncated(&info.display, 12),
                                egui::FontId::proportional(16.0),
                                TEXT,
                            );
                            painter.text(
                                rect.center() + Vec2::new(0.0, 14.0),
                                egui::Align2::CENTER_CENTER,
                                truncated(id, 20),
                                egui::FontId::monospace(11.0),
                                MUTED,
                            );
                            let response = ui.interact(
                                *rect,
                                egui::Id::new(("person", id)),
                                Sense::click_and_drag(),
                            );
                            if response.clicked() {
                                if let Some(from) = self.character_link.clone() {
                                    if &from != id {
                                        connect = Some((from, id.clone()));
                                    }
                                } else {
                                    select = Some(id.clone());
                                }
                            }
                            if response.dragged() && self.character_link.is_none() {
                                let pos = self.character_positions.entry(id.clone()).or_insert(
                                    Pos2::new(rect.min.x - canvas.min.x, rect.min.y - canvas.min.y),
                                );
                                *pos += ui.input(|i| i.pointer.delta());
                                pos.x = pos.x.max(10.0);
                                pos.y = pos.y.max(10.0);
                            }
                            let port =
                                Rect::from_center_size(rect.right_center(), Vec2::splat(20.0));
                            painter.circle_filled(port.center(), 4.5, ACCENT);
                            let response = ui.interact(
                                port,
                                egui::Id::new(("person-port", id)),
                                Sense::click_and_drag(),
                            );
                            if response.drag_started() || response.clicked() {
                                self.character_link = Some(id.clone());
                            }
                        }
                        if let Some(from) = &self.character_link {
                            if let (Some(rect), Some(pointer)) =
                                (rects.get(from), ui.input(|i| i.pointer.hover_pos()))
                            {
                                painter.line_segment(
                                    [rect.right_center(), pointer],
                                    Stroke::new(1.5_f32, ACCENT),
                                );
                                if ui.input(|i| i.pointer.any_released()) {
                                    if let Some((to, _)) = rects
                                        .iter()
                                        .find(|(id, r)| *id != from && r.contains(pointer))
                                    {
                                        connect = Some((from.clone(), to.clone()));
                                    }
                                }
                            }
                        }
                        if ids.is_empty() {
                            painter.text(
                                canvas.center(),
                                egui::Align2::CENTER_CENTER,
                                "创建人物后,在这里连接他们的关系",
                                egui::FontId::proportional(17.0),
                                MUTED,
                            );
                        }
                    });
                if let Some(id) = select {
                    self.select_character(&id);
                }
                if let Some((from, to)) = connect {
                    self.select_character(&from);
                    if let Some(mut editor) = self.character_editor.clone() {
                        editor.draft.relations.push((to, "关联".into()));
                        if self.commit("人物关系已创建", |p| {
                            p.write_character(&editor.path, Some(&from), &editor.draft)
                        }) {
                            self.character_link = None;
                            self.character_editor = Some(editor);
                        }
                    }
                }
            });
    }

    fn character_inspector(&mut self, ctx: &egui::Context) {
        egui::SidePanel::right("character-inspector")
            .default_width(320.0)
            .width_range(285.0..=420.0)
            .frame(theme::panel())
            .show(ctx, |ui| {
                let Some(mut editor) = self.character_editor.take() else {
                    ui.label(RichText::new("人物档案").strong().size(17.0));
                    ui.label(theme::muted(
                        "选择一位人物,编辑姓名、属性和关系,查看其关联事件。",
                    ));
                    return;
                };
                let mut close = false;
                let mut apply = false;
                ui.horizontal(|ui| {
                    ui.label(RichText::new("人物档案").strong().size(17.0));
                    if ui.small_button("×").clicked() {
                        close = true;
                    }
                });
                egui::ScrollArea::vertical()
                    .id_salt("person-form")
                    .show(ui, |ui| {
                        field(
                            ui,
                            "角色 ID（改名会同步全部结构引用）",
                            &mut editor.draft.id,
                        );
                        field(ui, "姓名", &mut editor.draft.display);
                        ui.label(theme::muted(
                            editor
                                .path
                                .strip_prefix(&self.project.root)
                                .unwrap_or(&editor.path)
                                .display()
                                .to_string(),
                        ));
                        ui.separator();
                        ui.label(RichText::new("人物属性").strong());
                        ui.menu_button("＋ 常用资料栏目", |ui| {
                            for (key, label) in [
                                ("appearance", "外貌与识别特征"),
                                ("background", "背景经历"),
                                ("personality", "性格与行为习惯"),
                                ("motivation", "欲望与动机"),
                                ("boundaries", "底线与恐惧"),
                                ("voice", "说话方式"),
                                ("dialogue_examples", "口吻例句"),
                            ] {
                                let exists =
                                    editor.draft.properties.iter().any(|(name, _)| name == key);
                                if ui.add_enabled(!exists, egui::Button::new(label)).clicked() {
                                    editor.draft.properties.push((
                                        key.into(),
                                        worldline_core::ast::PropertyValue::Str(String::new()),
                                    ));
                                    ui.close();
                                }
                            }
                        });
                        ui.label(theme::muted(
                            "栏目可留空；例句是创作参考，不会成为发生过的事件。",
                        ));
                        properties(ui, &mut editor.draft.properties);
                        ui.separator();
                        ui.label(RichText::new("人物关系").strong());
                        let ids = self
                            .snapshot
                            .as_ref()
                            .map(|s| s.result.analysis.symbols.character_order.clone())
                            .unwrap_or_default();
                        let mut remove = None;
                        for (i, (target, label)) in editor.draft.relations.iter_mut().enumerate() {
                            ui.push_id(("relation", i), |ui| {
                                ui.horizontal(|ui| {
                                    egui::ComboBox::from_id_salt("target")
                                        .width(150.0)
                                        .selected_text(target.as_str())
                                        .show_ui(ui, |ui| {
                                            for id in &ids {
                                                ui.selectable_value(target, id.clone(), id);
                                            }
                                        });
                                    if ui.small_button("×").clicked() {
                                        remove = Some(i);
                                    }
                                });
                                ui.add(
                                    egui::TextEdit::singleline(label)
                                        .hint_text("关系名称")
                                        .desired_width(f32::INFINITY),
                                );
                            });
                        }
                        if let Some(index) = remove {
                            editor.draft.relations.remove(index);
                        }
                        if ui.button("＋ 添加关系").clicked() {
                            editor.draft.relations.push((
                                ids.iter()
                                    .find(|id| **id != editor.draft.id)
                                    .cloned()
                                    .unwrap_or_default(),
                                "关联".into(),
                            ));
                        }
                        ui.add_space(8.0);
                        apply = ui
                            .add_sized([ui.available_width(), 36.0], theme::primary("应用人物档案"))
                            .clicked();
                        ui.separator();
                        if let Some(id) = &editor.original {
                            self.object_links(
                                ui,
                                &worldline_core::catalog::TargetRef::new("character", id),
                            );
                        }
                        ui.label(RichText::new("关联事件 · 反向索引").strong());
                        let info = self
                            .snapshot
                            .as_ref()
                            .and_then(|s| {
                                editor
                                    .original
                                    .as_ref()
                                    .and_then(|id| s.result.analysis.symbols.characters.get(id))
                            })
                            .cloned();
                        if let Some(info) = info {
                            if info.events.is_empty() {
                                ui.label(theme::muted("尚未关联事件,可在事件详情中勾选此人物"));
                            }
                            for event in &info.events {
                                if ui
                                    .add_sized(
                                        [ui.available_width(), 32.0],
                                        egui::Button::new(format!("↗  {event}")),
                                    )
                                    .clicked()
                                {
                                    self.select_event(event);
                                    self.tab = Tab::Timeline;
                                }
                            }
                            ui.label(theme::muted("包含 with、meet、part 与效果块的引用"));
                            if ui.small_button("查看人物源文件").clicked() {
                                self.jump_to_file(&info.decl_file, info.decl_span.line, 1);
                            }
                        }
                    });
                if apply
                    && self.commit("人物档案与引用已更新", |p| {
                        p.write_character(&editor.path, editor.original.as_deref(), &editor.draft)
                    })
                {
                    editor.original = Some(editor.draft.id.clone());
                }
                if !close {
                    self.character_editor = Some(editor);
                }
            });
    }
}
