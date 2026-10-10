use super::*;
use crate::app::localization_ui;
use std::path::Path;
use worldline_core::localization::{
    LocalizationCatalogQuery, LocalizationPart, LocalizationSelection,
};

pub(super) struct Cleanup(PathBuf);
impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub(super) fn fixture() -> (egui::Context, WorldeditApp, Cleanup) {
    let directory = std::env::temp_dir().join("worldedit-v034-dialogue-history");
    fs::create_dir_all(&directory).unwrap();
    let (ctx, mut app) = manuscript_app_in_directory(&directory);
    let source = concat!(
        "character traveler as \"同名\"\ncharacter second as \"同名\"\n",
        "event arrival as \"抵达\"\n",
        "  say traveler \"原来的正式对白\" direction \"私有方向\" #wl-localization:spoken\n",
        "  scene harbor\n    港口旁白。\n    -> END\n  -> END\n",
        "event departure as \"离港\"\n  第二章旁白。 #wl-localization:other\n  -> END\n",
        "entity a kind place as \"同名\"\nentity b kind organization as \"同名\"\n",
    );
    let entry = app.project.entry.clone();
    app.project.set_text(&entry, source.into()).unwrap();
    fs::create_dir_all(&app.project.root).unwrap();
    app.project.save().unwrap();
    let other = app.project.add_file(Path::new("remote.wl")).unwrap();
    app.project.set_text(&other, "character remote as \"远方角色\"\nevent remote_event as \"远方章节\"\n  远方正文。\n  -> END\n".into()).unwrap();
    let book = app.project.root.join(".world/manuscripts/novel.json");
    let mut book_value: serde_json::Value =
        serde_json::from_slice(app.project.authoring_document(&book).unwrap().bytes()).unwrap();
    book_value["entries"].as_array_mut().unwrap().push(serde_json::json!({"id":"remote", "kind":"chapter", "title":"远方章节", "target_ref":{"kind":"event", "id":"remote_event"}}));
    app.project
        .set_authoring_document(&book, serde_json::to_vec(&book_value).unwrap())
        .unwrap();
    let manifest = app.project.root.join(".world/project.json");
    let mut value: serde_json::Value =
        serde_json::from_slice(app.project.authoring_document(&manifest).unwrap().bytes()).unwrap();
    value["language_version"] = "1.11".into();
    value["required_features"]
        .as_array_mut()
        .unwrap()
        .push("content.localization.v1".into());
    app.project
        .set_authoring_document(&manifest, serde_json::to_vec(&value).unwrap())
        .unwrap();
    app.project.save().unwrap();
    app.reset_views();
    app.recompile();
    assert!(
        !app.snapshot.as_ref().unwrap().result.has_errors(),
        "{:?}",
        app.snapshot.as_ref().unwrap().result.diagnostics
    );
    app.saved_location = true;
    open_body(&ctx, &mut app);
    // Explicit typed submode; its deferred switch takes effect after the current document frame.
    super::super::dialogue::toggle_mode(&ctx, &mut app);
    open_body(&ctx, &mut app);
    let cleanup = Cleanup(app.project.root.clone());
    (ctx, app, cleanup)
}

