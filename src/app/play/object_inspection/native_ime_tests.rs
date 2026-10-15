//! 完整 App 的原生形状 IME 生命周期；合成事件不冒充桌面或物理输入法证据。
use super::*;
use crate::chrome::action_layout::{trace, ActionRect};
use egui::{Event, ImeEvent, Key, Modifiers};
use worldline_core::authoring::EventDraft;

fn frame(ctx: &egui::Context, app: &mut WorldeditApp, events: Vec<Event>) -> egui::FullOutput {
    let modifiers = events
        .iter()
        .find_map(|event| match event {
            Event::Key { modifiers, .. } => Some(*modifiers),
            _ => None,
        })
        .unwrap_or_default();
    let mut raw = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1100.0, 800.0),
        )),
        events,
        modifiers,
        focused: true,
        ..Default::default()
    };
    eframe::App::raw_input_hook(app, ctx, &mut raw);
    ctx.run(raw, |ctx| {
        eframe::App::update(app, ctx, &mut eframe::Frame::_new_kittest())
    })
}

fn key(key: Key, pressed: bool) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed,
        repeat: false,
        modifiers: Modifiers::NONE,
    }
}

struct Unchanged {
    story: (String, ReplayTrace, String),
    draft: EventDraft,
    stamp: InspectionStamp,
    query: StateInspectionQuery,
    submitted: Option<StateInspectionQuery>,
    page: StateInspectionPage,
}

impl Unchanged {
    fn capture(app: &WorldeditApp) -> Self {
        let state = &app.replay_debugger.object_inspection;
        Self {
            story: unchanged(app),
            draft: app.event_editor.as_ref().unwrap().draft.clone(),
            stamp: state.stamp.unwrap(),
            query: state.query.clone(),
            submitted: state.submitted.clone(),
            page: state.page.clone().unwrap(),
        }
    }

    fn assert(&self, app: &WorldeditApp) {
        let state = &app.replay_debugger.object_inspection;
        assert_eq!(unchanged(app), self.story);
        assert!(app.event_editor.as_ref().unwrap().draft == self.draft);
        assert_eq!(
            app.play
                .as_ref()
                .unwrap()
                .story
                .as_ref()
                .unwrap()
                .inspection_stamp(),
            self.stamp
        );
        assert_eq!(state.run, Some(RunKind::Ordinary));
        assert_eq!(state.stamp, Some(self.stamp));
        assert_eq!(
            state.query, self.query,
            "navigation must not change the side query"
        );
        assert_eq!(
            state.submitted, self.submitted,
            "navigation must not submit a query"
        );
        assert_eq!(state.page.as_ref(), Some(&self.page));
        assert!(!state.awaiting && !state.run_changed);
        assert!(state.open, "the object window must remain open");
        assert!(app.object_rehearsal_snapshot().is_none());
    }
}

struct FullApp {
    ctx: egui::Context,
    app: WorldeditApp,
    before: Unchanged,
}

impl FullApp {
    fn new() -> Self {
        let (ctx, mut app) = setup();
        ctx.style_mut(|style| style.animation_time = 0.0);
        start(&mut app);
        app.personal.pending_restore = false;
        app.tab = Tab::Edit;
        app.select_event("start");
        app.event_editor.as_mut().unwrap().draft.summary = "未应用的对象旁查事件稿".into();
        for _ in 0..3 {
            frame(&ctx, &mut app, vec![]);
        }
        select(&ctx, &mut app, "character", "hero");
        for _ in 0..3 {
            frame(&ctx, &mut app, vec![]);
        }
        let before = Unchanged::capture(&app);
        before.assert(&app);
        Self { ctx, app, before }
    }

    fn frame(&mut self, events: Vec<Event>) -> egui::FullOutput {
        let output = frame(&self.ctx, &mut self.app, events);
        self.before.assert(&self.app);
        output
    }

    fn tap(&mut self, pressed: Key) {
        self.frame(vec![key(pressed, true)]);
        self.frame(vec![key(pressed, false)]);
        self.frame(vec![]);
    }

    fn action(&self, role: &str) -> ActionRect {
        trace(&self.ctx)
            .into_iter()
            .find(|action| action.role == role)
            .unwrap_or_else(|| panic!("missing actual object action: {role}"))
    }

