//! 命令/对象快速切换的真实 egui 帧；不替代原生窗口或物理输入法验收。
use super::*;
use egui::{Event, Key, Modifiers, Pos2, Rect, Vec2};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Harness {
    ctx: egui::Context,
    app: WorldeditApp,
    size: Vec2,
}
impl Harness {
    fn new(objects: usize, size: Vec2, theme: crate::theme::ThemeMode) -> Self {
        static NEXT_ROOT: AtomicUsize = AtomicUsize::new(0);
        let ctx = egui::Context::default();
        let mut app = WorldeditApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()), None);
        let root = std::env::temp_dir().join(format!(
            "visible-command-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            NEXT_ROOT.fetch_add(1, Ordering::Relaxed)
        ));
        app.project = worldline_core::project::Project::new(&root);
        app.active_file = app.project.entry.clone();
        app.project.documents.retain(|p, _| p == &app.active_file);
        let source = (0..objects)
            .map(|i| format!("entity item_{i:04} kind place as \"同名对象 {i:04}\"\n"))
            .collect::<String>();
        app.project
            .set_text(&app.active_file.clone(), source)
            .unwrap();
        app.project.create_authoring_document(&root.join(".world/project.json"),
            br#"{"schema_version":1,"language_version":"1.13","required_features":["content.entities.v1"]}"#.to_vec()).unwrap();
        app.project.save().unwrap();
        app.saved_location = true;
        app.personal.settings.theme = theme;
        app.personal.pending_restore = false;
        app.recompile();
        app.tab = Tab::Edit;
        let mut h = Self { ctx, app, size };
        h.frame(vec![]);
        h.ctx.style_mut(|s| s.animation_time = 0.0);
        h
    }
    fn frame(&mut self, events: Vec<Event>) -> egui::FullOutput {
        self.ctx.run(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, self.size)),
                events,
                ..Default::default()
            },
            |ctx| {
                eframe::App::update(&mut self.app, ctx, &mut eframe::Frame::_new_kittest());
            },
        )
    }
    fn settle(&mut self) -> egui::FullOutput {
        for _ in 0..24 {
            self.frame(vec![]);
        }
        self.frame(vec![])
    }
    fn open(&mut self, commands_only: bool) {
        self.app.open_commands(&self.ctx, commands_only);
        self.settle();
    }
    fn press(&mut self, key: Key, times: usize) {
        self.frame((0..times).map(|_| key_event(key)).collect());
    }
}
impl Drop for Harness {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.app.project.root);
    }
}
fn key_event(key: Key) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    }
}
fn text_rect(shape: &egui::Shape, label: &str) -> Option<Rect> {
    match shape {
        egui::Shape::Text(text) if text.galley.job.text == label => {
            Some(text.galley.rect.translate(text.pos.to_vec2()))
        }
        egui::Shape::Vec(shapes) => shapes.iter().find_map(|shape| text_rect(shape, label)),
        _ => None,
    }
}
fn visible(output: &egui::FullOutput, label: &str, size: Vec2) -> Option<Rect> {
    output.shapes.iter().find_map(|shape| {
        let rect = text_rect(&shape.shape, label)?;
        (shape.clip_rect.contains_rect(rect)
            && Rect::from_min_size(Pos2::ZERO, size).contains_rect(rect))
        .then_some(rect)
    })
}
fn object_entry(h: &Harness, index: usize) -> (String, TargetRef) {
    let object = super::super::object_picker::candidates(
        &h.app.snapshot.as_ref().unwrap().result.analysis.catalog,
        "",
        &[],
    )[index];
    (
        super::super::object_picker::candidate_caption(object, Some(&h.app.project.root)),
        object.target.clone(),
    )
}