pub(super) fn open_body(ctx: &egui::Context, app: &mut WorldeditApp) {
    app.tab = Tab::Manuscript;
    for _ in 0..3 {
        frame(ctx, app, vec![], 13);
    }
}
pub(super) fn labels(output: &egui::FullOutput) -> String {
    let mut text = String::new();
    for shape in &output.shapes {
        collect_text(&shape.shape, &mut text);
    }
    text
}
pub(super) fn buffer(app: &WorldeditApp, path: &Path) -> WritingBuffer {
    app.manuscript
        .writing_buffers()
        .into_iter()
        .find(|b| b.path() == path)
        .unwrap_or_else(|| panic!("missing full buffer {}", path.display()))
}
pub(super) fn counts(app: &WorldeditApp) -> (usize, usize, usize, usize) {
    (
        app.history.len(),
        app.redo.len(),
        app.search_state.undo.len(),
        app.search_state.redo.len(),
    )
}
pub(super) fn assert_buffer(app: &WorldeditApp, expected: &WritingBuffer, current: bool) {
    let actual = buffer(app, expected.path());
    assert_eq!(actual.source(), expected.source());
    assert_eq!(actual.generation(), expected.generation());
    assert_eq!(actual.is_changed(), expected.is_changed());
    if current {
        assert_eq!(actual.baseline(), app.project.content_baseline());
        // original is intentionally private to core; this proves exact full-original equality.
        actual
            .clone()
            .rebase_unchanged_source(&app.project)
            .unwrap();
    } else {
        assert_eq!(actual.identity(), expected.identity());
        assert_eq!(actual.baseline(), expected.baseline());
    }
}
pub(super) fn disk(app: &WorldeditApp) -> BTreeMap<PathBuf, Vec<u8>> {
    worldline_core::file_access::workspace_files(&app.project.root)
        .unwrap()
        .into_iter()
        .map(|path| {
            let bytes = fs::read(&path).unwrap();
            (path, bytes)
        })
        .collect()
}
pub(super) fn project_files(project: &Project) -> BTreeMap<PathBuf, Vec<u8>> {
    project
        .sources()
        .into_iter()
        .map(|(p, s)| (p, s.into_bytes()))
        .chain(
            project
                .authoring_documents
                .iter()
                .filter(|(_, d)| !d.is_deleted())
                .map(|(p, d)| (p.clone(), d.bytes().to_vec())),
        )
        .collect()
}
pub(super) fn save_reopen(app: &mut WorldeditApp) {
    let expected = project_files(&app.project);
    #[cfg(not(target_arch = "wasm32"))]
    assert!(app.save(), "{:?}", app.io_error);
    #[cfg(target_arch = "wasm32")]
    app.project.save().unwrap();
    assert_eq!(disk(app), expected);
    let reopened = Project::open(&app.project.root).unwrap();
    assert_eq!(project_files(&reopened), expected);
    assert!(!reopened.is_dirty());
}

pub(super) fn stage(ctx: &egui::Context, app: &mut WorldeditApp, old: &str, text: &str) {
    open_body(ctx, app);
    click(ctx, app, 13, "编辑此句");
    replace_text_area(ctx, app, 13, old, text);
    assert!(app.manuscript.has_dialogue_input());
    let before = project_files(&app.project);
    click(ctx, app, 13, "预览语句变更");
    click(ctx, app, 13, "纳入正文草稿");
    assert!(
        !app.manuscript.has_dialogue_input(),
        "{}",
        labels(&frame(ctx, app, vec![], 13))
    );
    assert_eq!(project_files(&app.project), before);
    assert!(buffer(app, &app.project.entry).source().contains(text));
}
pub(super) fn apply_body(ctx: &egui::Context, app: &mut WorldeditApp) {
    open_body(ctx, app);
    click(ctx, app, 13, "应用正文草稿");
    assert!(app.io_error.is_none(), "{:?}", app.io_error);
}
pub(super) fn replace_source(ctx: &egui::Context, app: &mut WorldeditApp, text: &str) {
    frame(ctx, app, vec![], 13);
    let id = egui::Id::new(("writing-source", &app.project.entry, "event", "arrival"));
    let old = buffer(app, &app.project.entry).source().to_owned();
    focus_replace(ctx, id, &old);
    frame(ctx, app, vec![Event::Text(text.into())], 13);
    assert_eq!(buffer(app, &app.project.entry).source(), text);
}
pub(super) fn focus_replace(ctx: &egui::Context, id: egui::Id, old: &str) {
    let mut state = egui::TextEdit::load_state(ctx, id).expect("existing real TextEdit");
    state
        .cursor
        .set_char_range(Some(egui::text::CCursorRange::two(
            egui::text::CCursor::new(0),
            egui::text::CCursor::new(old.chars().count()),
        )));
    egui::TextEdit::store_state(ctx, id, state);
    ctx.memory_mut(|m| m.request_focus(id));
}