    fn assert_focus(&self, id: egui::Id, layer: egui::LayerId) -> egui::Response {
        assert_eq!(
            self.ctx.memory(|memory| memory.focused()),
            Some(id),
            "native lifecycle must preserve its real focus owner"
        );
        assert!(
            self.ctx.viewport(|viewport| {
                viewport
                    .this_pass
                    .widgets
                    .get(id)
                    .is_some_and(|widget| widget.enabled && widget.layer_id == layer)
            }),
            "the current pass must retain an enabled owner in its foreground domain"
        );
        let response = self.ctx.read_response(id).unwrap();
        assert_eq!(response.layer_id, layer);
        assert!(response.enabled() && response.has_focus());
        response
    }

    fn tab_to(&mut self, id: egui::Id) {
        for _ in 0..80 {
            if self.ctx.memory(|memory| memory.focused()) == Some(id) {
                self.assert_focus(id, keyboard::layer());
                return;
            }
            self.tap(Key::Tab);
            let focused = self.ctx.memory(|memory| memory.focused()).unwrap();
            self.assert_focus(focused, keyboard::layer());
        }
        panic!("real Tab did not reach the requested object-window control: {id:?}");
    }

    fn assert_run_menu(&self, popup: egui::Id, ordinary: egui::Id, layer: egui::LayerId) {
        assert!(egui::Popup::is_id_open(&self.ctx, popup));
        let current = self.action("object/run/ordinary");
        assert_eq!(current.id, ordinary);
        assert!(
            current.enabled,
            "the existing ordinary run remains available"
        );
        self.assert_focus(ordinary, layer);
        let draft = self.action("object/run/draft");
        assert!(!draft.enabled, "an unstarted draft run must stay disabled");
        assert!(!self.ctx.read_response(draft.id).unwrap().enabled());
    }

    fn open_run_menu(&mut self) -> (egui::Id, egui::Id, egui::Id, egui::LayerId) {
        let picker = self.action("object/run-picker").id;
        self.tab_to(picker);
        self.tap(Key::Enter);
        let popup = egui::Popup::default_response_id(&self.ctx.read_response(picker).unwrap());
        let ordinary = self.action("object/run/ordinary").id;
        let layer = self.ctx.read_response(ordinary).unwrap().layer_id;
        assert_eq!(layer.order, egui::Order::Foreground);
        assert_ne!(
            layer,
            keyboard::layer(),
            "the run menu owns a separate foreground layer"
        );
        self.assert_run_menu(popup, ordinary, layer);
        (picker, popup, ordinary, layer)
    }

    fn close_run_menu(&mut self, picker: egui::Id, popup: egui::Id) {
        self.tap(Key::Escape);
        assert!(!egui::Popup::is_id_open(&self.ctx, popup));
        assert!(!egui::Popup::is_any_open(&self.ctx));
        self.assert_focus(picker, keyboard::layer());
        self.before.assert(&self.app);
    }
}

fn label_rect(output: &egui::FullOutput, label: &str) -> egui::Rect {
    fn find(shape: &egui::Shape, label: &str) -> Option<egui::Rect> {
        match shape {
            egui::Shape::Text(text) if text.galley.text() == label => {
                Some(text.galley.rect.translate(text.pos.to_vec2()))
            }
            egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| find(shape, label)),
            _ => None,
        }
    }
    output
        .shapes
        .iter()
        .find_map(|shape| {
            find(&shape.shape, label).filter(|rect| shape.clip_rect.contains_rect(*rect))
        })
        .unwrap_or_else(|| panic!("actual label must be fully painted: {label}"))
}

