use super::super::WorldeditApp;
use super::*;

impl WorldeditApp {
    pub(in crate::app) fn reader_publish_window(&mut self, ctx: &egui::Context) {
        #[cfg(not(target_arch = "wasm32"))]
        self.poll_native_reader_delivery();
        if self.reader_publish.busy() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        if !self.reader_publish.open {
            if self.reader_publish.busy() {
                self.cancel_reader_publish();
            }
            return;
        }
        let viewport = ctx.screen_rect().shrink(8.0);
        let style = ctx.style();
        let frame = egui::Frame::window(&style);
        let title = egui::RichText::new("发布给读者").heading();
        let title_height = ctx
            .fonts(|fonts| title.font_height(fonts, &style))
            .max(style.spacing.interact_size.y)
            + frame.inner_margin.sum().y;
        let content_height = (viewport.height()
            - frame.total_margin().sum().y
            - title_height
            - frame.stroke.width
            - 2.0)
            .max(120.0);
        let content_width = (viewport.width() - frame.total_margin().sum().x - 2.0).max(240.0);
        let mut open = true;
        let mut action = None;
        let state = &mut self.reader_publish;
        egui::Window::new(title)
            .frame(frame)
            .id(egui::Id::new("reader-publish-window"))
            .open(&mut open)
            .resizable(true)
            .constrain_to(viewport)
            .default_width(820.0_f32.min(content_width))
            .max_width(content_width)
            .default_height(700.0_f32.min(content_height))
            .max_height(content_height)
            .show(ctx, |ui| {
                ui.set_max_width(content_width);
                ui.label("离线选择不是权限认证；拿到阅读包的人可以查看包内全部内容。");
                ui.horizontal_wrapped(|ui| {
                    for (step, label) in [
                        (PublishStep::Select, "1 选择内容"),
                        (PublishStep::Resources, "2 核对资源"),
                        (PublishStep::Preview, "3 预览页面"),
                        (PublishStep::Generate, "4 确认生成"),
                    ] {
                        ui.add_enabled_ui(
                            !state.busy()
                                && (step == PublishStep::Select || state.reviewed.is_some()),
                            |ui| {
                                ui.selectable_value(&mut state.step, step, label);
                            },
                        );
                    }
                });
                if let Some(status) = &state.status {
                    ui.add(egui::Label::new(status).wrap());
                }
                if state.busy() {
                    ui.spinner();
                }
                ui.separator();
                let scroll_height = (ui.available_height() - 68.0).max(80.0);
                egui::ScrollArea::vertical()
                    .id_salt(("reader-step-content", state.step as u8))
                    .max_height(scroll_height)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.add_enabled_ui(!state.busy(), |ui| match state.step {
                            PublishStep::Select => {
                                let (changed, next_action) = state.selection_ui(ui);
                                if changed {
                                    state.invalidate_review();
                                }
                                action = next_action;
                            }
                            PublishStep::Resources => state.resource_review_ui(ui),
                            PublishStep::Preview => action = state.page_review_ui(ui),
                            PublishStep::Generate => action = state.generation_ui(ui),
                        });
                    });
                ui.separator();
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .button(if state.busy() {
                            "取消任务并关闭"
                        } else {
                            "取消发布"
                        })
                        .clicked()
                    {
                        action = Some(PublishAction::Cancel);
                    }
                    if !state.busy()
                        && state.step != PublishStep::Select
                        && ui.button("上一步").clicked()
                    {
                        state.step = match state.step {
                            PublishStep::Generate => PublishStep::Preview,
                            PublishStep::Preview => PublishStep::Resources,
                            _ => PublishStep::Select,
                        };
                    }
                    match state.step {
                        PublishStep::Select => {
                            if ui
                                .add_enabled(
                                    !state.busy() && state.has_selection(),
                                    crate::theme::primary("生成 / 更新预览"),
                                )
                                .clicked()
                            {
                                action = Some(PublishAction::Preview(state.selection()));
                            }
                            if !state.has_selection() {
                                ui.label("未选内容，不创建空包");
                            }
                        }
                        PublishStep::Resources | PublishStep::Preview => {
                            if ui
                                .add_enabled(!state.busy(), crate::theme::primary("继续"))
                                .clicked()
                            {
                                state.step = if state.step == PublishStep::Resources {
                                    PublishStep::Preview
                                } else {
                                    PublishStep::Generate
                                };
                            }
                        }
                        PublishStep::Generate => {
                            #[cfg(not(target_arch = "wasm32"))]
                            let label = "发布 ZIP";
                            #[cfg(target_arch = "wasm32")]
                            let label = "下载阅读包";
                            if ui
                                .add_enabled(
                                    !state.busy() && state.confirmed && state.reviewed.is_some(),
                                    crate::theme::primary(label),
                                )
                                .clicked()
                            {
                                action = Some(PublishAction::Publish);
                            }
                        }
                    }
                });
            });
        if !open {
            self.cancel_reader_publish();
            return;
        }
        match action {
            Some(PublishAction::Preview(selection)) => {
                self.start_reader_publish_preview(selection, ctx)
            }
            Some(PublishAction::Cancel) => {
                self.cancel_reader_publish();
                return;
            }
            Some(PublishAction::Publish) => self.publish_reader_package(ctx),
            #[cfg(not(target_arch = "wasm32"))]
            Some(PublishAction::Browse) => self.choose_reader_package_destination(),
            #[cfg(not(target_arch = "wasm32"))]
            Some(PublishAction::BrowserPreview(page)) => self.open_reader_browser_preview(page),
            Some(other) => self.reader_profile_action(other, ctx),
            None => {}
        }
        // X/取消先决定；按下尚未释放也不能让已排队完成先触发打开或下载。
        if self.reader_publish.open && !ctx.input(|input| input.pointer.any_down()) {
            self.poll_reader_profile_job();
            self.poll_reader_publish_job();
            #[cfg(not(target_arch = "wasm32"))]
            self.poll_reader_browser_preview();
        }
    }

    pub(in crate::app) fn cancel_reader_publish(&mut self) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        if self.reader_publish.close_drain.is_some() {
            self.reader_publish.open = false;
            return false;
        }
        #[cfg(not(target_arch = "wasm32"))]
        if self
            .reader_publish
            .delivery_job
            .as_ref()
            .is_some_and(|job| !job.cancel())
        {
            self.reader_publish.invalidate_review();
            self.reader_publish.open = false;
            self.message = Some("阅读包已进入原子提交阶段；等待真实结果，未宣称取消成功。".into());
            return false;
        }
        // 关闭只撤销本次任务与审核，不删除作者选择/配置输入；显式“新建选择”才重置。
        self.reader_publish.invalidate_review();
        self.reader_publish.open = false;
        true
    }
}