pub(super) fn locale_frame(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    events: Vec<Event>,
) -> egui::FullOutput {
    let output = ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1700.0, 1400.0))),
            events,
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                if localization_ui::show(
                    ui,
                    &mut app.project,
                    &mut app.localization_ui,
                    app.version,
                ) {
                    // Exact production Tab::Localization host contract, with UI-owned before snapshot.
                    let before = app.localization_ui.take_applied_before().unwrap();
                    app.remember(before);
                    app.recompile();
                }
            });
        },
    );
    app.localization_ui
        .settle_pending_for_test(&app.project, app.version);
    output
}
pub(super) fn locale_click(ctx: &egui::Context, app: &mut WorldeditApp, label: &str) {
    let mut last = String::new();
    for _ in 0..30 {
        let output = locale_frame(ctx, app, vec![]);
        last = labels(&output);
        if let Some(pos) = output
            .shapes
            .iter()
            .find_map(|s| clipped_text_position(&s.shape, label, s.clip_rect))
        {
            for pressed in [true, false] {
                locale_frame(
                    ctx,
                    app,
                    vec![
                        Event::PointerMoved(pos),
                        Event::PointerButton {
                            pos,
                            button: PointerButton::Primary,
                            pressed,
                            modifiers: egui::Modifiers::NONE,
                        },
                    ],
                );
            }
            return;
        }
        locale_frame(
            ctx,
            app,
            vec![
                Event::PointerMoved(pos2(850.0, 700.0)),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: vec2(0.0, -350.0),
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
    panic!("missing locale control {label}: {last}");
}
pub(super) fn open_locale(ctx: &egui::Context, app: &mut WorldeditApp) {
    app.tab = Tab::Localization;
    app.localization_ui.source_locale = "en".into();
    app.localization_ui.open_translation("fr", Some("spoken"));
    for _ in 0..3 {
        locale_frame(ctx, app, vec![]);
    }
}
pub(super) fn typed_locale(ctx: &egui::Context, app: &mut WorldeditApp) {
    open_locale(ctx, app);
    let key = "fr\u{1f}spoken";
    let id = egui::Id::new(("localization-part-text", &app.project.root, key, 0usize));
    focus_replace(ctx, id, "原来的正式对白");
    locale_frame(ctx, app, vec![Event::Text("Bonjour 😀".into())]);
    assert!(app.localization_ui.has_unsubmitted_work());
    let before = app.history.len();
    locale_click(ctx, app, "预览 1 项译文");
    locale_click(ctx, app, "应用到工程（可撤销）");
    assert_eq!(app.history.len(), before + 1);
    assert!(!app.localization_ui.has_unsubmitted_work());
    assert_translation(app);
}
pub(super) fn import_locale(ctx: &egui::Context, app: &mut WorldeditApp) {
    let selection = LocalizationSelection {
        schema_version: 1,
        source_locale: "en".into(),
        target_locale: "fr".into(),
        string_ids: vec!["spoken".into()],
    };
    let export = app.project.preview_localization_export(&selection).unwrap();
    assert!(export.can_export, "{:?}", export.diagnostics);
    let mut exchange = export.exchange;
    exchange.entries[0].translation_parts = Some(vec![LocalizationPart::Text {
        text: "Bonjour 😀".into(),
    }]);
    app.tab = Tab::Localization;
    app.localization_ui.advanced = true;
    app.localization_ui.source_locale = "en".into();
    app.localization_ui.target_locale = "fr".into();
    app.localization_ui.string_ids = "spoken".into();
    app.localization_ui.exchange_json = serde_json::to_string(&exchange).unwrap();
    let before = app.history.len();
    locale_click(ctx, app, "预览导入");
    assert!(app.localization_ui.import_plan.as_ref().unwrap().can_apply);
    locale_click(ctx, app, "复核通过 · 确认导入…");
    locale_click(ctx, app, "确认并原子导入");
    assert_eq!(app.history.len(), before + 1);
    assert!(!app.localization_ui.has_unsubmitted_work());
    assert_translation(app);
}
pub(super) fn sidecar(app: &WorldeditApp) -> PathBuf {
    app.project.root.join(".world/localization/fr.json")
}
pub(super) fn assert_translation(app: &WorldeditApp) {
    let json: serde_json::Value = serde_json::from_slice(
        app.project
            .authoring_document(&sidecar(app))
            .unwrap()
            .bytes(),
    )
    .unwrap();
    assert_eq!(
        json["entries"]["spoken"]["translation_parts"][0]["text"],
        "Bonjour 😀"
    );
}
pub(super) fn stable_id(ctx: &egui::Context, app: &mut WorldeditApp) {
    open_locale(ctx, app);
    let page = app
        .project
        .query_localization_catalog(&LocalizationCatalogQuery {
            target_locale: Some("fr".into()),
            string_ids: vec!["spoken".into()],
            ..Default::default()
        })
        .unwrap();
    let key = &page.entries[0].unit_key;
    locale_click(ctx, app, "稳定身份与来源");
    locale_frame(ctx, app, vec![]);
    let id = egui::Id::new(("localization-stable-id", &app.project.root, key));
    focus_replace(ctx, id, "");
    locale_frame(ctx, app, vec![Event::Text("spoken_new".into())]);
    locale_click(ctx, app, "预览此 ID 修改");
    let before = app.history.len();
    locale_click(ctx, app, "应用到工程（可撤销）");
    assert_eq!(app.history.len(), before + 1);
    assert!(app
        .project
        .document(&app.project.entry)
        .unwrap()
        .contains("#wl-localization:spoken_new"));
}

fn search_frame(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    events: Vec<Event>,
) -> egui::FullOutput {
    ctx.run(
        RawInput {
            screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1700.0, 1400.0))),
            events,
            ..Default::default()
        },
        |ctx| {
            app.manuscript_tab(ctx);
            app.project_search(ctx);
        },
    )
}
fn search_click(ctx: &egui::Context, app: &mut WorldeditApp, label: &str) {
    for _ in 0..3 {
        search_frame(ctx, app, vec![]);
    }
    let output = search_frame(ctx, app, vec![]);
    let pos = output
        .shapes
        .iter()
        .rev()
        .find_map(|shape| clipped_text_position(&shape.shape, label, shape.clip_rect))
        .unwrap_or_else(|| {
            panic!(
                "missing visible search control {label}: {}",
                labels(&output)
            )
        });
    for pressed in [true, false] {
        search_frame(
            ctx,
            app,
            vec![
                Event::PointerMoved(pos),
                Event::PointerButton {
                    pos,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
}
pub(super) fn search_remote_draft(ctx: &egui::Context, app: &mut WorldeditApp) {
    // Read-only search can inspect retained drafts without granting write authority.
    app.open_search(ctx, true, false);
    for _ in 0..3 {
        search_frame(ctx, app, vec![]);
    }
    focus_replace(
        ctx,
        egui::Id::new("author-search-query"),
        &app.project_query,
    );
    search_frame(ctx, app, vec![Event::Text("远方正文".into())]);
    assert_eq!(app.project_query, "远方正文");
    let hits = app.current_search_hits().unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].path, app.project.root.join("remote.wl"));
    assert!(hits[0].draft);
    search_click(ctx, app, "下一处");
    search_click(ctx, app, "关闭查找 · Esc");
    assert!(!app.search_open);
    open_body(ctx, app);
    assert_eq!(
        app.manuscript_session().selected_id.as_deref(),
        Some("remote")
    );
    assert_eq!(
        app.manuscript.active_writing_target(),
        Some((
            TargetRef::new("event", "remote_event"),
            app.project.root.join("remote.wl")
        ))
    );
}

pub(super) fn click_body_output(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    label: &str,
) -> egui::FullOutput {
    let mut last = String::new();
    for _ in 0..24 {
        let output = frame(ctx, app, vec![], 13);
        last = labels(&output);
        if let Some(pos) = output
            .shapes
            .iter()
            .find_map(|shape| clipped_text_position(&shape.shape, label, shape.clip_rect))
        {
            frame(
                ctx,
                app,
                vec![
                    Event::PointerMoved(pos),
                    Event::PointerButton {
                        pos,
                        button: PointerButton::Primary,
                        pressed: true,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                13,
            );
            return frame(
                ctx,
                app,
                vec![
                    Event::PointerMoved(pos),
                    Event::PointerButton {
                        pos,
                        button: PointerButton::Primary,
                        pressed: false,
                        modifiers: egui::Modifiers::NONE,
                    },
                ],
                13,
            );
        }
        frame(
            ctx,
            app,
            vec![
                Event::PointerMoved(pos2(850.0, 700.0)),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: vec2(0.0, -300.0),
                    modifiers: egui::Modifiers::NONE,
                },
            ],
            13,
        );
    }
    panic!("missing visible body control {label}: {last}");
}