fn search_to_changed_only(idle_enabled: bool) {
    let mut h = FullApp::new();
    // 每条路径都经真实键盘确认 Draft 不可用，Escape 只关闭运行菜单。
    let (picker, popup, _, _) = h.open_run_menu();
    h.close_run_menu(picker, popup);
    let search = egui::Id::new("object-inspection-search");
    h.tab_to(search);
    assert!(egui::TextEdit::load_state(&h.ctx, search).is_some());
    if idle_enabled {
        h.frame(vec![Event::Ime(ImeEvent::Enabled)]);
        h.assert_focus(search, keyboard::layer());
    }

    // 原生次序：Tab down 离开搜索框，两次空帧，Tab up 同批 idle Disabled。
    // 全程没有 Preedit、Commit、查询赋值或测试代码直接请求焦点。
    let output = h.frame(vec![key(Key::Tab, true)]);
    let checkbox = h.ctx.memory(|memory| memory.focused()).unwrap();
    assert_ne!(checkbox, search);
    let response = h.assert_focus(checkbox, keyboard::layer());
    assert!(
        output
            .platform_output
            .events
            .iter()
            .any(|event| matches!(event,
                egui::output::OutputEvent::FocusGained(info)
                    if info.typ == egui::WidgetType::Checkbox
                        && info.label.as_deref() == Some("只看变化")
            )),
        "Tab must focus the actual changed-only checkbox"
    );
    assert!(response.rect.contains_rect(label_rect(&output, "只看变化")));
    assert!(h.ctx.screen_rect().contains_rect(response.rect));
    for _ in 0..2 {
        h.frame(vec![]);
        h.assert_focus(checkbox, keyboard::layer());
    }
    h.frame(vec![key(Key::Tab, false), Event::Ime(ImeEvent::Disabled)]);
    h.assert_focus(checkbox, keyboard::layer());
    for _ in 0..3 {
        h.frame(vec![]);
        h.assert_focus(checkbox, keyboard::layer());
    }
    h.before.assert(&h.app);
}

#[test]
fn object_full_app_native_tab_release_disabled_keeps_changed_only_focus() {
    search_to_changed_only(false);
}

#[test]
fn object_full_app_native_idle_enabled_allows_tab_to_changed_only() {
    search_to_changed_only(true);
}

#[test]
fn object_full_app_native_idle_disabled_keeps_run_menu_focus_and_escape_restores_picker() {
    let mut h = FullApp::new();
    let (picker, popup, ordinary, layer) = h.open_run_menu();
    // 只有现存 Ordinary 可选；真实 Tab 在菜单内循环，不落回对象窗或作者输入。
    h.frame(vec![key(Key::Tab, true)]);
    h.assert_run_menu(popup, ordinary, layer);
    for _ in 0..2 {
        h.frame(vec![]);
        h.assert_run_menu(popup, ordinary, layer);
    }
    h.frame(vec![key(Key::Tab, false), Event::Ime(ImeEvent::Disabled)]);
    h.assert_run_menu(popup, ordinary, layer);
    for _ in 0..3 {
        h.frame(vec![]);
        h.assert_run_menu(popup, ordinary, layer);
    }
    h.close_run_menu(picker, popup);
}

#[test]
fn object_full_app_native_idle_enabled_escape_restores_author_without_changing_run_or_draft() {
    let (ctx, mut app) = setup();
    ctx.style_mut(|style| style.animation_time = 0.0);
    start(&mut app);
    app.personal.pending_restore = false;
    app.tab = Tab::Edit;
    app.select_event("start");
    app.event_editor.as_mut().unwrap().draft.summary = "返回时保留的未应用稿".into();
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![]);
    }
    for pressed in [true, false] {
        frame(&ctx, &mut app, vec![key(Key::Tab, pressed)]);
    }
    frame(&ctx, &mut app, vec![]);
    let opener = ctx
        .memory(|memory| memory.focused())
        .expect("real Tab establishes the author opener");
    let response = ctx.read_response(opener).unwrap();
    assert!(response.enabled() && response.has_focus());
    assert!(ctx.screen_rect().contains_rect(response.rect));
    let tab = app.tab;
    let file = app.active_file.clone();
    select(&ctx, &mut app, "character", "hero");
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![]);
    }
    assert_eq!(
        app.replay_debugger.object_inspection.return_focus,
        Some(opener)
    );
    let before = Unchanged::capture(&app);
    let mut h = FullApp { ctx, app, before };
    let search = egui::Id::new("object-inspection-search");
    h.tab_to(search);
    h.frame(vec![Event::Ime(ImeEvent::Enabled)]);
    h.assert_focus(search, keyboard::layer());
    assert!(!egui::Popup::is_any_open(&h.ctx));
    for events in [
        vec![key(Key::Escape, true)],
        vec![key(Key::Escape, false)],
        vec![],
    ] {
        frame(&h.ctx, &mut h.app, events);
        let state = &h.app.replay_debugger.object_inspection;
        assert!(
            !state.open,
            "idle Enabled must not trap Escape in the object window"
        );
        assert_eq!(h.ctx.memory(|memory| memory.focused()), Some(opener));
        assert_eq!(h.app.tab, tab);
        assert_eq!(h.app.active_file, file);
        assert_eq!(unchanged(&h.app), h.before.story);
        assert!(h.app.event_editor.as_ref().unwrap().draft == h.before.draft);
        assert_eq!(state.stamp, Some(h.before.stamp));
        assert_eq!(state.query, h.before.query);
        assert_eq!(state.submitted, h.before.submitted);
        assert_eq!(state.page.as_ref(), Some(&h.before.page));
        assert!(!state.awaiting && !state.run_changed);
        assert!(h.app.object_rehearsal_snapshot().is_none());
    }
}

