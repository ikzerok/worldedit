//! EDS-11 抛弃式技术原型。仅在显式 feature + 入口打开，永不写作品。
use eframe::egui;

const DEMO_SOURCE: &str = "event start\n  雾港的晨钟响了。\n  -> END\n";

#[derive(Clone, Copy, PartialEq, Eq)]
enum Task {
    World,
    Writing,
    Run,
    Review,
}

impl Task {
    fn label(self) -> &'static str {
        match self {
            Self::World => "J1 世界资料",
            Self::Writing => "J2 写作旁查",
            Self::Run => "J3 演练",
            Self::Review => "J4 审阅",
        }
    }
}

pub struct Prototype {
    task: Task,
    pending_task: Option<Task>,
    project: usize,
    pending_project: Option<usize>,
    drafts: [String; 2],
    selected: &'static str,
    pinned_b: bool,
    narrow: bool,
    focus: bool,
    focus_return: bool,
    drawer: bool,
    drawer_focus_return: Option<egui::Id>,
    inspector_tab: bool,
    read_only: bool,
    stale: bool,
    locked_layer: bool,
    preview_position: f32,
    placement_position: f32,
    undo_position: Option<f32>,
    preview_event: bool,
    runtime_event: bool,
    notice: String,
}

impl Default for Prototype {
    fn default() -> Self {
        Self {
            task: Task::World,
            pending_task: None,
            project: 0,
            pending_project: None,
            drafts: [String::new(), String::new()],
            selected: "A",
            pinned_b: false,
            narrow: false,
            focus: false,
            focus_return: false,
            drawer: false,
            drawer_focus_return: None,
            inspector_tab: false,
            read_only: false,
            stale: false,
            locked_layer: false,
            preview_position: 30.0,
            placement_position: 30.0,
            undo_position: None,
            preview_event: false,
            runtime_event: false,
            notice: "样例数据；Project 写入次数始终为 0。".into(),
        }
    }
}

