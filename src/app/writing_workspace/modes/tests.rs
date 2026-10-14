//! 真实工具栏与原文档同帧消费；不以直接改模式代替控件操作。
use super::*;
use egui::{Event, ImeEvent, PointerButton};
mod reload_tests;
mod tier_tests;
mod toolbar_tests;

struct Workbench {
    project: Project,
    buffer: WritingBuffer,
    target: TargetRef,
    view: ViewState,
    ctx: egui::Context,
    size: egui::Vec2,
    enabled: bool,
    metadata: Option<String>,
    appearance: Option<theme::AppearancePreferences>,
    column: Option<f32>,
    production: bool,
    apply: bool,
    allow_apply: bool,
    force_multipass: bool,
    error: Option<String>,
}
impl Workbench {
    fn new(body: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "dialogue-modes-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut project = Project::new(&root);
        project
            .set_text(
                &project.entry.clone(),
                format!("character a as \"人物\"\nevent start\n{body}  -> END\n"),
            )
            .unwrap();
        project.create_authoring_document(&root.join(".world/project.json"), br#"{"schema_version":1,"language_version":"1.11","required_features":[],"maps":{},"graph_views":{}}"#.to_vec()).unwrap();
        std::fs::create_dir_all(&root).unwrap();
        project.save().unwrap();
        let target = TargetRef::new("event", "start");
        let buffer = project.open_writing_buffer(&target).unwrap();
        let ctx = egui::Context::default();
        ctx.style_mut(|style| style.animation_time = 0.0);
        let mut app = Self {
            project,
            buffer,
            target,
            view: Default::default(),
            ctx,
            size: egui::vec2(1700.0, 2200.0),
            enabled: true,
            metadata: None,
            appearance: None,
            column: None,
            production: false,
            apply: false,
            allow_apply: false,
            force_multipass: false,
            error: None,
        };
        app.frame(vec![]);
        app
    }
    fn frame(&mut self, events: Vec<Event>) -> egui::FullOutput {
        self.ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, self.size)),
                events,
                ..Default::default()
            },
            |ctx| {
                let _theme = self
                    .appearance
                    .as_ref()
                    .map(|p| theme::configure_appearance(ctx, p));
                let _input = self.view.begin_input(ctx);
                egui::CentralPanel::default().show(ctx, |ui| {
                    // CentralPanel 的 min_rect 已占整屏；必须创建真实受限子 Ui。
                    let available = ui.available_rect_before_wrap();
                    let width = self
                        .column
                        .unwrap_or(available.width())
                        .min(available.width());
                    let rect = egui::Rect::from_min_size(
                        available.min,
                        egui::vec2(width, available.height()),
                    );
                    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
                        ui.set_clip_rect(ui.clip_rect().intersect(rect));
                        theme::add_enabled_ui(ui, self.enabled, |ui| {
                            if let Some(metadata) = &mut self.metadata {
                                let response = ui.add(
                                    egui::TextEdit::singleline(metadata)
                                        .id(egui::Id::new("mode-metadata")),
                                );
                                register_input(&response);
                            }
                            let typography = Typography {
                                compact: false,
                                size: self.appearance.as_ref().map_or(17.0, |p| p.body_size),
                                spacing: 1.6,
                                width: 950.0,
                                source_size: 13.0,
                            };
                            assert!(
                                (ui.available_width() - width).abs() < 0.1,
                                "正文实际列宽 {} 必须等于请求宽度 {width}",
                                ui.available_width()
                            );
                            let mut action = draw_controls(
                                ui,
                                &self.project,
                                &mut self.buffer,
                                &self.target,
                                "测试稿",
                                &mut self.view,
                                typography,
                            );
                            draw_document(
                                ui,
                                &self.project,
                                &mut self.buffer,
                                &self.target,
                                "测试稿",
                                &mut self.view,
                                typography,
                                &mut action,
                            );
                            self.error = action.error;
                            self.production = action.production;
                            self.apply = action.apply;
                            assert!(!action.apply || self.allow_apply);
                            assert!(action.dialogue_plan.is_none());
                        });
                    });
                });
                if self.force_multipass && ctx.current_pass_index() == 0 {
                    ctx.request_discard("核对本正文工具新开轮的多 pass 策略");
                }
            },
        )
    }
    fn point(&mut self, label: &str) -> egui::Pos2 {
        self.frame(vec![]);
        let output = self.frame(vec![]);
        output
            .shapes
            .iter()
            .find_map(|shape| point(&shape.shape, label))
            .unwrap_or_else(|| panic!("missing {label}"))
    }
    fn click(&mut self, label: &str) {
        let pos = self.point(label);
        for pressed in [true, false] {
            self.frame(pointer(pos, pressed));
        }
        self.frame(vec![]);
    }
    fn batch(&mut self, label: &str, event: Event) {
        let pos = self.point(label);
        let mut events = vec![event];
        events.extend(pointer(pos, true));
        events.extend(pointer(pos, false));
        self.frame(events);
    }
    fn retained(&self) -> String {
        serde_json::to_string(&self.view.dialogue_runtime_drafts(&self.project.root)).unwrap()
    }
    fn focus_body(&mut self) {
        self.view.focus_existing_editor();
        self.frame(vec![]);
        self.frame(vec![]);
        assert!(self.ctx.memory(|memory| memory.focused()).is_some());
    }
}
impl Drop for Workbench {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.project.root).unwrap();
    }
}
fn point(shape: &egui::Shape, label: &str) -> Option<egui::Pos2> {
    match shape {
        egui::Shape::Text(text) if text.galley.text() == label => {
            Some(text.pos + text.galley.rect.center().to_vec2())
        }
        egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| point(shape, label)),
        _ => None,
    }
}
fn pointer(pos: egui::Pos2, pressed: bool) -> Vec<Event> {
    vec![
        Event::PointerMoved(pos),
        Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        },
    ]
}
fn mixed() -> &'static str {
    "  原有旁白正文。\n  say a \"原有正式对白\" direction \"私密备注\"\n"
}