#[test]
fn all_31_commands_follow_keyboard_selection_across_dark_light_and_sizes() {
    assert_eq!(commands().len(), 31);
    let review_index = commands()
        .iter()
        .position(|(_, action)| matches!(action, Action::Tab(Tab::Review)))
        .unwrap();
    let import_index = commands()
        .iter()
        .position(|(_, action)| matches!(action, Action::Tab(Tab::CatalogImport)))
        .expect("CSV import must have an explicit author command");
    let source_jump_index = commands()
        .iter()
        .position(|(_, action)| matches!(action, Action::SourceJump))
        .expect("source coordinates must have an explicit author command");
    let move_entity_index = commands()
        .iter()
        .position(|(_, action)| matches!(action, Action::MoveEntitySource))
        .expect("entity source move must have an explicit author command");
    for theme in [
        crate::theme::ThemeMode::Dark,
        crate::theme::ThemeMode::Light,
    ] {
        for size in [
            egui::vec2(1040.0, 660.0),
            egui::vec2(1280.0, 800.0),
            egui::vec2(1600.0, 1000.0),
        ] {
            let mut h = Harness::new(1, size, theme);
            let baseline = h.app.project.content_baseline();
            h.open(true);
            for index in 1..commands().len() {
                h.press(Key::ArrowDown, 1);
                let output = h.settle();
                assert_eq!(h.app.command_palette.selected, index);
                assert!(
                    visible(&output, commands()[index].0, size).is_some(),
                    "{theme:?} {size:?}: command {index} hidden"
                );
            }
            for index in (0..commands().len() - 1).rev() {
                h.press(Key::ArrowUp, 1);
                let output = h.settle();
                assert_eq!(h.app.command_palette.selected, index);
                assert!(
                    visible(&output, commands()[index].0, size).is_some(),
                    "{theme:?} {size:?}: upward command {index} hidden"
                );
            }
            h.press(Key::ArrowDown, review_index);
            let output = h.settle();
            assert_eq!(h.app.command_palette.selected, review_index);
            assert!(visible(&output, commands()[review_index].0, size).is_some());
            h.press(Key::Enter, 1);
            assert!(!h.app.command_palette.open);
            assert_eq!(h.app.tab, Tab::Review);
            assert_eq!(h.app.project.content_baseline(), baseline);
            // The added command is a new entry point, not merely a count change:
            // prove it is visible and activates the import workbench by keyboard.
            h.open(true);
            h.press(Key::ArrowDown, import_index);
            let output = h.settle();
            assert_eq!(h.app.command_palette.selected, import_index);
            assert!(visible(&output, commands()[import_index].0, size).is_some());
            h.press(Key::Enter, 1);
            assert!(!h.app.command_palette.open);
            assert_eq!(h.app.tab, Tab::CatalogImport);
            assert_eq!(h.app.project.content_baseline(), baseline);
            // 每种主题和尺寸均实际执行新增命令，不能只增长计数。
            h.app.catalog_target = Some(TargetRef::new("entity", "item_0000"));
            h.open(true);
            h.press(Key::ArrowDown, move_entity_index);
            let output = h.settle();
            assert_eq!(h.app.command_palette.selected, move_entity_index);
            assert!(visible(&output, commands()[move_entity_index].0, size).is_some());
            h.press(Key::Enter, 1);
            assert!(!h.app.command_palette.open);
            assert_eq!(
                h.app.entity_source_move_form.as_ref().unwrap().id,
                "item_0000"
            );
            assert_eq!(h.app.project.content_baseline(), baseline);
            h.press(Key::Escape, 1);
            assert!(h.app.entity_source_move_form.is_none());
            assert_eq!(h.app.project.content_baseline(), baseline);
            h.app.tab = Tab::Edit;
            h.open(true);
            h.press(Key::ArrowDown, source_jump_index);
            let output = h.settle();
            assert_eq!(h.app.command_palette.selected, source_jump_index);
            assert!(visible(&output, commands()[source_jump_index].0, size).is_some());
            h.press(Key::Enter, 1);
            h.settle();
            assert!(!h.app.command_palette.open);
            assert!(h.app.source_jump.open);
            assert_eq!(h.app.project.content_baseline(), baseline);
            h.press(Key::Escape, 1);
            assert!(!h.app.source_jump.open);
            assert_eq!(h.app.project.content_baseline(), baseline);
        }
    }
}