impl Prototype {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        crate::fonts::install_cjk_fonts(&cc.egui_ctx);
        Self::default()
    }

    fn choose_task(&mut self, target: Task) {
        if self.task == target {
            return;
        }
        if self.drafts[self.project].is_empty() {
            self.task = target;
        } else {
            self.pending_task = Some(target);
        }
    }

    fn choose_project(&mut self, target: usize) {
        if self.project == target {
            return;
        }
        if self.drafts[self.project].is_empty() {
            self.project = target;
            self.selected = "A";
            self.pinned_b = false;
        } else {
            self.pending_project = Some(target);
        }
    }

    fn cancel_preview(&mut self) {
        self.preview_position = self.placement_position;
        self.notice = "预览取消：Project 写入 0。".into();
    }

    fn apply_preview(&mut self) {
        if self.read_only || self.stale || self.locked_layer {
            self.notice = "只读、旧基线或锁层：拒绝应用，候选保留。".into();
        } else {
            self.undo_position = Some(self.placement_position);
            self.placement_position = self.preview_position;
            self.notice = "原型局部状态应用一次；Project 写入仍为 0。".into();
        }
    }

    fn undo_preview(&mut self) {
        if let Some(before) = self.undo_position.take() {
            self.placement_position = before;
            self.preview_position = before;
        }
    }

    fn header(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("prototype_header").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.strong("EDS-11 · 抛弃式 egui 工作台");
                egui::ComboBox::from_id_salt("task")
                    .selected_text(self.task.label())
                    .show_ui(ui, |ui| {
                        for task in [Task::World, Task::Writing, Task::Run, Task::Review] {
                            if ui.selectable_label(self.task == task, task.label()).clicked() {
                                self.choose_task(task);
                            }
                        }
                    });
                egui::ComboBox::from_id_salt("sample_project")
                    .selected_text(format!("样例作品 {}", self.project + 1))
                    .show_ui(ui, |ui| {
                        for project in 0..2 {
                            if ui
                                .selectable_label(self.project == project, format!("样例作品 {}", project + 1))
                                .clicked()
                            {
                                self.choose_project(project);
                            }
                        }
                    });
                ui.toggle_value(&mut self.narrow, "窄窗区域");
                if ui.toggle_value(&mut self.focus, "专注").clicked() && !self.focus {
                    self.focus_return = true;
                }
                ui.toggle_value(&mut self.read_only, "只读");
                ui.toggle_value(&mut self.stale, "旧基线");
                if ui.button("资源抽屉").clicked() {
                    self.drawer = !self.drawer;
                }
            });
            ui.small("未实现的地图写入、书稿与审阅数据均为醒目标记的假数据；真实分析只调用 worldline-core。默认编辑器不加载此页面。");
        });
    }

    fn navigation(&mut self, ctx: &egui::Context) {
        if self.focus || self.narrow {
            return;
        }
        egui::SidePanel::left("prototype_navigation")
            .default_width(190.0)
            .show(ctx, |ui| {
                ui.heading("结构索引");
                ui.label("资料 B · entity:b");
                ui.label("地图入口 P · harbor/p");
                ui.label("事件 E · start");
                ui.label("提案 R · 样例");
                ui.separator();
                for task in [Task::World, Task::Writing, Task::Run, Task::Review] {
                    if ui.button(task.label()).clicked() {
                        self.choose_task(task);
                    }
                }
            });
    }

    fn inspector(&mut self, ctx: &egui::Context) {
        if self.focus || (self.narrow && !self.inspector_tab) {
            return;
        }
        egui::SidePanel::right("prototype_inspector")
            .default_width(250.0)
            .min_width(220.0)
            .show(ctx, |ui| {
                ui.heading(if self.pinned_b {
                    "已固定：B · entity:b"
                } else {
                    "跟随选择"
                });
                ui.label(format!("画布选择：{}", self.selected));
                ui.label(format!(
                    "编辑 owner：{}",
                    if self.pinned_b { "B" } else { self.selected }
                ));
                if ui.button("固定/解除 B").clicked() {
                    self.pinned_b = !self.pinned_b;
                }
                if self.narrow && ui.button("返回主区域").clicked() {
                    self.inspector_tab = false;
                }
                if self.stale {
                    ui.colored_label(egui::Color32::YELLOW, "目标来源已变化；旧输入不得覆盖。");
                }
            });
    }

    fn body(&mut self, ui: &mut egui::Ui) {
        match self.task {
            Task::World => {
                ui.heading("世界资料 / 地图入口（假数据）");
                ui.label("标记 P 与资料 B 有不同身份。拖动预览不写 Project。");
                ui.horizontal(|ui| {
                    if ui.button("选资料 A").clicked() {
                        self.selected = "A";
                    }
                    if ui.button("选入口 P").clicked() {
                        self.selected = "P";
                    }
                    ui.checkbox(&mut self.locked_layer, "锁层");
                });
                ui.add(
                    egui::Slider::new(&mut self.preview_position, 0.0..=100.0).text("入口位置预览"),
                );
                ui.label(format!(
                    "已应用位置 {:.0}；候选 {:.0}",
                    self.placement_position, self.preview_position
                ));
                ui.horizontal(|ui| {
                    if ui.button("取消预览 / Escape").clicked() {
                        self.cancel_preview();
                    }
                    if ui.button("应用一次（假数据）").clicked() {
                        self.apply_preview();
                    }
                    if ui.button("撤销一次（假数据）").clicked() {
                        self.undo_preview();
                    }
                });
            }
            Task::Writing => {
                ui.heading("正文 A 与旁查 B（假数据）");
                ui.label("原句：雾港的晨钟响了。选文建档尚由 CAP-01B 交付；本原型不创建资料。");
                ui.add(
                    egui::TextEdit::multiline(&mut self.drafts[self.project])
                        .id(egui::Id::new(("eds11_draft", self.project)))
                        .hint_text("在此输入未应用草稿，切任务/工程应明确保留")
                        .desired_width(f32::INFINITY)
                        .desired_rows(12),
                );
                if ui.button("临时旁查 B").clicked() {
                    self.drawer = true;
                }
                if ui.button("固定 B").clicked() {
                    self.pinned_b = true;
                }
            }
            Task::Run => {
                ui.heading("事件 E：定义 / 时间 / 本次运行");
                let core = worldline_core::compile_source("sample.wl", DEMO_SOURCE);
                ui.label(format!(
                    "worldline-core 真正编译样例：{} 条诊断",
                    core.diagnostics.len()
                ));
                ui.label("事件时间约束与控制流是不同投影；无日期事件仍可执行。");
                ui.horizontal(|ui| {
                    if ui.button("临时预览 E").clicked() {
                        self.preview_event = true;
                    }
                    if ui.button("显式运行 E（假会话）").clicked() {
                        self.runtime_event = true;
                    }
                });
                ui.label(format!(
                    "预览位置：{}；运行位置：{}",
                    self.preview_event, self.runtime_event
                ));
            }
            Task::Review => {
                ui.heading("提案 R 三方对照（假数据）");
                ui.columns(3, |columns| {
                    columns[0].label("base：雾港的晨钟响了。");
                    columns[1].label("current：雾港的晨钟再次响了。");
                    columns[2].label("proposal：雾港钟声消失了。");
                });
                ui.label("外部刷新时，解决草稿保留；不默认选赢家。");
                if ui.button("重新比较（不采纳）").clicked() {
                    self.notice = "三方比较仅为样例；Project 写入 0。".into();
                }
            }
        }
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        if self.drawer {
            egui::Window::new("资料 B · 临时侧览（假数据）")
                .open(&mut self.drawer)
                .show(ctx, |ui| {
                    ui.label("B / entity:b · 关闭只结束旁查，不提交、也不清正文草稿。");
                    ui.label("同名候选须展示 kind+ID；这里不连接创建命令。");
                });
        }
        if let Some(target) = self.pending_task {
            egui::Window::new("保留当前草稿？")
                .collapsible(false)
                .show(ctx, |ui| {
                    ui.label("当前作品 A 有未应用输入。可保留草稿并切任务，或取消切换。");
                    if ui.button("保留并切任务").clicked() {
                        self.task = target;
                        self.pending_task = None;
                    }
                    if ui.button("取消切换").clicked() {
                        self.pending_task = None;
                    }
                });
        }
        if let Some(target) = self.pending_project {
            egui::Window::new("按作品保留草稿？")
                .collapsible(false)
                .show(ctx, |ui| {
                    ui.label("草稿绑定当前样例作品；切换后不会套到另一作品的同名对象。");
                    if ui.button("保留并切作品").clicked() {
                        self.project = target;
                        self.pending_project = None;
                        self.selected = "A";
                        self.pinned_b = false;
                    }
                    if ui.button("取消切换").clicked() {
                        self.pending_project = None;
                    }
                });
        }
    }
}