#[test]
fn dialogue_modes_are_explicit_for_plain_mixed_and_say_only_documents() {
    for (body, writable) in [
        ("  原有旁白正文。\n", true),
        (mixed(), true),
        ("  say a \"原有正式对白\"\n", false),
    ] {
        let mut app = Workbench::new(body);
        assert!(!app.view.dialogue.enabled);
        let output = app.frame(vec![]);
        assert!(!output
            .shapes
            .iter()
            .any(|shape| point(&shape.shape, "编辑此句").is_some()));
        if writable {
            app.focus_body();
            app.frame(vec![Event::Text("直接正文".into())]);
            assert_eq!(app.buffer.source().matches("直接正文").count(), 1);
            assert!(!app.view.has_dialogue_input());
        }
        app.click("逐句对白");
        assert!(app.view.dialogue.enabled);
        let output = app.frame(vec![]);
        assert!(output
            .shapes
            .iter()
            .any(|shape| point(&shape.shape, "编辑此句").is_some()));
        app.click("逐句对白");
        assert!(!app.view.dialogue.enabled);
    }
}

#[test]
fn dialogue_modes_defer_all_toolbar_changes_until_old_text_and_paste_are_consumed() {
    for (start, label, expected) in [
        ("写作", "源码", Mode::Source),
        ("源码", "结构", Mode::Structure),
        ("结构", "写作", Mode::Prose),
        ("写作", "逐句对白", Mode::Prose),
    ] {
        for paste in [false, true] {
            let mut app = Workbench::new(mixed());
            app.click(start);
            app.focus_body();
            let event = if paste {
                Event::Paste("原视图输入".into())
            } else {
                Event::Text("原视图输入".into())
            };
            app.batch(label, event);
            assert_eq!(
                app.buffer.source().matches("原视图输入").count(),
                1,
                "{start}->{label}"
            );
            assert_eq!(app.view.mode, expected);
            assert_eq!(app.view.dialogue.enabled, label == "逐句对白");
        }
    }
    for label in ["源码", "结构", "逐句对白"] {
        for paste in [false, true] {
            let mut app = Workbench::new(mixed());
            app.click("逐句对白");
            app.click("编辑此句");
            let identity = app.buffer.identity();
            let event = if paste {
                Event::Paste("原字段输入".into())
            } else {
                Event::Text("原字段输入".into())
            };
            app.batch(label, event);
            assert_eq!(
                app.retained().matches("原字段输入").count(),
                1,
                "F->{label}"
            );
            assert_eq!(app.buffer.identity(), identity);
            assert_eq!(app.view.dialogue.enabled, label != "逐句对白");
            if label != "逐句对白" {
                assert_ne!(app.view.mode, Mode::Prose);
            }
        }
    }
}

