//! 完整 update 的真实输入/控件焦点轨迹；不注入表单状态或请求焦点。
mod body_focus;
mod ime_lifecycle;
mod long_fields;
mod missing_required;
mod native_shape;
mod object_entries;
mod visibility;
use super::*;
use egui::{Key, Modifiers, WidgetType};

struct FullApp {
    ctx: egui::Context,
    app: WorldeditApp,
    physical: egui::Vec2,
    trace: Vec<String>,
    last_trace_focus: Option<egui::Id>,
}
impl FullApp {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let ctx = egui::Context::default();
        let mut app = WorldeditApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()), None);
        let root = std::env::temp_dir().join(format!(
            "schema-full-keyboard-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).expect("独占夹具目录");
        app.project = worldline_core::project::Project::new(&root);
        app.active_file = app.project.entry.clone();
        app.project
            .documents
            .retain(|path, _| path == &app.active_file);
        app.project.create_authoring_document(&root.join(".world/project.json"), br#"{"schema_version":1,"language_version":"1.13","required_features":[],"maps":{},"graph_views":{}}"#.to_vec()).unwrap();
        app.project.set_text(&app.active_file.clone(), "schema person_data for character\n  field memo_id memo text required\ncharacter lead as \"人物验收\"\n  property memo = \"旧资料\"\nevent start\n  正文。\n  -> END\n".into()).unwrap();
        app.personal.settings.appearance.reduce_motion = true;
        app.personal.pending_restore = false;
        app.tab = crate::app::Tab::Edit;
        app.recompile();
        assert!(!app.schema_authoring.open);
        let mut harness = Self {
            ctx,
            app,
            physical: vec2(1188.0, 848.0),
            trace: vec![],
            last_trace_focus: None,
        };
        harness.settle();
        harness
    }
    fn frame(&mut self, events: Vec<Event>) -> egui::FullOutput {
        // 原生后端按物理像素 / 实际 zoom 送入逻辑 viewport，不能只放大字体。
        let screen = Rect::from_min_size(pos2(0.0, 0.0), self.physical / self.ctx.zoom_factor());
        let modifiers = events
            .iter()
            .find_map(|event| match event {
                Event::Key { modifiers, .. } => Some(*modifiers),
                _ => None,
            })
            .unwrap_or_default();
        let mut raw = RawInput {
            screen_rect: Some(screen),
            events,
            modifiers,
            ..Default::default()
        };
        let viewport = raw.viewports.get_mut(&egui::ViewportId::ROOT).unwrap();
        viewport.native_pixels_per_point = Some(1.0);
        viewport.inner_rect = Some(screen);
        viewport.focused = Some(true);
        let output = self.ctx.run(raw, |ctx| {
            eframe::App::update(&mut self.app, ctx, &mut eframe::Frame::_new_kittest());
        });
        let focused = self.ctx.memory(|memory| memory.focused());
        let response = focused.and_then(|id| self.ctx.read_response(id));
        let gained: Vec<_> = output
            .platform_output
            .events
            .iter()
            .filter_map(|event| match event {
                egui::output::OutputEvent::FocusGained(info) => {
                    Some(format!("{:?}:{:?}", info.typ, info.label))
                }
                _ => None,
            })
            .collect();
        if !gained.is_empty() || self.trace.is_empty() || focused != self.last_trace_focus {
            let resize_edge = response.as_ref().and_then(|response| {
                (0..8).find(|index| {
                    response.layer_id
                        == egui::LayerId::new(
                            egui::Order::Foreground,
                            egui::Id::new(("resize-edge", *index as usize)),
                        )
                })
            });
            self.trace.push(format!(
                "focus={focused:?} response={:?} resize_edge={resize_edge:?} info={gained:?} ppp={} screen={:?} layers={:?}",
                response.as_ref().map(|r| (r.layer_id, r.rect, r.enabled())),
                output.pixels_per_point,
                screen,
                self.app.command_palette.focus_stack
            ));
            self.last_trace_focus = focused;
        }
        output
    }
    fn settle(&mut self) -> egui::FullOutput {
        for _ in 0..6 {
            self.frame(vec![]);
        }
        self.frame(vec![])
    }
    fn press(&mut self, key: Key, modifiers: Modifiers) -> Vec<egui::WidgetInfo> {
        let mut gained = Vec::new();
        for pressed in [true, false] {
            let output = self.frame(vec![Event::Key {
                key,
                physical_key: Some(key),
                pressed,
                repeat: false,
                modifiers,
            }]);
            for event in output.platform_output.events {
                if let egui::output::OutputEvent::FocusGained(info) = event {
                    gained.push(info);
                }
            }
        }
        gained
    }
    fn focused(&self) -> egui::Response {
        self.ctx
            .memory(|memory| memory.focused())
            .and_then(|id| self.ctx.read_response(id))
            .unwrap_or_else(|| panic!("没有真实获焦控件：{}", self.trace.join("\n")))
    }
    fn visible_text(output: &egui::FullOutput, label: &str) -> Option<Rect> {
        output.shapes.iter().find_map(|shape| {
            find(&shape.shape, label).filter(|rect| shape.clip_rect.contains_rect(*rect))
        })
    }
    fn response_visible(
        &self,
        response: &egui::Response,
        output: &egui::FullOutput,
        label: &str,
    ) -> bool {
        response.enabled()
            && self.ctx.screen_rect().contains_rect(response.rect)
            && self.ctx.layer_id_at(response.rect.center()) == Some(response.layer_id)
            && if label == "Close window" {
                // 标题栏的关闭图标没有文字；真实交互区域须完整落在 clip 内。
                response.interact_rect.contains_rect(response.rect)
            } else {
                visibility::response_text(output, label, response.rect)
            }
    }
    fn tab_to(&mut self, label: &str, typ: WidgetType, reverse: bool) -> egui::Id {
        for _ in 0..180 {
            let info = self.press(
                Key::Tab,
                if reverse {
                    Modifiers::SHIFT
                } else {
                    Modifiers::NONE
                },
            );
            let output = self.settle();
            if let Some(response) = self
                .ctx
                .memory(|memory| memory.focused())
                .and_then(|id| self.ctx.read_response(id))
            {
                if self.app.schema_authoring.open && !egui::Popup::is_any_open(&self.ctx) {
                    self.assert_sheet_focus();
                }
                if info.iter().any(|info| {
                    info.typ == typ
                        && (info.label.as_deref() == Some(label)
                            || typ == WidgetType::ComboBox
                            || typ == WidgetType::TextEdit)
                }) && self.response_visible(&response, &output, label)
                {
                    return response.id;
                }
            }
        }
        panic!(
            "Tab(reverse={reverse})未到达可见真实{typ:?} {label}：{}",
            self.trace.join("\n")
        );
    }
    fn open_from_real_character_entry(&mut self) -> egui::Id {
        self.press(Key::P, Modifiers::COMMAND);
        self.frame(vec![Event::Text("lead".into())]);
        self.settle();
        assert!(self.app.command_palette.open, "{}", self.trace.join("\n"));
        self.press(Key::Enter, Modifiers::NONE);
        self.settle();
        assert_eq!(
            self.app
                .character_editor
                .as_ref()
                .unwrap()
                .original
                .as_deref(),
            Some("lead")
        );
        // 搜索只选中人物；档案仍须从真实工作台动作展开。
        self.tab_to("编辑档案 / 旧式关系", WidgetType::Button, false);
        self.press(Key::Enter, Modifiers::NONE);
        self.settle();
        assert!(self.app.character_focus.inspector_open);
        let entry = self.tab_to("约束与属性…", WidgetType::Button, false);
        self.press(Key::Enter, Modifiers::NONE);
        self.settle();
        assert!(
            self.app.schema_authoring.open,
            "真实入口Enter没有打开工作表：{}",
            self.trace.join("\n")
        );
        assert_eq!(self.app.schema_authoring.return_focus, Some(entry));
        entry
    }
    fn assert_sheet_focus(&self) {
        let response = self.focused();
        assert_eq!(
            response.layer_id,
            egui::LayerId::new(egui::Order::Middle, egui::Id::new("schema-authoring-form")),
            "工作表打开后焦点仍在底层：{}",
            self.trace.join("\n")
        );
    }
    fn assert_frame_visible(&mut self) {
        let output = self.settle();
        let window = self
            .ctx
            .memory(|memory| memory.area_rect(egui::Id::new("schema-authoring-form")))
            .unwrap();
        assert!(
            self.ctx.screen_rect().contains_rect(window),
            "整个工作表越出原生viewport: {window:?}; {:?}",
            self.ctx.screen_rect()
        );
        for label in [
            "资料 · 约束与属性",
            "预览字段修改",
            "应用到工程",
            "取消（保留输入）",
        ] {
            assert!(
                Self::visible_text(&output, label).is_some(),
                "工作表标题/固定操作被裁: {label}; {window:?}; {}",
                self.trace.join("\n")
            );
        }
    }
    fn bind_and_fill(&mut self) -> TargetRef {
        self.assert_sheet_focus();
        let baseline = self.app.project.content_baseline();
        let combo = self.tab_to("未绑定", WidgetType::ComboBox, false);
        self.assert_sheet_focus();
        self.press(Key::Enter, Modifiers::NONE);
        assert!(
            egui::ComboBox::is_open(&self.ctx, combo),
            "真实Enter没有展开绑定combo"
        );
        // egui 0.32 的 selectable_value 使用 Button::selectable，输出 Button 身份。
        self.tab_to("person_data", WidgetType::Button, false);
        self.press(Key::Enter, Modifiers::NONE);
        self.settle();
        assert!(!egui::ComboBox::is_open(&self.ctx, combo));
        let target = TargetRef::new("character", "lead");
        assert_eq!(
            self.app.schema_authoring.sessions[&target]
                .selected_schema
                .as_deref(),
            Some("person_data")
        );
        // 先反向离开再顺向回来，不能把父容器的焦点当作按钮焦点。
        self.tab_to("预览所选绑定", WidgetType::Button, true);
        self.press(Key::Enter, Modifiers::NONE);
        self.settle();
        assert_eq!(self.app.project.content_baseline(), baseline);
        assert!(self.app.schema_authoring.sessions[&target]
            .preview
            .is_some());
        self.tab_to("旧资料", WidgetType::TextEdit, false);
        let field = self.focused();
        assert_eq!(field.id, fields::input_id(&target, "memo_id"));
        self.press(Key::A, Modifiers::COMMAND);
        self.frame(vec![Event::Text("键盘填写的新资料".into())]);
        self.settle();
        assert_eq!(
            self.app.schema_authoring.sessions[&target].values["memo_id"],
            Some(PropertyValue::Str("键盘填写的新资料".into()))
        );
        target
    }
    fn complete_keyboard_flow(&mut self) {
        self.assert_sheet_focus();
        self.tab_to("Close window", WidgetType::Button, false);
        let baseline = self.app.project.content_baseline();
        let target = self.bind_and_fill();
        self.tab_to("预览字段修改", WidgetType::Button, true);
        self.press(Key::Enter, Modifiers::NONE);
        self.settle();
        assert_eq!(self.app.project.content_baseline(), baseline);
        assert!(
            self.app.schema_authoring.sessions[&target]
                .preview
                .as_ref()
                .unwrap()
                .runtime_changed
        );
        self.tab_to("确认应用这次运行指纹变更", WidgetType::Checkbox, false);
        self.press(Key::Space, Modifiers::NONE);
        self.tab_to("应用到工程", WidgetType::Button, true);
        self.press(Key::Enter, Modifiers::NONE);
        self.settle();
        assert_ne!(self.app.project.content_baseline(), baseline);
        assert_eq!(self.app.history.len(), 1);
        assert_eq!(
            self.app.current_schema_authoring(&target).unwrap().fields[0].value,
            Some(PropertyValue::Str("键盘填写的新资料".into()))
        );
    }
}

#[test]
fn schema_full_app_keyboard_flow_initial_focus_after_real_entry() {
    let mut h = FullApp::new();
    h.open_from_real_character_entry();
    // 独立于几何断言，保留开窗时是否把键盘留在底层的证据。
    h.assert_sheet_focus();
}

#[test]
fn schema_full_app_keyboard_flow_at_native_100_percent() {
    let mut h = FullApp::new();
    let entry = h.open_from_real_character_entry();
    h.assert_frame_visible();
    h.complete_keyboard_flow();
    h.press(Key::Escape, Modifiers::NONE);
    h.settle();
    assert!(!h.app.schema_authoring.open);
    assert_eq!(h.ctx.memory(|memory| memory.focused()), Some(entry));
}

#[test]
fn schema_full_app_keyboard_flow_after_real_zoom_to_200_percent() {
    let mut h = FullApp::new();
    h.open_from_real_character_entry();
    for _ in 0..10 {
        h.press(Key::Equals, Modifiers::COMMAND);
    }
    h.settle();
    assert_eq!(h.app.personal.appearance().ui_scale, 2.0);
    assert_eq!(h.ctx.pixels_per_point(), 2.0);
    assert_eq!(h.ctx.screen_rect().size(), vec2(594.0, 424.0));
    h.assert_frame_visible();
    h.complete_keyboard_flow();
}

#[test]
fn schema_full_app_keyboard_flow_preserves_dirty_source_return_and_ime_focus() {
    let mut h = FullApp::new();
    let entry = h.open_from_real_character_entry();
    let baseline = h.app.project.content_baseline();
    let target = h.bind_and_fill();
    let values = h.app.schema_authoring.sessions[&target].values.clone();
    h.press(Key::Escape, Modifiers::NONE);
    h.settle();
    assert!(!h.app.schema_authoring.open);
    assert_eq!(h.ctx.memory(|memory| memory.focused()), Some(entry));
    assert_eq!(h.app.schema_authoring.sessions[&target].values, values);
    // 由实际获焦的原入口重新进入，没有调用 open handler。
    h.press(Key::Enter, Modifiers::NONE);
    h.settle();
    assert!(h.app.schema_authoring.open);
    h.assert_sheet_focus();
    assert_eq!(h.app.schema_authoring.sessions[&target].values, values);

    h.tab_to("字段身份与来源", WidgetType::CollapsingHeader, false);
    h.press(Key::Enter, Modifiers::NONE);
    h.tab_to("查看现值来源", WidgetType::Link, false);
    h.press(Key::Enter, Modifiers::NONE);
    h.settle();
    assert!(h.app.schema_authoring.source_return);
    assert!(!h.app.schema_authoring.open);
    assert_eq!(h.app.schema_authoring.sessions[&target].values, values);
    // 源码编辑器的 Tab 有意用于缩进：先用真实 Escape 离开输入，再反向遍历到返回。
    h.press(Key::Escape, Modifiers::NONE);
    h.tab_to("返回资料表单", WidgetType::Button, true);
    h.press(Key::Enter, Modifiers::NONE);
    h.settle();
    h.assert_sheet_focus();
    assert_eq!(h.app.schema_authoring.active, Some(target.clone()));
    assert_eq!(h.app.schema_authoring.sessions[&target].values, values);

    let field = h.tab_to("键盘填写的新资料", WidgetType::TextEdit, false);
    h.frame(vec![Event::Ime(egui::ImeEvent::Enabled)]);
    h.frame(vec![Event::Ime(egui::ImeEvent::Preedit("候选".into()))]);
    let composing = h.app.schema_authoring.sessions[&target].values.clone();
    for key in [Key::Tab, Key::Escape] {
        h.press(key, Modifiers::NONE);
        h.settle();
        assert!(h.app.schema_authoring.open);
        assert_eq!(h.ctx.memory(|memory| memory.focused()), Some(field));
        assert_eq!(h.app.schema_authoring.sessions[&target].values, composing);
    }
    h.frame(vec![Event::Ime(egui::ImeEvent::Commit("完成".into()))]);
    h.frame(vec![Event::Ime(egui::ImeEvent::Disabled)]);
    h.settle();
    let committed = h.app.schema_authoring.sessions[&target].values.clone();
    h.press(Key::Escape, Modifiers::NONE);
    h.settle();
    assert!(!h.app.schema_authoring.open);
    assert_eq!(h.app.schema_authoring.sessions[&target].values, committed);
    assert_eq!(h.app.project.content_baseline(), baseline);
    assert!(h.app.history.is_empty());
    let source = egui::Id::new(("source", &h.app.active_file));
    assert!(h.ctx.read_response(source).unwrap().enabled());
    assert_eq!(h.ctx.memory(|memory| memory.focused()), Some(source));
}

#[test]
fn schema_full_app_keyboard_flow_allows_scrolling_away_from_focused_field() {
    let mut h = FullApp::new();
    let extra_fields = (0..6)
        .map(|index| format!("  field extra_{index} extra_{index} text\n"))
        .collect::<String>();
    let source = h
        .app
        .project
        .document(&h.app.active_file)
        .unwrap()
        .replace("character lead", &format!("{extra_fields}character lead"));
    h.app
        .project
        .set_text(&h.app.active_file.clone(), source)
        .unwrap();
    h.app.recompile();
    h.open_from_real_character_entry();
    for _ in 0..10 {
        h.press(Key::Equals, Modifiers::COMMAND);
    }
    h.settle();
    let target = h.bind_and_fill();
    let field = h.focused();
    let values = h.app.schema_authoring.sessions[&target].values.clone();
    let (scroll_id, viewport, content) = h.app.schema_authoring.body_scroll.unwrap();
    let before_offset = egui::scroll_area::State::load(&h.ctx, scroll_id)
        .unwrap()
        .offset
        .y;
    assert!(
        content.y - viewport.height() > before_offset + 20.0,
        "滚前须有真实下行余量: content={content:?}, viewport={viewport:?}, offset={before_offset}"
    );
    h.frame(vec![
        Event::PointerMoved(viewport.center()),
        Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: vec2(0.0, -180.0),
            modifiers: Modifiers::NONE,
        },
    ]);
    h.settle();
    let after_scroll = h.ctx.read_response(field.id).unwrap().rect;
    let after_offset = egui::scroll_area::State::load(&h.ctx, scroll_id)
        .unwrap()
        .offset
        .y;
    assert!(
        after_offset > before_offset + 5.0,
        "wheel须实际下滚: {before_offset} -> {after_offset}"
    );
    assert!(after_scroll.top() < field.rect.top() - 5.0);
    for _ in 0..20 {
        h.frame(vec![]);
    }
    assert_eq!(h.ctx.memory(|memory| memory.focused()), Some(field.id));
    assert!(
        egui::scroll_area::State::load(&h.ctx, scroll_id)
            .unwrap()
            .offset
            .y
            >= after_offset - 1.0
    );
    assert!(h.ctx.read_response(field.id).unwrap().rect.top() <= after_scroll.top() + 1.0);
    assert_eq!(h.app.schema_authoring.sessions[&target].values, values);
}

