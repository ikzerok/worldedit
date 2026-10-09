//! Cross-feature travel uses the real C plan/apply and A1 preview/confirmation host paths.
use super::cross_feature_undo::{click_localization, localization_frame};
use super::*;
use worldline_core::{localization::LocalizationSelection, manuscript::WritingBuffer};

pub(super) fn locale_fixture() -> (egui::Context, WorldeditApp) {
    let (ctx, mut app) = fixture();
    let path = app.project.entry.clone();
    let manifest = app.project.root.join(".world/project.json");
    app.project
        .set_text(
            &path,
            TEXT.replace("走向灯塔。", "走向灯塔。 #wl-localization:body"),
        )
        .unwrap();
    let mut value: serde_json::Value =
        serde_json::from_slice(app.project.authoring_document(&manifest).unwrap().bytes()).unwrap();
    value["required_features"] =
        serde_json::json!(["presentation.manuscripts.v1", "content.localization.v1"]);
    app.project
        .set_authoring_document(&manifest, serde_json::to_vec(&value).unwrap())
        .unwrap();
    app.project.save().unwrap();
    app.reset_views();
    app.recompile();
    open_body(&ctx, &mut app);
    (ctx, app)
}

pub(super) fn open_body(ctx: &egui::Context, app: &mut WorldeditApp) {
    app.tab = Tab::Manuscript;
    for _ in 0..3 {
        frame(ctx, app, Vec::new());
    }
}

pub(super) fn apply_locale(ctx: &egui::Context, app: &mut WorldeditApp) {
    apply_locale_to(ctx, app, "zh-Hant");
}

fn edit_exchange_target_locale(ctx: &egui::Context, app: &mut WorldeditApp, locale: &str) {
    for _ in 0..3 {
        localization_frame(ctx, app, Vec::new());
    }
    // Advanced exchange owns a separate locale draft after its first render.
    // Edit that actual TextEdit; changing the catalog's target_locale field is insufficient.
    let id = egui::Id::new(("localization-target-locale", &app.project.root, true));
    let response = ctx
        .read_response(id)
        .expect("advanced target locale TextEdit");
    assert!(ctx.screen_rect().contains_rect(response.rect));
    let pos = response.rect.center();
    for pressed in [true, false] {
        localization_frame(
            ctx,
            app,
            vec![
                Event::PointerMoved(pos),
                Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: egui::Modifiers::NONE,
                },
            ],
        );
    }
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(id));
    let modifiers = egui::Modifiers {
        ctrl: true,
        command: true,
        ..egui::Modifiers::NONE
    };
    for pressed in [true, false] {
        localization_frame(
            ctx,
            app,
            vec![Event::Key {
                key: egui::Key::A,
                physical_key: Some(egui::Key::A),
                pressed,
                repeat: false,
                modifiers,
            }],
        );
    }
    localization_frame(ctx, app, vec![Event::Text(locale.into())]);
}

pub(super) fn apply_locale_to(ctx: &egui::Context, app: &mut WorldeditApp, locale: &str) {
    let selection = LocalizationSelection {
        schema_version: 1,
        source_locale: "en".into(),
        target_locale: locale.into(),
        string_ids: vec!["body".into()],
    };
    let export = app.project.preview_localization_export(&selection).unwrap();
    assert!(export.can_export, "{:?}", export.diagnostics);
    let mut exchange = export.exchange;
    for entry in &mut exchange.entries {
        entry.translation_parts = Some(entry.source_parts.clone());
    }
    app.tab = Tab::Localization;
    app.localization_ui.advanced = true;
    app.localization_ui.source_locale = selection.source_locale;
    app.localization_ui.target_locale = selection.target_locale;
    app.localization_ui.string_ids = "body".into();
    app.localization_ui.exchange_json = serde_json::to_string(&exchange).unwrap();
    edit_exchange_target_locale(ctx, app, locale);
    assert_eq!(
        app.localization_ui.exchange_json,
        serde_json::to_string(&exchange).unwrap()
    );
    assert!(!click_localization(ctx, app, "预览导入"));
    let plan = app.localization_ui.import_plan.as_ref().unwrap().clone();
    let rendered = labels(&localization_frame(ctx, app, Vec::new()).1);
    assert!(
        plan.can_apply,
        "requested={locale}; ordinary source={}; ordinary target={}; ids={}; plan={plan:?}; rendered advanced form:\n{rendered}",
        app.localization_ui.source_locale,
        app.localization_ui.target_locale,
        app.localization_ui.string_ids
    );
    assert_eq!(plan.target_locale, locale);
    assert!(!click_localization(ctx, app, "复核通过 · 确认导入…"));
    assert!(click_localization(ctx, app, "确认并原子导入"));
}

