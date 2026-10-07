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
        app.personal.settings.appearance.theme = theme;
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
    let page = h
        .app
        .snapshot
        .as_ref()
        .unwrap()
        .result
        .analysis
        .catalog
        .search_objects_filtered_page(
            "",
            &super::super::object_picker::filter(&[], None),
            worldline_core::object_search::ObjectSearchOptions {
                offset: index,
                limit: 1,
                ..Default::default()
            },
        )
        .unwrap();
    let object = &page.items[0];
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
fn fifteen_hundred_objects_page_to_exact_identity_and_enter_only_visible_selection() {
    let mut h = Harness::new(
        1500,
        egui::vec2(1280.0, 800.0),
        crate::theme::ThemeMode::Dark,
    );
    let baseline = h.app.project.content_baseline();
    let (label, target) = object_entry(&h, 1499);
    h.open(false);
    h.press(Key::PageDown, 74);
    h.press(Key::ArrowDown, 19);
    let output = h.settle();
    assert_eq!(h.app.command_palette.objects.options.offset, 1480);
    assert_eq!(h.app.command_palette.selected, 19);
    assert!(
        visible(&output, &label, h.size).is_some(),
        "1500th object hidden"
    );
    h.press(Key::PageUp, 50);
    let output = h.settle();
    assert_eq!(h.app.command_palette.objects.options.offset, 480);
    assert!(visible(&output, &object_entry(&h, 480).0, h.size).is_some());
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
        let index = if objects { 19 } else { 21 };
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
    h.press(Key::ArrowDown, 19);
    h.settle();
    h.frame(vec![
        Event::Ime(egui::ImeEvent::Enabled),
        key_event(Key::ArrowUp),
        key_event(Key::Enter),
        key_event(Key::Escape),
    ]);
    assert!(h.app.command_palette.open);
    assert_eq!(h.app.command_palette.selected, 19);
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

#[test]
fn refresh_rejects_old_page_enter_and_budget_error_never_opens_an_object() {
    let mut h = Harness::new(
        1500,
        egui::vec2(1280.0, 800.0),
        crate::theme::ThemeMode::Light,
    );
    h.open(false);
    h.press(Key::PageDown, 74);
    h.settle();
    assert_eq!(h.app.command_palette.objects.options.offset, 1480);
    let source = h
        .app
        .project
        .document(&h.app.active_file)
        .unwrap()
        .to_owned();
    h.app
        .project
        .set_text(
            &h.app.active_file.clone(),
            format!("// 新来源代次\n{source}"),
        )
        .unwrap();
    h.app.recompile();
    let baseline = h.app.project.content_baseline();
    h.press(Key::Enter, 1);
    assert!(h.app.command_palette.open);
    assert_eq!(h.app.command_palette.objects.options.offset, 0);
    assert_eq!(h.app.tab, Tab::Edit);
    assert!(h.app.catalog_target.is_none());
    h.app.command_palette.objects.options.max_candidates = 100;
    h.settle();
    h.press(Key::Enter, 1);
    assert!(h
        .app
        .command_palette
        .objects
        .result
        .as_ref()
        .unwrap()
        .is_err());
    assert!(h.app.command_palette.open);
    assert!(h.app.catalog_target.is_none());
    assert_eq!(h.app.project.content_baseline(), baseline);
}

#[test]
fn full_app_mention_escape_keeps_diagnostics_and_defers_to_command_and_ime_layers() {
    let mut h = Harness::new(2, egui::vec2(1280.0, 800.0), crate::theme::ThemeMode::Light);
    let source =
        "entity a kind place as \"同名\"\nentity b kind place as \"同名\"\nevent start\n  @同名";
    h.app
        .project
        .set_text(&h.app.active_file.clone(), source.into())
        .unwrap();
    h.app.recompile();
    h.app.personal.settings.diagnostics = true;
    h.settle();
    let editor = egui::Id::new(("source", &h.app.active_file));
    let mut state = egui::TextEdit::load_state(&h.ctx, editor).unwrap();
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::one(
            egui::text::CCursor::new(source.chars().count()),
        )));
    state.store(&h.ctx, editor);
    h.ctx.memory_mut(|memory| memory.request_focus(editor));
    h.settle();
    assert!(h.app.source_mention_owns_escape(&h.ctx));
    let before = h.app.project.content_baseline();
    h.press(Key::Escape, 1);
    assert!(h.app.personal.settings.diagnostics);
    assert!(h.app.mention_suppression.is_some());
    assert_eq!(h.app.project.content_baseline(), before);
    h.app.mention_suppression = None;
    h.settle();
    h.open(true);
    h.press(Key::Escape, 1);
    assert!(!h.app.command_palette.open);
    assert!(h.app.mention_suppression.is_none());
    assert!(h.app.personal.settings.diagnostics);
    h.ctx.memory_mut(|memory| memory.request_focus(editor));
    h.size = egui::vec2(640.0, 600.0);
    h.app.reading_panels.pin(TargetRef::new("entity", "a"));
    h.app.personal.settings.references_visible = true;
    h.app.personal.settings.dock_references = true;
    h.ctx
        .data_mut(|data| data.insert_temp(egui::Id::new("compact-reference-drawer"), true));
    h.frame(vec![]);
    assert!(h.app.compact_reference_open(&h.ctx));
    h.ctx.memory_mut(|memory| memory.request_focus(editor));
    assert!(!h.app.source_mention_owns_escape(&h.ctx));
    h.press(Key::Escape, 1);
    assert!(!h.app.compact_reference_open(&h.ctx));
    assert!(h.app.mention_suppression.is_none());
    h.ctx.memory_mut(|memory| memory.request_focus(editor));
    h.frame(vec![
        Event::Ime(egui::ImeEvent::Enabled),
        key_event(Key::Escape),
    ]);
    assert!(h.app.personal.settings.diagnostics);
    assert!(h.app.mention_suppression.is_none());
    assert_eq!(h.app.project.content_baseline(), before);
    h.frame(vec![Event::Ime(egui::ImeEvent::Disabled)]);
}

#[test]
fn palette_pointer_press_cannot_survive_source_revision_or_page_replacement() {
    for page_change in [false, true] {
        let mut h = Harness::new(
            50,
            egui::vec2(1280.0, 800.0),
            crate::theme::ThemeMode::Light,
        );
        h.open(false);
        let output = h.settle();
        let point = visible(&output, &object_entry(&h, 1).0, h.size)
            .unwrap()
            .center();
        h.frame(vec![
            Event::PointerMoved(point),
            Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Modifiers::NONE,
            },
        ]);
        if page_change {
            h.app.command_palette.objects.options.offset = 20;
        } else {
            let source = h
                .app
                .project
                .document(&h.app.active_file)
                .unwrap()
                .replace("同名对象 0001", "更新对象 0001");
            h.app
                .project
                .set_text(&h.app.active_file.clone(), source)
                .unwrap();
            h.app.recompile();
        }
        let baseline = h.app.project.content_baseline();
        h.frame(vec![]);
        h.frame(vec![]);
        h.frame(vec![
            Event::PointerMoved(point),
            Event::PointerButton {
                pos: point,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            },
        ]);
        assert!(h.app.command_palette.open);
        assert!(h.app.catalog_target.is_none());
        assert_eq!(h.app.tab, Tab::Edit);
        assert_eq!(h.app.project.content_baseline(), baseline);
    }
}