#[test]
fn schema_full_app_keyboard_flow_source_ime_blocks_return_without_losing_draft() {
    let mut h = FullApp::new();
    h.open_from_real_character_entry();
    let baseline = h.app.project.content_baseline();
    let target = h.bind_and_fill();
    let values = h.app.schema_authoring.sessions[&target].values.clone();
    h.tab_to("字段身份与来源", WidgetType::CollapsingHeader, false);
    h.press(Key::Enter, Modifiers::NONE);
    h.tab_to("查看现值来源", WidgetType::Link, false);
    h.press(Key::Enter, Modifiers::NONE);
    h.settle();
    let source_id = egui::Id::new(("source", &h.app.active_file));
    let source = h.ctx.read_response(source_id).unwrap();
    let rect = source.interact_rect;
    let point = [
        rect.min + vec2(8.0, 8.0),
        rect.center(),
        rect.max - vec2(8.0, 8.0),
    ]
    .into_iter()
    .find(|point| h.ctx.layer_id_at(*point) == Some(source.layer_id))
    .unwrap();
    for pressed in [true, false] {
        h.frame(vec![
            Event::PointerMoved(point),
            Event::PointerButton {
                pos: point,
                button: PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            },
        ]);
    }
    assert_eq!(h.ctx.memory(|memory| memory.focused()), Some(source_id));
    h.frame(vec![Event::Ime(egui::ImeEvent::Enabled)]);
    h.frame(vec![Event::Ime(egui::ImeEvent::Preedit("来源组合".into()))]);
    assert!(h.app.ime_composing);
    let source_draft = h.app.ime_source_draft.clone();
    assert!(source_draft.is_some());
    let output = h.settle();
    let point = FullApp::visible_text(&output, "返回资料表单")
        .unwrap()
        .center();
    for pressed in [true, false] {
        h.frame(vec![
            Event::PointerMoved(point),
            Event::PointerButton {
                pos: point,
                button: PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            },
        ]);
    }
    h.settle();
    assert!(!h.app.schema_authoring.open);
    assert!(h.app.schema_authoring.source_return);
    assert_eq!(h.app.ime_source_draft, source_draft);
    assert_eq!(h.app.schema_authoring.sessions[&target].values, values);
    assert_eq!(h.app.project.content_baseline(), baseline);
}

#[test]
fn schema_full_app_keyboard_flow_enum_selection_closes_popup() {
    let mut h = FullApp::new();
    let source = h.app.project.document(&h.app.active_file).unwrap()
        .replace("  field memo_id memo text required\n", "  field memo_id memo text required\n  field status_id status enum \"draft\" \"ready\"\n")
        .replace("  property memo = \"旧资料\"\n", "  property memo = \"旧资料\"\n  property status = \"draft\"\n");
    h.app
        .project
        .set_text(&h.app.active_file.clone(), source)
        .unwrap();
    h.app.recompile();
    h.open_from_real_character_entry();
    let target = h.bind_and_fill();
    let combo = h.tab_to("draft", WidgetType::ComboBox, false);
    h.press(Key::Enter, Modifiers::NONE);
    h.tab_to("ready", WidgetType::Button, false);
    h.press(Key::Enter, Modifiers::NONE);
    h.settle();
    assert!(!egui::ComboBox::is_open(&h.ctx, combo));
    assert_eq!(h.ctx.memory(|memory| memory.focused()), Some(combo));
    assert_eq!(
        h.app.schema_authoring.sessions[&target].values["status_id"],
        Some(PropertyValue::Str("ready".into()))
    );
}