#[test]
fn thousand_objects_scroll_to_exact_identity_and_enter_only_visible_selection() {
    let mut h = Harness::new(
        1000,
        egui::vec2(1280.0, 800.0),
        crate::theme::ThemeMode::Dark,
    );
    let baseline = h.app.project.content_baseline();
    let (label, target) = object_entry(&h, 999);
    h.open(false);
    h.press(Key::ArrowDown, 999);
    let output = h.settle();
    assert_eq!(h.app.command_palette.selected, 999);
    assert!(
        visible(&output, &label, h.size).is_some(),
        "1000th object hidden"
    );
    h.press(Key::ArrowUp, 500);
    let output = h.settle();
    assert!(visible(&output, &object_entry(&h, 499).0, h.size).is_some());
    h.frame(vec![Event::Text(target.id.clone())]);
    let output = h.settle();
    assert_eq!(
        h.app.command_palette.selected, 0,
        "真实输入过滤后应归一首项"
    );
    assert!(visible(&output, &label, h.size).is_some());
    h.press(Key::Enter, 1);
    assert!(!h.app.command_palette.open);
    assert_eq!(h.app.catalog_target, Some(target.clone()));
    if target.kind == "entity" {
        assert_eq!(h.app.entity_editor.as_ref().unwrap().draft.id, target.id);
    }
    assert_eq!(h.app.project.content_baseline(), baseline);
}

#[test]
fn manual_scroll_stays_put_and_hidden_enter_never_executes() {
    for objects in [false, true] {
        let mut h = Harness::new(
            1000,
            egui::vec2(1280.0, 800.0),
            crate::theme::ThemeMode::Light,
        );
        h.open(!objects);
        let index = if objects { 999 } else { 21 };
        let label = if objects {
            object_entry(&h, index).0
        } else {
            commands()[index].0.into()
        };
        h.press(Key::ArrowDown, index);
        let output = h.settle();
        let point = visible(&output, &label, h.size).unwrap().center();
        h.frame(vec![
            Event::PointerMoved(point),
            Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, 100_000.0),
                modifiers: Modifiers::NONE,
            },
        ]);
        let output = h.settle();
        assert!(
            visible(&output, &label, h.size).is_none(),
            "manual scroll was pulled back"
        );
        assert_eq!(h.app.command_palette.selected, index);
        h.press(Key::Enter, 1);
        assert!(
            h.app.command_palette.open,
            "hidden selection must not execute"
        );
        assert_eq!(h.app.tab, Tab::Edit);
        assert!(!h.app.personal.preferences_open);
        let output = h.settle();
        assert!(
            visible(&output, &label, h.size).is_none(),
            "idle/Enter must not scroll"
        );
        h.press(Key::ArrowUp, 1);
        let output = h.settle();
        let next = if objects {
            object_entry(&h, index - 1).0
        } else {
            commands()[index - 1].0.into()
        };
        assert!(visible(&output, &next, h.size).is_some());
    }
}

#[test]
fn empty_filter_and_ime_do_not_execute_and_escape_restores_source_focus() {
    let mut h = Harness::new(
        1000,
        egui::vec2(1280.0, 800.0),
        crate::theme::ThemeMode::Dark,
    );
    let focus = egui::Id::new(("source", &h.app.active_file));
    h.ctx.memory_mut(|m| m.request_focus(focus));
    h.frame(vec![]);
    let baseline = h.app.project.content_baseline();
    h.open(false);
    h.press(Key::ArrowDown, 999);
    h.settle();
    h.frame(vec![
        Event::Ime(egui::ImeEvent::Enabled),
        key_event(Key::ArrowUp),
        key_event(Key::Enter),
        key_event(Key::Escape),
    ]);
    assert!(h.app.command_palette.open);
    assert_eq!(h.app.command_palette.selected, 999);
    for event in [
        egui::ImeEvent::Preedit("中".into()),
        egui::ImeEvent::Commit("中".into()),
    ] {
        h.frame(vec![
            Event::Ime(event),
            key_event(Key::ArrowUp),
            key_event(Key::Enter),
            key_event(Key::Escape),
        ]);
        assert!(h.app.command_palette.open);
        assert_eq!(h.app.tab, Tab::Edit);
    }
    h.app.command_palette.query = "unmatched-object".into();
    h.settle();
    h.press(Key::Enter, 1);
    assert!(h.app.command_palette.open);
    h.press(Key::Escape, 1);
    assert!(!h.app.command_palette.open);
    assert_eq!(h.ctx.memory(|m| m.focused()), Some(focus));
    assert_eq!(h.app.project.content_baseline(), baseline);
    h.open(false);
    let output = h.settle();
    assert_eq!(h.app.command_palette.selected, 0);
    assert!(visible(&output, &object_entry(&h, 0).0, h.size).is_some());
}