pub(super) fn dirty_buffers(ctx: &egui::Context, app: &mut WorldeditApp) -> Vec<WritingBuffer> {
    open_body(ctx, app);
    let path = app.project.entry.clone();
    let source = app.manuscript.writing_buffers[&path]
        .source()
        .replace("走向", "慢慢走向");
    app.manuscript
        .writing_buffers
        .get_mut(&path)
        .unwrap()
        .replace_source(source);
    let mut target = app
        .project
        .open_source_writing_buffer(&app.project.root.join("资料.wl"))
        .unwrap();
    target.replace_source("// 尚未应用的完整资料说明😀\n".into());
    app.manuscript.restore_writing_buffers(&[target]);
    app.manuscript.writing_buffers()
}

pub(super) fn apply_link(ctx: &egui::Context, app: &mut WorldeditApp, creates: bool) {
    open_body(ctx, app);
    select_name(ctx, app);
    app.begin_manuscript_world_links(ctx);
    if creates {
        new_character(app);
    } else {
        app.manuscript.world_links.as_mut().unwrap().chosen =
            Some(TargetRef::new("character", "lin"));
    }
    let mut state = app.manuscript.world_links.take().unwrap();
    app.preview_manuscript_world_link(&mut state);
    assert!(state.plan.is_some(), "{:?}", state.error);
    assert!(
        app.apply_manuscript_world_link(ctx, &mut state),
        "{:?}",
        state.error
    );
}

pub(super) fn assert_buffers(app: &WorldeditApp, expected: &[WritingBuffer]) {
    for buffer in expected {
        let actual = app
            .manuscript
            .writing_buffers
            .get(buffer.path())
            .unwrap_or_else(|| panic!("missing full buffer {}", buffer.path().display()));
        assert_eq!(actual.source(), buffer.source());
        assert_eq!(actual.generation(), buffer.generation());
        assert_eq!(actual.is_changed(), buffer.is_changed());
        assert_eq!(actual.baseline(), app.project.content_baseline());
        let mut proof = actual.clone();
        proof.rebase_unchanged_source(&app.project).unwrap();
    }
}

pub(super) fn ordinary_edit(app: &mut WorldeditApp, number: usize) {
    let path = app.project.root.join("unrelated.wl");
    assert!(
        app.commit("历史邻近操作", |project| {
            project.set_text(&path, format!("// unrelated {number}\n"))
        }),
        "{:?}",
        app.io_error
    );
}

