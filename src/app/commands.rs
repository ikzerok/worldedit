//! 固定作者命令与完整对象快速切换，不执行用户脚本。
mod edit;
use super::{Tab, WorldeditApp};
use worldline_core::TargetRef;
#[derive(Default)]
pub(super) struct CommandPalette {
    pub open: bool,
    pub commands_only: bool,
    pub query: String,
    pub focus: bool,
    selected: usize,
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
        self.command_palette.previous_focus = ctx.memory(|m| m.focused());
        self.command_palette.open = true;
        self.command_palette.commands_only = commands_only;
        self.command_palette.query.clear();
        self.command_palette.selected = 0;
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
                            format!(
                                "{} · {}:{}\n{}:{}",
                                object.display,
                                super::catalog::kind_label(&object.target.kind),
                                object.target.id,
                                object.file,
                                object.line
                            ),
                            Action::Object(object.target.clone()),
                        )
                    }),
                );
            }
            if !self.ime_composing && !palette.ime && !palette.ime_frame {
                if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown)) {
                    palette.selected = palette.selected.saturating_add(1);
                }
                if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp)) {
                    palette.selected = palette.selected.saturating_sub(1);
                }
            }
            palette.selected = palette.selected.min(entries.len().saturating_sub(1));
            egui::ScrollArea::vertical()
                .max_height(420.0)
                .show(ui, |ui| {
                    for (index, (label, candidate)) in entries.iter().enumerate() {
                        let row = ui.selectable_label(index == palette.selected, label);
                        if row.clicked() {
                            action = Some(candidate.clone());
                        }
                        if index == palette.selected
                            && ui.input(|i| {
                                i.key_pressed(egui::Key::ArrowDown)
                                    || i.key_pressed(egui::Key::ArrowUp)
                            })
                        {
                            row.scroll_to_me(None);
                        }
                    }
                });
            if entries.is_empty() {
                ui.label("没有匹配项；不会自动改选同名对象");
            }
            if entries.len() == 1000 {
                ui.label("最多显示1000项，请继续输入缩小范围");
            }
            if !palette.ime
                && !palette.ime_frame
                && !self.ime_composing
                && ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter))
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
            }
        }
    }
}