#[test]
fn dialogue_modes_preserve_protected_input_and_return_to_the_last_real_field() {
    let mut app = Workbench::new("  say a \"原有正式对白\" direction \"私密备注\"\n");
    app.click("逐句对白");
    app.click("编辑此句");
    app.click("私密备注");
    app.frame(vec![Event::Text("新增备注".into())]);
    let owner = app.ctx.memory(|memory| memory.focused());
    let retained = app.retained();
    let identity = app.buffer.identity();
    app.click("源码");
    assert_eq!(app.retained(), retained);
    app.click("写作");
    assert_eq!(app.ctx.memory(|memory| memory.focused()), owner);
    assert_eq!(app.retained(), retained);
    app.click("逐句对白");
    assert!(!app.view.dialogue.enabled);
    assert_eq!(app.retained(), retained);
    app.click("逐句对白");
    assert_eq!(app.ctx.memory(|memory| memory.focused()), owner);
    app.frame(vec![Event::Text("继续备注".into())]);
    assert!(app.retained().contains("继续备注"));
    assert_eq!(app.buffer.identity(), identity);
}

#[test]
fn dialogue_modes_restore_only_the_proven_original_prose_cursor() {
    let mut app = Workbench::new(mixed());
    app.focus_body();
    let id = app.ctx.memory(|memory| memory.focused()).unwrap();
    let mut state = egui::TextEdit::load_state(&app.ctx, id).unwrap();
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::one(
            egui::text::CCursor::new(3),
        )));
    state.store(&app.ctx, id);
    app.frame(vec![]);
    let saved = app
        .view
        .cursor_for_buffer(&app.buffer, &app.target)
        .unwrap();
    app.click("逐句对白");
    app.click("编辑此句");
    app.frame(vec![Event::Text("仍在字段".into())]);
    let retained = app.retained();
    app.click("逐句对白");
    assert_eq!(app.ctx.memory(|memory| memory.focused()), Some(id));
    let restored = app
        .view
        .cursor_for_buffer(&app.buffer, &app.target)
        .unwrap();
    assert_eq!(restored.cursor, saved.cursor);
    assert_eq!(restored.block_offset, saved.block_offset);
    assert_eq!(app.retained(), retained);
    app.click("逐句对白");
    app.buffer
        .replace_source(format!("{}\n// 新原文\n", app.buffer.source()));
    app.view.invalidate_projection();
    app.click("逐句对白");
    assert!(
        app.view.pending_cursor.is_none(),
        "改变完整原文后不能恢复旧范围"
    );
}

#[test]
fn dialogue_modes_commit_frame_and_late_or_wrong_target_intents_never_switch() {
    let mut app = Workbench::new(mixed());
    app.click("逐句对白");
    app.click("编辑此句");
    app.batch("源码", Event::Ime(ImeEvent::Commit("组合保留".into())));
    assert_eq!(app.view.mode, Mode::Prose);
    assert!(app.view.dialogue.enabled);
    assert!(app.retained().contains("组合保留"));
    app.frame(vec![]);
    let _ = app.ctx.run(Default::default(), |ctx| {
        app.view
            .request_mode(ctx, &app.buffer, &app.target, Mode::Source);
    });
    app.frame(vec![]);
    assert_eq!(app.view.mode, Mode::Prose);
    let _ = app.ctx.run(Default::default(), |ctx| {
        app.view
            .request_mode(ctx, &app.buffer, &app.target, Mode::Source);
        app.view
            .finish_mode_request(ctx, &app.buffer, &TargetRef::new("event", "elsewhere"));
        assert!(app.view.mode_request.is_none());
        assert_eq!(app.view.mode, Mode::Prose);
    });
}