#[test]
fn a1_then_c_multi_step_history_preserves_full_drafts_before_and_after_save() {
    for save_first in [false, true] {
        let (ctx, mut app) = locale_fixture();
        let root = app.project.root.clone();
        let initial = app.project.sources();
        let sidecar = root.join(".world/localization/zh-Hant.json");
        apply_locale(&ctx, &mut app);
        let originals = dirty_buffers(&ctx, &mut app);
        apply_link(&ctx, &mut app, true);
        assert_eq!(app.history.len(), 2);
        let applied = app.project.sources();
        let translation = app
            .project
            .authoring_document(&sidecar)
            .unwrap()
            .bytes()
            .to_vec();
        if save_first {
            app.project.save().unwrap();
        }
        for _ in 0..2 {
            app.edit_undo(false);
            assert!(app.io_error.is_none(), "{:?}", app.io_error);
            assert_buffers(&app, &originals);
            app.edit_undo(false);
            assert!(app.io_error.is_none(), "{:?}", app.io_error);
            assert_eq!(app.project.sources(), initial);
            assert_buffers(&app, &originals);
            assert!(app
                .project
                .authoring_document(&sidecar)
                .unwrap()
                .is_deleted());
            app.edit_undo(true);
            assert_buffers(&app, &originals);
            app.edit_undo(true);
            assert!(app.io_error.is_none(), "{:?}", app.io_error);
            assert_eq!(app.project.sources(), applied);
            assert_eq!(
                app.project.authoring_document(&sidecar).unwrap().bytes(),
                translation
            );
        }
        app.project.save().unwrap();
        assert_eq!(Project::open(&root).unwrap().sources(), applied);
        assert_eq!(fs::read(&sidecar).unwrap(), translation);
        app.edit_undo(false);
        app.edit_undo(false);
        assert_buffers(&app, &originals);
        app.project.save().unwrap();
        assert_eq!(Project::open(&root).unwrap().sources(), initial);
        assert!(!sidecar.exists());
        assert_buffers(&app, &originals);
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn clean_buffer_generation_survives_a1_then_reference_or_compound_and_double_redo() {
    for creates in [false, true] {
        let (ctx, mut app) = locale_fixture();
        let root = app.project.root.clone();
        apply_locale(&ctx, &mut app);
        open_body(&ctx, &mut app);
        let path = app.project.entry.clone();
        let original = app.manuscript.writing_buffers[&path].source().to_owned();
        let buffer = app.manuscript.writing_buffers.get_mut(&path).unwrap();
        buffer.replace_source(format!("{original}\n// temporary input\n"));
        buffer.replace_source(original);
        assert!(!buffer.is_changed());
        assert!(buffer.generation() > 0);
        let originals = app.manuscript.writing_buffers();
        apply_link(&ctx, &mut app, creates);
        let linked = if creates {
            app.project.document(&path).unwrap().to_owned()
        } else {
            app.manuscript.writing_buffers[&path].source().to_owned()
        };
        for _ in 0..2 {
            app.edit_undo(false);
            assert_buffers(&app, &originals);
            app.edit_undo(false);
            assert_buffers(&app, &originals);
            app.edit_undo(true);
            assert_buffers(&app, &originals);
            app.edit_undo(true);
            assert!(app.io_error.is_none(), "{:?}", app.io_error);
            if creates {
                assert_eq!(app.project.document(&path).unwrap(), linked);
            } else {
                assert_eq!(app.manuscript.writing_buffers[&path].source(), linked);
            }
        }
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn new_identical_compound_branch_owns_its_own_draft_generation_and_clears_redo() {
    let (ctx, mut app) = locale_fixture();
    let root = app.project.root.clone();
    let originals = dirty_buffers(&ctx, &mut app);
    apply_link(&ctx, &mut app, true);
    let first_applied = app.project.content_baseline();
    app.edit_undo(false);
    assert_buffers(&app, &originals);
    let old_node = app.history_state.current;
    for original in &originals {
        let buffer = app
            .manuscript
            .writing_buffers
            .get_mut(original.path())
            .unwrap();
        buffer.replace_source(format!("{}\n// 临时改稿", original.source()));
        buffer.replace_source(original.source().to_owned());
    }
    let branch = app.manuscript.writing_buffers();
    apply_link(&ctx, &mut app, true);
    assert_eq!(app.project.content_baseline(), first_applied);
    assert_ne!(app.history_state.current, old_node);
    assert!(app.redo.is_empty());
    apply_locale(&ctx, &mut app);
    app.edit_undo(false);
    app.edit_undo(false);
    assert_buffers(&app, &branch);
    assert!(branch.iter().all(|buffer| originals
        .iter()
        .any(|old| { old.path() == buffer.path() && old.generation() < buffer.generation() })));
    app.edit_undo(true);
    app.edit_undo(true);
    assert!(app.io_error.is_none(), "{:?}", app.io_error);
    fs::remove_dir_all(root).unwrap();
}