#[test]
fn object_full_app_native_idle_enabled_source_button_and_back_keep_frozen_context() {
    // 回源只在无未应用执行稿时合法；其他生命周期用例继续保留其未应用 summary。
    let (ctx, mut app) = setup();
    ctx.style_mut(|style| style.animation_time = 0.0);
    start(&mut app);
    app.personal.pending_restore = false;
    app.tab = Tab::Edit;
    app.select_event("start");
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![]);
    }
    assert!(app.unapplied_play_inputs().is_empty());
    select(&ctx, &mut app, "character", "hero");
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![]);
    }
    assert!(app
        .replay_debugger
        .object_inspection
        .source_reason
        .is_none());
    let before = Unchanged::capture(&app);
    let mut h = FullApp { ctx, app, before };
    let source = h.action("object/source/mood");
    assert!(source.enabled);
    h.tab_to(source.id);
    // 真实 Tab 触发的 ScrollArea 动画独立于控件 animation_time；等其稳定后验完整命中区。
    for _ in 0..24 {
        h.frame(vec![]);
        h.assert_focus(source.id, keyboard::layer());
    }
    let response = h.assert_focus(source.id, keyboard::layer());
    let action = h.action("object/source/mood");
    assert!(
        action.clip.expand(1.0).contains_rect(response.rect),
        "settled source response must fully reveal: action={action:?}, response={:?}",
        response.rect
    );
    assert!(
        h.ctx.screen_rect().contains_rect(response.rect),
        "settled source response must fit screen: action={action:?}, response={:?}, screen={:?}",
        response.rect,
        h.ctx.screen_rect()
    );
    h.frame(vec![Event::Ime(ImeEvent::Enabled)]);
    h.assert_focus(source.id, keyboard::layer());
    for pressed in [true, false] {
        frame(&h.ctx, &mut h.app, vec![key(Key::Enter, pressed)]);
    }
    frame(&h.ctx, &mut h.app, vec![]);
    assert!(!h.app.replay_debugger.object_inspection.open);
    assert_eq!(h.app.tab, Tab::Edit);
    let (source_id, text) = h.app.source_position_document(&h.app.active_file).unwrap();
    let range = egui::TextEdit::load_state(&h.ctx, source_id)
        .unwrap()
        .cursor
        .char_range()
        .unwrap();
    let low = range.primary.index.min(range.secondary.index);
    let high = range.primary.index.max(range.secondary.index);
    let selected = text.chars().skip(low).take(high - low).collect::<String>();
    assert_eq!(
        selected,
        "state mood on character hero with calm as \"情绪\""
    );
    assert_eq!(h.ctx.memory(|memory| memory.focused()), Some(source_id));
    assert_eq!(unchanged(&h.app), h.before.story);
    assert!(h.app.event_editor.as_ref().unwrap().draft == h.before.draft);
    frame(&h.ctx, &mut h.app, vec![Event::Ime(ImeEvent::Disabled)]);
    frame(&h.ctx, &mut h.app, vec![]);
    for pressed in [true, false] {
        frame(
            &h.ctx,
            &mut h.app,
            vec![Event::Key {
                key: Key::ArrowLeft,
                physical_key: Some(Key::ArrowLeft),
                pressed,
                repeat: false,
                modifiers: Modifiers::ALT,
            }],
        );
    }
    frame(&h.ctx, &mut h.app, vec![]);
    h.before.assert(&h.app);
    h.assert_focus(source.id, keyboard::layer());
}
