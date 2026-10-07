//! 固定作者命令与完整对象快速切换，不执行用户脚本。
mod edit;
mod layers;
mod window;
use super::{Tab, WorldeditApp};
use worldline_core::catalog::CatalogObject;
#[cfg(test)]
use worldline_core::TargetRef;
#[derive(Default)]
pub(super) struct CommandPalette {
    pub open: bool,
    pub commands_only: bool,
    pub query: String,
    pub focus: bool,
    selected: usize,
    objects: super::object_picker::CandidatePage,
    notice: Option<String>,
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
    Object(CatalogObject, bool),
    Save,
    Focus,
    Navigation,
    References,
    Back,
    Settings,
    Capabilities,
    TemporalIssues,
    Problems,
    NextProblem,
    PreviousProblem,
    MoveEntitySource,
    SourceOutline,
    SourceJump,
}
fn commands() -> Vec<(&'static str, Action)> {
    vec![
        ("写作 · 书稿工作台", Action::Tab(Tab::Manuscript)),
        ("阅读 · 正文概览", Action::Tab(Tab::Overview)),
        ("世界 · 时间线", Action::Tab(Tab::Timeline)),
        ("时间 · 问题与先后比较", Action::TemporalIssues),
        ("工程 · 问题工作台", Action::Problems),
        ("工程 · 下一问题", Action::NextProblem),
        ("工程 · 上一问题", Action::PreviousProblem),
        ("世界 · 人物", Action::Tab(Tab::Characters)),
        ("世界 · 资料与状态", Action::Tab(Tab::Catalog)),
        ("资料 · 世界资料导入", Action::Tab(Tab::CatalogImport)),
        ("世界 · 世界观", Action::Tab(Tab::World)),
        ("结构 · 事件关系图", Action::Tab(Tab::Graph)),
        ("资料 · Wiki词条", Action::Tab(Tab::Wiki)),
        ("空间 · 地图画布", Action::Tab(Tab::Map)),
        ("关系 · 世界关联", Action::Tab(Tab::Network)),
        ("编辑 · 源文件", Action::Tab(Tab::Edit)),
        ("源码 · 本文件结构", Action::SourceOutline),
        ("源码 · 跳转到行列", Action::SourceJump),
        ("实体 · 移到其他源码…", Action::MoveEntitySource),
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
        self.command_palette.objects = Default::default();
        self.command_palette.notice = None;
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
        self.source_outline_shortcut(ctx);
        self.source_jump_shortcut(ctx);
        self.problems_shortcuts(ctx);
        self.character_region_shortcut(ctx);
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
            if self.edit_layer_is_top("navigation-drawer") {
                self.close_navigation_drawer(ctx);
            } else if self.edit_layer_is_top("compact-references") {
                self.close_compact_reference(ctx);
            } else {
                self.author_back(ctx);
            }
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod cancel_tests;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod navigation_tests;
