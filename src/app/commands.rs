//! 固定作者命令与完整对象快速切换，不执行用户脚本。
mod edit;
mod layers;
use super::{Tab, WorldeditApp};
use worldline_core::TargetRef;
#[derive(Default)]
pub(super) struct CommandPalette {
    pub open: bool,
    pub commands_only: bool,
    pub query: String,
    pub focus: bool,
    selected: usize,
    scroll_selected: bool,
    previous_focus: Option<egui::Id>,
    pub ime: bool,
    pub ime_frame: bool,
    pub edit_focus: Option<egui::Id>,
    pub frame_focus: Option<egui::Id>,
    pub focus_stack: Vec<(&'static str, Option<egui::Id>)>,
}
#[derive(Clone)]
enum Action {
    Tab(Tab),
    Object(TargetRef),
    Save,
    Focus,
    Navigation,
    References,
    Back,
    Settings,
    Capabilities,
}
fn commands() -> Vec<(&'static str, Action)> {
    vec![
        ("写作 · 书稿工作台", Action::Tab(Tab::Manuscript)),
        ("阅读 · 正文概览", Action::Tab(Tab::Overview)),
        ("世界 · 时间线", Action::Tab(Tab::Timeline)),
        ("世界 · 人物", Action::Tab(Tab::Characters)),
        ("世界 · 资料与状态", Action::Tab(Tab::Catalog)),
        ("世界 · 世界观", Action::Tab(Tab::World)),
        ("结构 · 事件关系图", Action::Tab(Tab::Graph)),
        ("资料 · Wiki词条", Action::Tab(Tab::Wiki)),
        ("空间 · 地图画布", Action::Tab(Tab::Map)),
        ("关系 · 世界关联", Action::Tab(Tab::Network)),
        ("编辑 · 源文件", Action::Tab(Tab::Edit)),
        ("审阅 · 协作审阅", Action::Tab(Tab::Review)),
        ("语言 · 本地化", Action::Tab(Tab::Localization)),
        ("演练 · 试玩", Action::Tab(Tab::Play)),
        ("工程 · 显式启用语言与资料能力", Action::Capabilities),
        ("工程 · 工程模板", Action::Tab(Tab::Templates)),
        ("工程 · 检查点历史", Action::Tab(Tab::CheckpointHistory)),
        ("保存已应用的工程修改", Action::Save),
        ("切换正文专注模式", Action::Focus),
        ("显示 / 隐藏导航", Action::Navigation),
        ("显示 / 隐藏固定参考", Action::References),
        ("返回上一个作者位置", Action::Back),
        ("设置字体、行距与主题", Action::Settings),
    ]
}
impl WorldeditApp {
    pub(super) fn open_commands(&mut self, ctx: &egui::Context, commands_only: bool) {
        self.sync_edit_layers(ctx);
        if !self.command_palette.open {
            self.command_palette.previous_focus = ctx.memory(|m| m.focused());
        }
        self.command_palette.open = true;
        self.command_palette.commands_only = commands_only;
        self.command_palette.query.clear();
        self.command_palette.selected = 0;
        self.command_palette.scroll_selected = true;
        self.command_palette.focus = true;
    }
    pub(super) fn author_shortcuts(&mut self, ctx: &egui::Context) {
        self.command_palette.ime_frame = ctx.input(|input| {
            input
                .events
                .iter()
                .any(|event| matches!(event, egui::Event::Ime(_)))
        });
        ctx.input(|input| {
            for event in &input.events {
                if let egui::Event::Ime(event) = event {
                    match event {
                        egui::ImeEvent::Enabled | egui::ImeEvent::Preedit(_) => {
                            self.command_palette.ime = true
                        }
                        egui::ImeEvent::Disabled | egui::ImeEvent::Commit(_) => {
                            self.command_palette.ime = false
                        }
                    }
                }
            }
        });
        if self.ime_composing || self.command_palette.ime || self.command_palette.ime_frame {
            return;
        }
        self.edit_shortcuts(ctx);
        if ctx.input_mut(|i| {
            i.consume_key(
                egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                egui::Key::P,
            )
        }) {
            self.open_commands(ctx, true);
        } else if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::P)) {
            self.open_commands(ctx, false);
        }
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::ALT, egui::Key::ArrowLeft)) {
            self.author_back(ctx);
        }
    }
    pub(super) fn command_window(&mut self, ctx: &egui::Context) {
        if !self.command_palette.open {
            return;
        }
        let top = self.edit_layer_is_top("commands");
        let mut palette = std::mem::take(&mut self.command_palette);
        let mut open = true;
        let mut action = None;
        egui::Window::new(if palette.commands_only {
            "任务命令"
        } else {
            "快速切换对象"
        })
        .id(egui::Id::new("author-command-palette"))
        .open(&mut open)
        .collapsible(false)
        .default_width(640.0)
        .anchor(egui::Align2::CENTER_TOP, [0.0, 64.0])
        .show(ctx, |ui| {
            let response = ui.add(
                egui::TextEdit::singleline(&mut palette.query)
                    .desired_width(f32::INFINITY)
                    .hint_text("输入名称、ID、类型或来源；↑↓选择，Enter打开，Esc取消"),
            );
            if palette.focus {
                response.request_focus();
                palette.focus = false;
            }
            if response.changed() {
                palette.selected = 0;
                palette.scroll_selected = true;
            }
            let query = palette.query.trim().to_lowercase();
            let mut entries: Vec<(String, Action)> = Vec::new();
            if palette.commands_only {
                entries.extend(
                    commands()
                        .into_iter()
                        .filter(|(label, _)| label.to_lowercase().contains(&query))
                        .map(|(label, action)| (label.into(), action)),
                );
            } else if let Some(snapshot) = &self.snapshot {
                entries.extend(
                    super::object_picker::candidates(
                        &snapshot.result.analysis.catalog,
                        &query,
                        &[],
                    )
                    .into_iter()
                    .take(1000)
                    .map(|object| {
                        (
                            super::object_picker::candidate_caption(
                                object,
                                Some(&self.project.root),
                            ),
                            Action::Object(object.target.clone()),
                        )
                    }),
                );
            }
            let previous_selection = palette.selected;
            if top && !self.ime_composing && !palette.ime && !palette.ime_frame {
                let (down, up) = ui.input_mut(|i| {
                    (
                        i.count_and_consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
                        i.count_and_consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
                    )
                });
                palette.selected = palette.selected.saturating_add(down).saturating_sub(up);
            }
            palette.selected = palette.selected.min(entries.len().saturating_sub(1));
            let scroll_selected = palette.scroll_selected || palette.selected != previous_selection;
            let mut selected_visible = false;
            egui::ScrollArea::vertical()
                .id_salt(("author-command-results", palette.commands_only))
                .max_height(420.0)
                .show(ui, |ui| {
                    for (index, (label, candidate)) in entries.iter().enumerate() {
                        let selected = index == palette.selected;
                        let object = match candidate {
                            Action::Object(target) => self.snapshot.as_ref().and_then(|snapshot| {
                                snapshot.result.analysis.catalog.object(target)
                            }),
                            _ => None,
                        };
                        let row = if let Some(object) = object {
                            super::object_picker::candidate_row(
                                ui,
                                object,
                                Some(&self.project.root),
                                selected,
                            )
                        } else {
                            ui.selectable_label(selected, label)
                        };
                        if row.clicked() {
                            action = Some(candidate.clone());
                        }
                        if selected {
                            selected_visible = ui.clip_rect().contains_rect(row.rect);
                            ui.painter().rect_stroke(
                                row.rect.shrink(0.5),
                                4,
                                egui::Stroke::new(1.5_f32, crate::theme::ACCENT()),
                                egui::StrokeKind::Inside,
                            );
                            if scroll_selected {
                                row.scroll_to_me(None);
                            }
                        }
                    }
                });
            palette.scroll_selected = false;
            if !entries.is_empty() && !selected_visible {
                ui.label("选中项在视野外；用↑↓定位或点击可见项");
            }
            if entries.is_empty() {
                ui.label("没有匹配项；不会自动改选同名对象");
            }
            if entries.len() == 1000 {
                ui.label("最多显示1000项，请继续输入缩小范围");
            }
            if top
                && !palette.ime
                && !palette.ime_frame
                && !self.ime_composing
                && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter))
                && selected_visible
            {
                action = entries.get(palette.selected).map(|(_, a)| a.clone());
            }
        });
        palette.open = open && action.is_none();
        if !palette.open {
            if let Some(id) = palette.previous_focus {
                ctx.memory_mut(|m| m.request_focus(id));
            }
        }
        self.command_palette = palette;
        if let Some(action) = action {
            match action {
                Action::Tab(tab) => self.switch_tab(tab),
                Action::Object(target) => {
                    let object = self
                        .snapshot
                        .as_ref()
                        .and_then(|s| s.result.analysis.catalog.object(&target))
                        .cloned();
                    if let Some(object) = object {
                        self.navigate_object(&object);
                    } else {
                        self.message = Some("对象已不存在，请重新查找".into());
                    }
                }
                Action::Save => {
                    self.save();
                }
                Action::Focus => self.personal.settings.focus = !self.personal.settings.focus,
                Action::Navigation => {
                    self.personal.settings.navigation = !self.personal.settings.navigation
                }
                Action::References => {
                    self.personal.settings.references_visible =
                        !self.personal.settings.references_visible
                }
                Action::Back => self.author_back(ctx),
                Action::Settings => self.personal.preferences_open = true,
                Action::Capabilities => self.open_capabilities(),
            }
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod cancel_tests;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod navigation_tests;