impl eframe::App for Prototype {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.draw(ctx);
    }
}

impl Prototype {
    fn draw(&mut self, ctx: &egui::Context) {
        let drawer_was_open = self.drawer;
        let focus_before_frame = ctx.memory(|memory| memory.focused());
        let primary_released =
            ctx.input(|input| input.pointer.button_released(egui::PointerButton::Primary));
        let current_draft = egui::Id::new(("eds11_draft", self.project));
        if self.task == Task::Writing && focus_before_frame == Some(current_draft) {
            self.drawer_focus_return = Some(current_draft);
        } else if !drawer_was_open && !primary_released {
            self.drawer_focus_return = None;
        }
        self.header(ctx);
        self.navigation(ctx);
        self.inspector(ctx);
        egui::TopBottomPanel::bottom("prototype_status").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(format!(
                    "{}｜样例作品 {}｜选择 {}｜固定 B {}｜草稿长度 {}｜Project 写入 0",
                    self.task.label(),
                    self.project + 1,
                    self.selected,
                    self.pinned_b,
                    self.drafts[self.project].chars().count()
                ));
                if self.narrow && ui.button("检查器标签").clicked() {
                    self.inspector_tab = !self.inspector_tab;
                }
            });
            ui.small(&self.notice);
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("eds11_main_scroll")
                .show(ui, |ui| {
                    self.body(ui);
                });
        });
        self.dialogs(ctx);
        if ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
            if self.drawer {
                self.drawer = false;
            } else if self.task == Task::World {
                self.cancel_preview();
            }
        }
        if !drawer_was_open && self.drawer && self.task == Task::Writing {
            self.drawer_focus_return = self
                .drawer_focus_return
                .or_else(|| focus_before_frame.filter(|id| *id == current_draft));
        } else if drawer_was_open && !self.drawer {
            if let Some(id) = self.drawer_focus_return.take() {
                if self.task == Task::Writing && id == current_draft {
                    ctx.memory_mut(|memory| memory.request_focus(id));
                }
            }
        }
        if self.focus_return && self.task == Task::Writing {
            ctx.memory_mut(|memory| {
                memory.request_focus(egui::Id::new(("eds11_draft", self.project)))
            });
            self.focus_return = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Prototype, Task};
    use egui::{pos2, vec2, Event, ImeEvent, PointerButton, RawInput, Rect};

    fn frame(
        ctx: &egui::Context,
        prototype: &mut Prototype,
        events: Vec<Event>,
        size: [f32; 2],
    ) -> egui::FullOutput {
        ctx.run(
            RawInput {
                screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(size[0], size[1]))),
                events,
                ..Default::default()
            },
            |ctx| prototype.draw(ctx),
        )
    }

    fn frame_with_native_scale(
        ctx: &egui::Context,
        prototype: &mut Prototype,
        size: [f32; 2],
        native_pixels_per_point: f32,
    ) -> egui::FullOutput {
        let mut input = RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(size[0], size[1]))),
            ..Default::default()
        };
        input
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .expect("root viewport")
            .native_pixels_per_point = Some(native_pixels_per_point);
        ctx.run(input, |ctx| prototype.draw(ctx))
    }

    fn text_position(shape: &egui::Shape, label: &str) -> Option<egui::Pos2> {
        match shape {
            egui::Shape::Text(text) if text.galley.job.text == label => {
                Some(text.pos + text.galley.rect.center().to_vec2())
            }
            egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| text_position(shape, label)),
            _ => None,
        }
    }

    fn contains_text(shape: &egui::Shape, label: &str) -> bool {
        match shape {
            egui::Shape::Text(text) => text.galley.job.text.contains(label),
            egui::Shape::Vec(shapes) => shapes.iter().any(|shape| contains_text(shape, label)),
            _ => false,
        }
    }

    fn click_at(ctx: &egui::Context, prototype: &mut Prototype, size: [f32; 2], point: egui::Pos2) {
        for pressed in [true, false] {
            let _ = frame(
                ctx,
                prototype,
                vec![
                    Event::PointerMoved(point),
                    Event::PointerButton {
                        pos: point,
                        button: PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                size,
            );
        }
    }

    fn click(ctx: &egui::Context, prototype: &mut Prototype, size: [f32; 2], label: &str) {
        for _ in 0..4 {
            let _ = frame(ctx, prototype, Vec::new(), size);
        }
        let output = frame(ctx, prototype, Vec::new(), size);
        let point = output
            .shapes
            .iter()
            .find_map(|shape| text_position(&shape.shape, label))
            .unwrap_or_else(|| panic!("原型控件未显示：{label}"));
        click_at(ctx, prototype, size, point);
    }

    fn click_last(ctx: &egui::Context, prototype: &mut Prototype, size: [f32; 2], label: &str) {
        for _ in 0..4 {
            let _ = frame(ctx, prototype, Vec::new(), size);
        }
        let output = frame(ctx, prototype, Vec::new(), size);
        let point = output
            .shapes
            .iter()
            .filter_map(|shape| text_position(&shape.shape, label))
            .last()
            .unwrap_or_else(|| panic!("原型控件未显示：{label}"));
        click_at(ctx, prototype, size, point);
    }

    fn drag_slider_left_of_label(
        ctx: &egui::Context,
        prototype: &mut Prototype,
        size: [f32; 2],
        label: &str,
        start_offset: f32,
        end_offset: f32,
    ) {
        for _ in 0..4 {
            let _ = frame(ctx, prototype, Vec::new(), size);
        }
        let output = frame(ctx, prototype, Vec::new(), size);
        let label_pos = output
            .shapes
            .iter()
            .find_map(|shape| text_position(&shape.shape, label))
            .unwrap_or_else(|| panic!("滑块标签未显示：{label}"));
        let start = label_pos + vec2(start_offset, 0.0);
        let end = label_pos + vec2(end_offset, 0.0);
        let _ = frame(
            ctx,
            prototype,
            vec![
                Event::PointerMoved(start),
                Event::PointerButton {
                    pos: start,
                    button: PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            size,
        );
        let _ = frame(ctx, prototype, vec![Event::PointerMoved(end)], size);
        let _ = frame(
            ctx,
            prototype,
            vec![
                Event::PointerMoved(end),
                Event::PointerButton {
                    pos: end,
                    button: PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            size,
        );
    }

    fn text(ctx: &egui::Context, prototype: &mut Prototype, size: [f32; 2], value: &str) {
        let _ = frame(ctx, prototype, vec![Event::Text(value.into())], size);
    }

    fn ime(ctx: &egui::Context, prototype: &mut Prototype, size: [f32; 2], event: ImeEvent) {
        let _ = frame(ctx, prototype, vec![Event::Ime(event)], size);
    }

    fn escape(ctx: &egui::Context, prototype: &mut Prototype, size: [f32; 2]) {
        let _ = frame(
            ctx,
            prototype,
            vec![Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            size,
        );
    }

    #[test]
    fn temporary_reader_returns_keyboard_focus_to_ime_draft() {
        let ctx = egui::Context::default();
        let mut prototype = Prototype::default();
        let size = [1024.0, 640.0];
        let draft_id = egui::Id::new(("eds11_draft", 0));

        click(&ctx, &mut prototype, size, "J2 写作旁查");
        assert!(prototype.task == Task::Writing);

        click(
            &ctx,
            &mut prototype,
            size,
            "在此输入未应用草稿，切任务/工程应明确保留",
        );
        assert_eq!(ctx.memory(|memory| memory.focused()), Some(draft_id));
        ime(&ctx, &mut prototype, size, ImeEvent::Enabled);
        ime(&ctx, &mut prototype, size, ImeEvent::Preedit("雾港".into()));
        assert_eq!(prototype.drafts[0], "雾港");
        assert!(prototype.task == Task::Writing && !prototype.drawer);
        ime(&ctx, &mut prototype, size, ImeEvent::Commit("雾港".into()));
        assert_eq!(prototype.drafts[0], "雾港");

        click(&ctx, &mut prototype, size, "临时旁查 B");
        assert!(prototype.drawer);
        escape(&ctx, &mut prototype, size);
        assert!(!prototype.drawer);
        assert_eq!(prototype.drafts[0], "雾港");
        assert_eq!(ctx.memory(|memory| memory.focused()), Some(draft_id));
        text(&ctx, &mut prototype, size, "继续");
        assert_eq!(prototype.drafts[0], "雾港继续");
    }

    #[test]
    fn world_placement_cancel_apply_undo_and_guards_use_egui_events() {
        let ctx = egui::Context::default();
        let mut prototype = Prototype::default();
        let size = [1024.0, 640.0];

        click(&ctx, &mut prototype, size, "选入口 P");
        assert_eq!(prototype.selected, "P");
        drag_slider_left_of_label(&ctx, &mut prototype, size, "入口位置预览", -100.0, -35.0);
        let candidate = prototype.preview_position;
        assert_ne!(candidate, 30.0, "egui 拖动应改变候选位置");
        assert_eq!(prototype.placement_position, 30.0);

        click(&ctx, &mut prototype, size, "取消预览 / Escape");
        assert_eq!(prototype.preview_position, 30.0);
        assert_eq!(prototype.placement_position, 30.0);

        drag_slider_left_of_label(&ctx, &mut prototype, size, "入口位置预览", -100.0, -35.0);
        let candidate = prototype.preview_position;
        click(&ctx, &mut prototype, size, "应用一次（假数据）");
        assert_eq!(prototype.placement_position, candidate);
        assert_eq!(prototype.undo_position, Some(30.0));
        click(&ctx, &mut prototype, size, "撤销一次（假数据）");
        assert_eq!(prototype.placement_position, 30.0);
        assert_eq!(prototype.preview_position, 30.0);
        assert!(prototype.undo_position.is_none());

        drag_slider_left_of_label(&ctx, &mut prototype, size, "入口位置预览", -100.0, -35.0);
        let candidate = prototype.preview_position;
        click(&ctx, &mut prototype, size, "只读");
        click(&ctx, &mut prototype, size, "应用一次（假数据）");
        assert_eq!(prototype.placement_position, 30.0);
        assert_eq!(prototype.preview_position, candidate);
        assert!(prototype.undo_position.is_none());

        click(&ctx, &mut prototype, size, "只读");
        click(&ctx, &mut prototype, size, "旧基线");
        click(&ctx, &mut prototype, size, "应用一次（假数据）");
        assert_eq!(prototype.placement_position, 30.0);
        assert_eq!(prototype.preview_position, candidate);
        assert!(prototype.undo_position.is_none());

        click(&ctx, &mut prototype, size, "旧基线");
        click(&ctx, &mut prototype, size, "锁层");
        click(&ctx, &mut prototype, size, "应用一次（假数据）");
        assert_eq!(prototype.placement_position, 30.0);
        assert_eq!(prototype.preview_position, candidate);
        assert!(prototype.undo_position.is_none());
        let output = frame(&ctx, &mut prototype, Vec::new(), size);
        assert!(output
            .shapes
            .iter()
            .any(|shape| contains_text(&shape.shape, "Project 写入 0")));
    }

    #[test]
    fn writing_project_and_task_switches_preserve_scoped_draft_via_events() {
        let ctx = egui::Context::default();
        let mut prototype = Prototype::default();
        let size = [1024.0, 640.0];

        click(&ctx, &mut prototype, size, "J2 写作旁查");
        click(
            &ctx,
            &mut prototype,
            size,
            "在此输入未应用草稿，切任务/工程应明确保留",
        );
        text(&ctx, &mut prototype, size, "雾港草稿");
        assert_eq!(prototype.drafts[0], "雾港草稿");

        click(&ctx, &mut prototype, size, "样例作品 1");
        click_last(&ctx, &mut prototype, size, "样例作品 2");
        assert_eq!(prototype.project, 0);
        assert_eq!(prototype.pending_project, Some(1));
        click(&ctx, &mut prototype, size, "取消切换");
        assert_eq!(prototype.project, 0);
        assert_eq!(prototype.drafts[0], "雾港草稿");

        click(&ctx, &mut prototype, size, "样例作品 1");
        click_last(&ctx, &mut prototype, size, "样例作品 2");
        click(&ctx, &mut prototype, size, "保留并切作品");
        assert_eq!(prototype.project, 1);
        assert_eq!(prototype.drafts[0], "雾港草稿");
        assert!(prototype.drafts[1].is_empty());

        click(&ctx, &mut prototype, size, "样例作品 2");
        click_last(&ctx, &mut prototype, size, "样例作品 1");
        assert_eq!(prototype.project, 0);
        click(&ctx, &mut prototype, size, "J3 演练");
        assert!(prototype.pending_task == Some(Task::Run));
        click(&ctx, &mut prototype, size, "取消切换");
        assert!(prototype.task == Task::Writing);
        assert_eq!(prototype.drafts[0], "雾港草稿");

        click(&ctx, &mut prototype, size, "J3 演练");
        click(&ctx, &mut prototype, size, "保留并切任务");
        assert!(prototype.task == Task::Run);
        assert_eq!(prototype.drafts[0], "雾港草稿");
    }

    #[test]
    fn run_and_review_paths_keep_preview_runtime_and_comparison_separate() {
        let ctx = egui::Context::default();
        let mut prototype = Prototype::default();
        let size = [1024.0, 640.0];

        click(&ctx, &mut prototype, size, "J3 演练");
        assert!(prototype.task == Task::Run);
        let output = frame(&ctx, &mut prototype, Vec::new(), size);
        assert!(output.shapes.iter().any(|shape| {
            contains_text(&shape.shape, "worldline-core 真正编译样例：0 条诊断")
        }));
        click(&ctx, &mut prototype, size, "临时预览 E");
        assert!(prototype.preview_event);
        assert!(!prototype.runtime_event);
        click(&ctx, &mut prototype, size, "显式运行 E（假会话）");
        assert!(prototype.preview_event && prototype.runtime_event);

        click(&ctx, &mut prototype, size, "J4 审阅");
        assert!(prototype.task == Task::Review);
        let before = (
            prototype.preview_event,
            prototype.runtime_event,
            prototype.drafts.clone(),
        );
        click(&ctx, &mut prototype, size, "重新比较（不采纳）");
        assert_eq!(
            before,
            (
                prototype.preview_event,
                prototype.runtime_event,
                prototype.drafts.clone()
            )
        );
        assert!(prototype.notice.contains("仅为样例"));
        let output = frame(&ctx, &mut prototype, Vec::new(), size);
        for label in [
            "base：雾港的晨钟响了。",
            "current：雾港的晨钟再次响了。",
            "proposal：雾港钟声消失了。",
            "Project 写入 0",
        ] {
            assert!(output
                .shapes
                .iter()
                .any(|shape| contains_text(&shape.shape, label)));
        }
    }

    #[test]
    fn logical_viewport_survives_1024_and_700_points_at_scaled_dpi_and_zoom() {
        let ctx = egui::Context::default();
        let mut prototype = Prototype::default();
        ctx.set_zoom_factor(1.25);
        let output = frame_with_native_scale(&ctx, &mut prototype, [1024.0, 640.0], 1.5);
        assert!((output.pixels_per_point - 1.875).abs() < 0.01);
        assert!(output.shapes.iter().any(|shape| {
            contains_text(&shape.shape, "世界资料 / 地图入口（假数据）")
        }));

        click(&ctx, &mut prototype, [1024.0, 640.0], "窄窗区域");
        let narrow = frame_with_native_scale(&ctx, &mut prototype, [700.0, 640.0], 1.5);
        assert!(narrow.shapes.iter().any(|shape| {
            contains_text(&shape.shape, "世界资料 / 地图入口（假数据）")
        }));
        assert!(narrow
            .shapes
            .iter()
            .any(|shape| contains_text(&shape.shape, "检查器标签")));
        click(&ctx, &mut prototype, [700.0, 640.0], "检查器标签");
        let with_inspector = frame_with_native_scale(&ctx, &mut prototype, [700.0, 640.0], 1.5);
        assert!(with_inspector
            .shapes
            .iter()
            .any(|shape| contains_text(&shape.shape, "跟随选择")));
    }

    #[test]
    fn placement_preview_cancel_apply_and_undo_are_local() {
        let mut prototype = Prototype::default();
        prototype.preview_position = 60.0;
        assert_eq!(prototype.placement_position, 30.0);
        prototype.cancel_preview();
        assert_eq!(prototype.preview_position, 30.0);
        prototype.preview_position = 60.0;
        prototype.apply_preview();
        assert_eq!(prototype.placement_position, 60.0);
        prototype.undo_preview();
        assert_eq!(prototype.placement_position, 30.0);
        assert_eq!(prototype.preview_position, 30.0);
    }

    #[test]
    fn read_only_stale_and_locked_reject_apply_without_losing_candidate() {
        for blocked in 0..3 {
            let mut prototype = Prototype::default();
            prototype.preview_position = 60.0;
            prototype.read_only = blocked == 0;
            prototype.stale = blocked == 1;
            prototype.locked_layer = blocked == 2;
            prototype.apply_preview();
            assert_eq!(prototype.placement_position, 30.0);
            assert_eq!(prototype.preview_position, 60.0);
            assert!(prototype.undo_position.is_none());
        }
    }

    #[test]
    fn switching_project_keeps_draft_bound_to_original_project() {
        let mut prototype = Prototype::default();
        prototype.task = Task::Writing;
        prototype.drafts[0] = "雾港草稿".into();
        prototype.choose_project(1);
        assert_eq!(prototype.project, 0);
        assert_eq!(prototype.pending_project, Some(1));
        prototype.project = prototype.pending_project.take().unwrap();
        assert!(prototype.drafts[1].is_empty());
        assert_eq!(prototype.drafts[0], "雾港草稿");
    }
}
