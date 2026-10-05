//! 显式预览、范围核对和副本交付；窗口绘制不自动启动验证。
use super::ReportRoute;
use crate::app::WorldeditApp;
use crate::theme;
use worldline_runtime::{PlaythroughReport, RouteStatus};

pub(super) fn status_text(report: &PlaythroughReport) -> &'static str {
    match report.status {
        RouteStatus::Replayed if report.complete && report.ended => "完整结束并验证通过",
        RouteStatus::Replayed => "已验证部分区段，非完整结局",
        RouteStatus::Diverged => "发生分歧，仅此前匹配的正文已验证",
        RouteStatus::StepBudgetExceeded => "步数预算耗尽，仅展示已验证前缀",
        RouteStatus::TimeBudgetExceeded => "时间预算耗尽，仅展示已验证前缀",
        RouteStatus::Cancelled => "验证已取消，仅展示已验证前缀",
        RouteStatus::IncompleteTrace => "记录缺少观察，无法证明完整路线",
        RouteStatus::StoryFailed => "故事运行失败，仅展示已验证前缀",
        RouteStatus::OutputBudgetExceeded => "输出预算耗尽，报告不完整",
    }
}

impl WorldeditApp {
    pub(in crate::app) fn playthrough_report_window(&mut self, ctx: &egui::Context) {
        if !self.playthrough_report.open {
            return;
        }
        // 仅显式打开时交接一次；后续Tab、指针或输入法导航不被旧请求拉回。
        let focus_on_open = std::mem::take(&mut self.playthrough_report.focus_on_open)
            && !self.ime_composing
            && !self.command_palette.ime
            && !self.command_palette.ime_frame;
        let inputs = self.unapplied_play_inputs();
        if self.playthrough_report.confirmed_inputs != inputs {
            self.playthrough_report.scope_confirmed = false;
            self.playthrough_report.confirmed_inputs = inputs.clone();
        }
        let mut open = true;
        let mut generate = false;
        let mut copy = false;
        let mut save = false;
        let mut cancel = false;
        let mut close = false;
        #[cfg(not(target_arch = "wasm32"))]
        let mut browse = false;
        let state = &mut self.playthrough_report;
        egui::Window::new("试玩路径报告 · 作者审阅副本")
            .id(egui::Id::new("playthrough-report"))
            .open(&mut open)
            .default_width(740.0)
            .max_width(ctx.screen_rect().width().min(900.0) - 24.0)
            .default_height(620.0)
            .resizable(true)
            .vscroll(true)
            .show(ctx, |ui| {
                ui.colored_label(theme::WARNING(), "作者私密内容：实际正文、选择、说话者及源码相对文件名可能含私人信息。复制或保存前请核对全部预览及接收范围。");
                ui.label("这是已验证的单条试玩区段；未探索分支不代表不可达，也不授予读者发布权限。");
                ui.separator();
                let before = (state.route, state.max_steps, state.time_budget_ms);
                ui.add_enabled_ui(state.job.is_none(), |ui| {
                    let route_name = match state.route {
                        ReportRoute::Live => "当前试玩路径（生成时捕获）".to_owned(),
                        ReportRoute::Saved(index) => self.replay_debugger.saved_paths.get(index)
                            .map(|path| format!("已录制：{}", path.name))
                            .unwrap_or_else(|| "已录制路径已失效".into()),
                    };
                    let route = egui::ComboBox::from_id_salt("report-route")
                        .selected_text(route_name)
                        .width(330.0)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut state.route, ReportRoute::Live, "当前试玩路径（生成时捕获）");
                            for (index, path) in self.replay_debugger.saved_paths.iter().enumerate() {
                                ui.selectable_value(&mut state.route, ReportRoute::Saved(index), &path.name);
                            }
                        });
                    if focus_on_open {
                        route.response.request_focus();
                        route.response.scroll_to_me_animation(Some(egui::Align::Min), egui::style::ScrollAnimation::none());
                    }
                    ui.horizontal_wrapped(|ui| {
                        ui.label("步数上限");
                        ui.add(egui::DragValue::new(&mut state.max_steps).range(0..=100_000));
                        ui.label("时限 ms");
                        ui.add(egui::DragValue::new(&mut state.time_budget_ms).range(0..=30_000));
                    });
                    ui.label(format!("验证当前已应用工程稿 #{}；已应用但未保存的修改也会参与。", self.version));
                    if !inputs.is_empty() {
                        egui::CollapsingHeader::new(format!("未纳入的创作草稿 · {} 项", inputs.len()))
                            .default_open(true)
                            .show(ui, |ui| {
                                for input in &inputs {
                                    ui.label(format!("{} · {}", input.kind, input.source));
                                }
                            });
                        ui.checkbox(&mut state.scope_confirmed, "我确认仅验证已应用稿，以上草稿仍保留且不进入报告");
                    }
                    generate = ui.add_enabled(inputs.is_empty() || state.scope_confirmed,
                        theme::primary("生成并预览已验证报告")).clicked();
                });
                if before != (state.route, state.max_steps, state.time_budget_ms) {
                    state.privacy_confirmed = false;
                }
                if (before.1, before.2) != (state.max_steps, state.time_budget_ms) {
                    state.reviewed = None;
                    state.notice = Some("验证预算已变化，请重新生成报告".into());
                }
                #[cfg(target_arch = "wasm32")]
                ui.label("浏览器每帧最多 512 步 / 4 ms，总计最多 2,000 ms；使用导入快照，不持续观察宿主磁盘。");
                if state.job.is_some() {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("只读验证中…");
                        cancel = ui.button("取消验证").clicked();
                    });
                }
                if let Some(reviewed) = &state.reviewed {
                    ui.separator();
                    ui.label(egui::RichText::new(&reviewed.name).strong());
                    ui.colored_label(if reviewed.report.complete && reviewed.report.ended && reviewed.report.status == RouteStatus::Replayed { theme::TEXT() } else { theme::WARNING() }, status_text(&reviewed.report));
                    ui.label(format!("已应用稿 #{} · 起点 {} · seed {} · 已验证 {} 次选择",
                        reviewed.scope.version, reviewed.report.origin.kind, reviewed.report.origin.seed,
                        reviewed.report.verified_choices));
                    if reviewed.report.origin.kind == "checkpoint" {
                        ui.colored_label(theme::WARNING(), "仅验证检查点之后的区段；先前覆盖为继承，未重建入口路线。");
                    }
                    let current = reviewed.scope.matches_project(&self.project, self.version);
                    let selected = state.route == reviewed.route;
                    if !current {
                        ui.colored_label(theme::WARNING(), "结果已过期：已应用版本或完整内容基线变化，请重新验证后导出。");
                    }
                    if !selected {
                        ui.colored_label(theme::WARNING(), "以下预览属于之前的路径选择，请重新生成。");
                    }
                    if !reviewed.scope.excluded_inputs.is_empty() {
                        ui.label(format!("生成时排除 {} 项未应用草稿；输入仍保留。", reviewed.scope.excluded_inputs.len()));
                    }
                    egui::ScrollArea::vertical()
                        .id_salt("playthrough-report-preview")
                        .max_height(280.0)
                        .show(ui, |ui| {
                            let mut markdown = reviewed.report.markdown.as_str();
                            ui.add(egui::TextEdit::multiline(&mut markdown).font(egui::TextStyle::Monospace)
                                .desired_width(f32::INFINITY).desired_rows(12));
                        });
                    ui.checkbox(&mut state.privacy_confirmed, "我已核对预览、验证范围与私密内容，确认复制或保存此作者报告");
                    #[cfg(not(target_arch = "wasm32"))]
                    ui.horizontal_wrapped(|ui| {
                        ui.label("新 Markdown 文件");
                        ui.add(egui::TextEdit::singleline(&mut state.destination)
                            .hint_text("工作区外的绝对完整路径，以 .md 结尾").desired_width(370.0));
                        browse = ui.button("系统选择器（可选）").clicked();
                    });
                    ui.add_enabled_ui(current && selected && state.privacy_confirmed && state.job.is_none(), |ui| {
                        ui.horizontal_wrapped(|ui| {
                            copy = ui.button("复制 Markdown").clicked();
                            #[cfg(not(target_arch = "wasm32"))]
                            { save = ui.button("保存新 Markdown 文件").clicked(); }
                            #[cfg(target_arch = "wasm32")]
                            { save = ui.button("下载 Markdown").clicked(); }
                        });
                    });
                }
                if let Some(notice) = &state.notice {
                    ui.colored_label(theme::WARNING(), notice);
                }
                ui.separator();
                close = ui.button("关闭报告").clicked();
            });
        if !open || close {
            self.close_playthrough_report();
            return;
        }
        if cancel {
            if let Some(job) = &self.playthrough_report.job {
                job.cancellation.cancel();
            }
        }
        if generate {
            self.begin_playthrough_report(ctx);
        }
        #[cfg(not(target_arch = "wasm32"))]
        if browse {
            self.choose_playthrough_report_destination();
        }
        if copy {
            self.copy_playthrough_report(ctx);
        }
        if save {
            self.save_playthrough_report();
        }
    }
}
