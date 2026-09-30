//! EDS-11 隔离技术原型。显式入口；真实core仅修改虚构内存工程，永不保存作品。
use eframe::egui;

mod core_bridge;
use core_bridge::MemoryDemo;

#[cfg(not(target_arch = "wasm32"))]
pub fn requested_native(mut arguments: impl Iterator<Item = String>) -> bool {
    arguments.any(|argument| argument == "--eds11-prototype")
}

#[cfg(any(target_arch = "wasm32", test))]
pub fn requested_web_query(query: &str) -> bool {
    query
        .trim_start_matches('?')
        .split('&')
        .any(|part| part == "eds11=1")
}

#[cfg(not(target_arch = "wasm32"))]
pub fn native_options() -> eframe::NativeOptions {
    eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_app_id("worldedit-eds11-prototype")
            .with_inner_size([1280.0, 760.0])
            .with_min_inner_size([640.0, 480.0]),
        persist_window: false,
        ..Default::default()
    }
}

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
    demos: [MemoryDemo; 2],
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
            notice: "两个虚构内存 Project；从不打开或保存用户作品。".into(),
            demos: [MemoryDemo::new(0), MemoryDemo::new(1)],
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
            self.activate_project(target);
        } else {
            self.pending_project = Some(target);
        }
    }

    fn activate_project(&mut self, target: usize) {
        self.project = target;
        self.selected = "A";
        self.pinned_b = false;
        self.placement_position = self.demos[target].position();
        self.preview_position = self.placement_position;
        self.undo_position = self.demos[target].undo_from;
        self.preview_event = false;
        self.runtime_event = self.demos[target].runtime.is_some();
        self.notice = format!(
            "样例作品 {}：内存缓冲/运行记录分别保留；作品磁盘写入 0。",
            target + 1
        );
    }

    fn cancel_preview(&mut self) {
        self.preview_position = self.placement_position;
        self.notice = "预览取消：当前 Project 缓冲未修改；作品磁盘写入 0。".into();
    }

    fn apply_preview(&mut self) {
        match self.demos[self.project].apply_position(
            self.preview_position,
            self.read_only,
            self.stale,
            self.locked_layer,
        ) {
            Ok(()) => {
                self.undo_position = self.demos[self.project].undo_from;
                self.placement_position = self.demos[self.project].position();
                self.notice =
                    "真实 core 应用一次：harbor/p 内存缓冲已改变；作品磁盘写入 0。".into();
            }
            Err(error) => self.notice = format!("core 拒绝，候选保留：{error}"),
        }
    }

    fn undo_preview(&mut self) {
        match self.demos[self.project].undo_position(self.read_only, self.stale, self.locked_layer)
        {
            Ok(()) => {
                self.placement_position = self.demos[self.project].position();
                self.preview_position = self.placement_position;
                self.undo_position = self.demos[self.project].undo_from;
                self.notice = "真实 core 撤销：恢复地图原字节；作品磁盘写入 0。".into();
            }
            Err(error) => self.notice = format!("core 拒绝撤销：{error}"),
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
            ui.small("J1地图命令/撤销、J3运行、J4比较使用真实core；仅虚构内存工程。J2布局仍是假数据；模式开关构造拒绝样例。默认编辑器不加载此页。");
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
                ui.heading("世界资料 / 地图入口（内存 core 样例）");
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
                    if ui.button("应用一次（真实 core）").clicked() {
                        self.apply_preview();
                    }
                    if ui.button("撤销一次（真实 core）").clicked() {
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
                let core = self.demos[self.project].project.compile();
                ui.label(format!(
                    "worldline-core 真正编译样例：{} 条诊断",
                    core.diagnostics.len()
                ));
                ui.label("事件时间约束与控制流是不同投影；无日期事件仍可执行。");
                ui.horizontal(|ui| {
                    if ui.button("临时预览 E").clicked() {
                        self.preview_event = true;
                    }
                    if ui.button("显式运行 E（真实 runtime）").clicked() {
                        match self.demos[self.project].run_event() {
                            Ok(()) => {
                                self.runtime_event = true;
                                self.notice =
                                    "真实 Story 执行完成；Project 缓冲和磁盘未修改。".into();
                            }
                            Err(error) => self.notice = error,
                        }
                    }
                });
                ui.label(format!(
                    "预览位置：{}；运行位置：{}",
                    self.preview_event, self.runtime_event
                ));
                if let Some(run) = &self.demos[self.project].runtime {
                    ui.label(format!(
                        "runtime ended={}；start visits={}",
                        run.ended, run.start_visits
                    ));
                    ui.label(&run.output);
                }
            }
            Task::Review => {
                ui.heading("提案 R 三方对照（真实 core 预览）");
                let sides = self.demos[self.project].review_sides();
                ui.columns(3, |columns| {
                    columns[0].label(format!("base：{}", sides[0]));
                    columns[1].label(format!("current：{}", sides[1]));
                    columns[2].label(format!("proposal：{}", sides[2]));
                });
                ui.label("只读比较不默认选赢家；解决草稿与采纳仍未在本原型接通。");
                if ui.button("重新比较（不采纳）").clicked() {
                    match self.demos[self.project].compare() {
                        Ok(()) => {
                            self.notice =
                                "真实 core 三方比较完成；当前 Project 缓冲与磁盘未修改。".into()
                        }
                        Err(error) => self.notice = error,
                    }
                }
                if let Some(preview) = &self.demos[self.project].review {
                    ui.label(format!(
                        "core：{} 文件，{} 冲突，可应用={}（本页无采纳动作）",
                        preview.files.len(),
                        preview.conflicts.len(),
                        preview.can_apply()
                    ));
                    for conflict in &preview.conflicts {
                        ui.label(format!("{}：{}", conflict.path, conflict.message));
                    }
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
                        self.activate_project(target);
                        self.pending_project = None;
                    }
                    if ui.button("取消切换").clicked() {
                        self.pending_project = None;
                    }
                });
        }
    }
}

impl eframe::App for Prototype {
    fn persist_egui_memory(&self) -> bool {
        false
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.draw(ctx);
    }
}

impl Prototype {
    fn draw(&mut self, ctx: &egui::Context) {
        let drawer_was_open = self.drawer;
        let focus_before_frame = ctx.memory(|memory| memory.focused());
        let (primary_pressed, primary_released, primary_down) = ctx.input(|input| {
            (
                input.pointer.button_pressed(egui::PointerButton::Primary),
                input.pointer.button_released(egui::PointerButton::Primary),
                input.pointer.button_down(egui::PointerButton::Primary),
            )
        });
        let current_draft = egui::Id::new(("eds11_draft", self.project));
        if self.task == Task::Writing && focus_before_frame == Some(current_draft) {
            self.drawer_focus_return = Some(current_draft);
        } else if !drawer_was_open && (primary_pressed || (!primary_released && !primary_down)) {
            self.drawer_focus_return = None;
        }
        self.header(ctx);
        self.navigation(ctx);
        self.inspector(ctx);
        egui::TopBottomPanel::bottom("prototype_status").show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(format!(
                    "{}｜样例作品 {}｜选择 {}｜固定 B {}｜草稿长度 {}｜core已提交操作 {}｜作品磁盘写入 0",
                    self.task.label(),
                    self.project + 1,
                    self.selected,
                    self.pinned_b,
                    self.drafts[self.project].chars().count(),
                    self.demos[self.project].applied_commands
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
mod core_ui_tests;
#[cfg(all(test, not(target_arch = "wasm32")))]
mod profile;
#[cfg(test)]
mod tests;
