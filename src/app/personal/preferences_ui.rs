use super::{preferences::PreferencesAction, WorldeditApp};
use crate::theme::{
    self, AccentChoice, AppearancePreferences, BodyFamily, Density, PaletteId, StylePreset,
    ThemeMode,
};

impl WorldeditApp {
    pub(in crate::app) fn preferences_window(&mut self, ctx: &egui::Context) {
        let focus_entry = egui::Id::new("preferences-focus-entry");
        if !self.personal.preferences_open {
            let preview_was_open = self.personal.appearance_draft.is_some();
            self.personal.preferences_action(PreferencesAction::Cancel);
            ctx.data_mut(|data| data.remove::<bool>(focus_entry));
            // Escape can be handled by the application before this window is drawn.
            // Repaint once so a reduced-motion/idle canvas also restores committed colors.
            if preview_was_open {
                ctx.request_repaint();
            }
            return;
        }
        let entering = ctx.data_mut(|data| {
            let entering = data.get_temp::<bool>(focus_entry) != Some(true);
            data.insert_temp(focus_entry, true);
            entering
        });
        self.personal.begin_preferences();
        let ime = self.ime_composing
            || self.command_palette.ime
            || ctx.input(|input| {
                input
                    .events
                    .iter()
                    .any(|event| matches!(event, egui::Event::Ime(_)))
            });
        if !ime
            && ctx.input_mut(|input| input.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
        {
            self.personal.preferences_action(PreferencesAction::Escape);
            ctx.data_mut(|data| data.remove::<bool>(focus_entry));
            ctx.request_repaint();
            return;
        }
        let mut open = true;
        let mut action = None;
        let mut reset = false;
        let screen = ctx.screen_rect();
        let width = (screen.width() - 32.0).clamp(240.0, 650.0);
        let height = (screen.height() - 32.0).max(180.0);
        let before = *self.personal.appearance();
        egui::Window::new("阅读与外观 · 仅此设备")
            .id(egui::Id::new("appearance-preferences-v2"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_pos(egui::pos2(
                screen.left()
                    + ((screen.width() - width - theme::metrics().panel_margin * 2.0) * 0.5)
                        .max(8.0),
                screen.top() + 8.0,
            ))
            .default_width(width)
            .min_width(220.0)
            .max_width(width)
            .max_height(height)
            .constrain_to(screen.shrink(8.0))
            .show(ctx, |ui| {
                ui.add(egui::Label::new(theme::muted("正在预览 · 应用后保存，取消可还原")).wrap());
                // The resizable window can be shorter than the screen budget. Reserve
                // the action row before allocating the scroll area, including relayout
                // after a palette adds its fixed-mode notice.
                let footer_height =
                    theme::metrics().control_height + ui.spacing().item_spacing.y * 3.0 + 8.0;
                let visible_height = ui
                    .available_height()
                    .min(ui.clip_rect().bottom() - ui.cursor().top());
                let scroll_height = (visible_height - footer_height)
                    .min(height - 156.0)
                    .max(24.0);
                egui::ScrollArea::vertical()
                    .id_salt("appearance-options")
                    .max_height(scroll_height)
                    .min_scrolled_height(24.0)
                    // A focused choice must be visible in the very next layout. A zero
                    // animation duration alone still defers egui's content geometry.
                    .animated(false)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        if let Some(notice) = self.personal.storage_warning() {
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(notice).color(theme::WARNING()),
                                )
                                .wrap(),
                            );
                        }
                        let p = self
                            .personal
                            .appearance_draft
                            .as_mut()
                            .expect("preview initialized");
                        appearance_controls(ui, p, entering);
                        reset = ui
                            .button("恢复默认外观")
                            .on_hover_text("只重置当前预览；保留布局、参考和作者位置")
                            .clicked();
                        ui.separator();
                        preview(ui, p);
                    });
                ui.separator();
                ui.horizontal_wrapped(|ui| {
                    if ui.button("取消").clicked() {
                        action = Some(PreferencesAction::Cancel);
                    }
                    if ui.button("应用").clicked() {
                        action = Some(PreferencesAction::Apply);
                    }
                    if ui.add(theme::primary("确定")).clicked() {
                        action = Some(PreferencesAction::Accept);
                    }
                });
            });
        if reset {
            self.personal.reset_appearance_preview();
        }
        if !open {
            action = Some(PreferencesAction::WindowClose);
        }
        if let Some(action) = action {
            self.personal.preferences_action(action);
            ctx.request_repaint();
        }
        if !self.personal.preferences_open {
            ctx.data_mut(|data| data.remove::<bool>(focus_entry));
        }
        if before != *self.personal.appearance() {
            ctx.request_repaint();
        }
    }
}
fn appearance_controls(ui: &mut egui::Ui, p: &mut AppearancePreferences, entering: bool) {
    ui.label(egui::RichText::new("配色与结构").strong());
    ui.horizontal_wrapped(|ui| {
        ui.label("明暗偏好");
        let first = ui.selectable_value(&mut p.theme, ThemeMode::System, "跟随系统");
        if entering {
            first.request_focus();
        }
        ui.selectable_value(&mut p.theme, ThemeMode::Light, "浅色");
        ui.selectable_value(&mut p.theme, ThemeMode::Dark, "深色");
    });
    ui.horizontal_wrapped(|ui| {
        ui.label("配色");
        for palette in PaletteId::ALL {
            let support = palette.mode_support();
            let label = if support == theme::PaletteModeSupport::Both {
                palette.label().to_owned()
            } else {
                format!("{} · {}", palette.label(), support.label())
            };
            let response = ui
                .selectable_value(&mut p.palette, palette, label)
                .on_hover_text(format!("{} · {}", support.label(), palette.description()));
            if response.gained_focus() {
                response.scroll_to_me(Some(egui::Align::Center));
            }
        }
    });
    ui.add(egui::Label::new(theme::muted(p.palette.description())).wrap());
    if let Some(notice) = p.palette.mode_support().notice() {
        ui.add(egui::Label::new(egui::RichText::new(notice).color(theme::TEXT())).wrap());
    }
    ui.horizontal_wrapped(|ui| {
        ui.label("结构");
        ui.selectable_value(&mut p.style, StylePreset::Studio, "Studio 编辑台")
            .on_hover_text("贴边平面分栏，留白分组，连续正文");
        ui.selectable_value(&mut p.style, StylePreset::Manuscript, "Manuscript 稿纸")
            .on_hover_text("居中连续纸面与页边，工具在纸外；不改变作品分页");
        ui.selectable_value(&mut p.style, StylePreset::Technical, "Technical 工具台")
            .on_hover_text("贴边网格，栏头带与直角结构");
        ui.selectable_value(&mut p.style, StylePreset::Focus, "Focus 专注")
            .on_hover_text("居中连续内容，导航与参考按需召回；保留原布局选择");
        ui.selectable_value(&mut p.style, StylePreset::Ledger, "Ledger 档案")
            .on_hover_text("侧题签与对齐内容列；窄窗题签上移，不压缩正文");
    });
    ui.horizontal_wrapped(|ui| {
        ui.label("密度");
        ui.selectable_value(&mut p.density, Density::Compact, "紧凑 · 26");
        ui.selectable_value(&mut p.density, Density::Standard, "标准 · 30");
        ui.selectable_value(&mut p.density, Density::Spacious, "宽松 · 36");
    });
    ui.horizontal_wrapped(|ui| {
        ui.label("强调色");
        for (choice, label) in [
            (AccentChoice::Palette, "配色本色"),
            (AccentChoice::Pine, "松绿"),
            (AccentChoice::Plum, "梅紫"),
            (AccentChoice::Copper, "铜橙"),
            (AccentChoice::Indigo, "靛蓝"),
            (AccentChoice::IceCyan, "冰青"),
        ] {
            ui.selectable_value(&mut p.accent, choice, label);
        }
    });
    ui.add_space(8.0);
    ui.label(egui::RichText::new("阅读与源码").strong());
    ui.horizontal_wrapped(|ui| {
        ui.label("正文字体");
        ui.selectable_value(&mut p.body_family, BodyFamily::Sans, "Sans 比例");
        ui.selectable_value(&mut p.body_family, BodyFamily::Mono, "Mono 等宽")
            .on_hover_text("Latin 等宽字族；中文按字形回退，不保证中文与英文等格");
    });
    theme::slider(
        ui,
        egui::Slider::new(&mut p.body_size, 12.0..=36.0).text("正文字号"),
    );
    theme::slider(
        ui,
        egui::Slider::new(&mut p.source_size, 12.0..=28.0).text("源码字号"),
    );
    theme::slider(
        ui,
        egui::Slider::new(&mut p.line_spacing, 1.0..=2.1).text("行距倍数"),
    );
    theme::slider(
        ui,
        egui::Slider::new(&mut p.reading_width, 480.0..=1400.0).text("阅读宽度"),
    );
    ui.add_space(8.0);
    ui.label(egui::RichText::new("界面与辅助显示").strong());
    theme::slider(
        ui,
        egui::Slider::new(&mut p.ui_scale, 0.8..=2.0)
            .text("界面缩放")
            .custom_formatter(|v, _| format!("{:.0}%", v * 100.0))
            .custom_parser(|s| {
                s.trim_end_matches('%')
                    .parse::<f64>()
                    .ok()
                    .map(|v| v / 100.0)
            }),
    );
    ui.label(theme::muted(
        "Ctrl/Cmd + 加号 / 减号 缩放；Ctrl/Cmd + 0 恢复 100%",
    ));
    ui.checkbox(&mut p.high_contrast, "增强文字与控件对比");
    ui.checkbox(&mut p.reduce_motion, "减少动态效果与光标闪烁");
    p.normalize();
}
fn preview(ui: &mut egui::Ui, p: &AppearancePreferences) {
    let resolved = theme::resolve(p, ui.ctx().system_theme());
    let c = resolved.colors;
    egui::Frame::new()
        .fill(c.document)
        .inner_margin(12)
        .show(ui, |ui| {
            ui.scope(|ui| {
                let visuals = ui.visuals_mut();
                visuals.override_text_color = Some(c.text);
                visuals.selection.bg_fill = c.selection;
                visuals.selection.stroke = egui::Stroke::new(1.0_f32, c.selection_text);
                visuals.widgets.inactive.bg_fill = c.document;
                visuals.widgets.inactive.weak_bg_fill = c.hover;
                visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0_f32, c.control_border);
                visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0_f32, c.text);
                visuals.text_edit_bg_color = Some(c.document);
                visuals.disabled_alpha = if p.high_contrast { 1.0 } else { 0.99 };
                ui.add(
                    egui::Label::new(
                        egui::RichText::new("中文正文与阅读预览 Aa 0123")
                            .font(resolved.type_roles.body.clone())
                            .color(c.text),
                    )
                    .wrap(),
                );
                ui.add(
                    egui::Label::new(
                        egui::RichText::new("event chapter  // 源码排版 Aa 0123")
                            .font(resolved.type_roles.source.clone())
                            .color(resolved.syntax.keyword),
                    )
                    .wrap(),
                );
                ui.horizontal_wrapped(|ui| {
                    ui.add(
                        egui::Button::new("✓ 已选中项")
                            .selected(true)
                            .sense(egui::Sense::hover()),
                    );
                    theme::add_enabled_with_colors(ui, false, egui::Button::new("不可用操作"), c);
                });
                let mut sample = "焦点边界示例（仅展示）".to_owned();
                let response = ui.add(
                    egui::TextEdit::singleline(&mut sample)
                        .interactive(false)
                        .desired_width(ui.available_width())
                        .font(resolved.type_roles.ui.clone()),
                );
                ui.painter().rect_stroke(
                    response.rect,
                    resolved.shapes.control,
                    egui::Stroke::new(resolved.focus_width, c.focus),
                    egui::StrokeKind::Inside,
                );
                ui.add(
                    egui::Label::new(
                        egui::RichText::new("! 错误示例：必填字段仍为空").color(c.danger),
                    )
                    .wrap(),
                );
                ui.add(
                    egui::Label::new(
                        egui::RichText::new("△ 警告示例：来源需要重新检查").color(c.warning),
                    )
                    .wrap(),
                );
            });
        });
    ui.add(
        egui::Label::new(theme::muted(
            "以上控件仅展示状态，不改变焦点或作品。源码自动换行在「视图」菜单中设置。",
        ))
        .wrap(),
    );
}