#[test]
fn dialogue_modes_narrow_menu_closes_before_the_new_editor_receives_text() {
    for typed in [false, true] {
        let mut app = Workbench::new(mixed());
        if typed {
            app.click("逐句对白");
            app.click("编辑此句");
        }
        app.size = egui::vec2(550.0, 900.0);
        app.frame(vec![]);
        app.click("正文工具");
        app.click("源码");
        assert_eq!(app.view.mode, Mode::Source);
        assert!(!egui::Popup::is_any_open(&app.ctx));
        app.frame(vec![Event::Text("窄窗返回输入".into())]);
        assert_eq!(app.buffer.source().matches("窄窗返回输入").count(), 1);
    }
}

#[test]
fn dialogue_modes_disabled_document_never_creates_or_steals_a_form_focus() {
    let mut app = Workbench::new(mixed());
    app.click("逐句对白");
    let foreign = egui::Id::new("readonly-foreign-owner");
    app.ctx.memory_mut(|memory| memory.request_focus(foreign));
    app.view.focus_existing_editor();
    app.enabled = false;
    let identity = app.buffer.identity();
    app.frame(vec![Event::Text("不能进只读正文".into())]);
    assert!(!app.view.dialogue.has_form(&app.buffer, &app.target));
    assert!(app
        .ctx
        .memory(|memory| memory.focused())
        .is_none_or(|id| id == foreign));
    assert_eq!(app.buffer.identity(), identity);
    assert!(!app.view.has_dialogue_input());
}

#[test]
fn dialogue_modes_checkbox_from_source_or_structure_always_enters_the_chosen_body_mode() {
    for mode in ["源码", "结构"] {
        for previously_enabled in [false, true] {
            let mut app = Workbench::new(mixed());
            if previously_enabled {
                app.click("逐句对白");
            }
            app.click(mode);
            app.batch("逐句对白", Event::Paste("// 模式原输入\n".into()));
            assert_eq!(app.view.mode, Mode::Prose);
            assert_eq!(app.view.dialogue.enabled, !previously_enabled);
            assert_eq!(app.buffer.source().matches("模式原输入").count(), 1);
            assert!(
                !app.view.dialogue.has_form(&app.buffer, &app.target),
                "初次勾选只显示完整语句，不伪造一次编辑点击"
            );
        }
    }
}

#[test]
fn dialogue_modes_keep_original_owner_when_metadata_registers_before_the_toolbar() {
    for mode in ["写作", "源码", "逐句对白"] {
        let mut app = Workbench::new(mixed());
        app.metadata = Some("编排书名".into());
        app.click(mode);
        if mode == "逐句对白" {
            app.click("编辑此句");
        }
        app.focus_body();
        let before = app.buffer.source().to_owned();
        app.batch("结构", Event::Paste("原owner独占".into()));
        if mode == "逐句对白" {
            assert_eq!(app.retained().matches("原owner独占").count(), 1);
            assert_eq!(app.buffer.source(), before);
        } else {
            assert_eq!(app.buffer.source().matches("原owner独占").count(), 1);
        }
        assert_eq!(app.metadata.as_deref(), Some("编排书名"));
        assert_eq!(app.view.mode, Mode::Structure);
    }
    let mut app = Workbench::new(mixed());
    app.metadata = Some("编排书名".into());
    app.click("编排书名");
    let before = app.buffer.source().to_owned();
    app.frame(vec![Event::Text("元数据独占".into())]);
    app.click("源码");
    assert_eq!(
        app.metadata.as_ref().unwrap().matches("元数据独占").count(),
        1
    );
    assert_eq!(app.buffer.source(), before);
    assert!(!app.view.has_dialogue_input());
}
