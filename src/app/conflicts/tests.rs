//! egui输入/几何与状态回归；不宣称物理平台或真实IME验收。
use super::*;
use egui::{Event, Key, Modifiers, Rect};
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};
mod geometry;

static JOB_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn fixture() -> (egui::Context, WorldeditApp) {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let root = std::env::temp_dir().join(format!(
        "worldedit-reconciliation-ui-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("world.wl"), "event start\n  基线\n  -> END\n").unwrap();
    let ctx = egui::Context::default();
    let mut app = WorldeditApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()), None);
    app.personal.settings.appearance.reduce_motion = true;
    app.project = Project::open_read_only(&root).unwrap();
    app.active_file = app.project.entry.clone();
    app.project
        .set_text(
            &app.active_file,
            "event start\n  本地雨🙂\n  -> END\n".into(),
        )
        .unwrap();
    fs::write(&app.active_file, "event start\n  磁盘风🙂\n  -> END\n").unwrap();
    app.recompile();
    app.conflict_view = ConflictView {
        open: true,
        workspace_root: app.project.root.clone(),
        session: Some(app.project.capture_reconciliation().unwrap()),
        request_focus: true,
        ..Default::default()
    };
    (ctx, app)
}
fn frame(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    size: egui::Vec2,
    events: Vec<Event>,
) -> egui::FullOutput {
    ctx.run(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(egui::Pos2::ZERO, size)),
            events,
            ..Default::default()
        },
        |ctx| {
            let _theme = crate::theme::configure_appearance(ctx, app.personal.appearance());
            app.conflict_view.prepare_frame(ctx);
            if !app.conflict_view.is_open() {
                app.author_shortcuts(ctx);
            }
            egui::CentralPanel::default().show(ctx, |ui| {
                let mut original = String::from("原作者位置");
                ui.add(
                    egui::TextEdit::singleline(&mut original)
                        .id(egui::Id::new("original-source-focus")),
                );
            });
            app.show_reconciliation(ctx);
        },
    )
}
fn settle(ctx: &egui::Context, app: &mut WorldeditApp, size: egui::Vec2) -> egui::FullOutput {
    for _ in 0..6 {
        frame(ctx, app, size, vec![]);
    }
    frame(ctx, app, size, vec![])
}
fn visible(output: &egui::FullOutput, label: &str, viewport: Rect) -> bool {
    output.shapes.iter().any(|shape| match &shape.shape {
        egui::Shape::Text(text) if text.galley.text() == label => {
            let rect = text.galley.rect.translate(text.pos.to_vec2());
            shape.clip_rect.contains_rect(rect) && viewport.contains_rect(rect)
        }
        _ => false,
    })
}
fn key(key: Key) -> Event {
    Event::Key {
        key,
        physical_key: Some(key),
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    }
}

#[test]
fn reconciliation_actions_stay_inside_800x600_and_200_percent_logical_viewport() {
    for size in [egui::vec2(800.0, 600.0), egui::vec2(400.0, 300.0)] {
        let (ctx, mut app) = fixture();
        let output = settle(&ctx, &mut app, size);
        let viewport = Rect::from_min_size(egui::Pos2::ZERO, size);
        for label in ["关闭并保留", "预览候选", "采纳到内存"] {
            assert!(
                visible(&output, label, viewport),
                "{label} clipped at {size:?}"
            );
        }
        let _ = fs::remove_dir_all(app.project.root);
    }
}

#[test]
fn escape_closes_only_materials_and_preserves_manual_choice_and_source_focus() {
    let (ctx, mut app) = fixture();
    let source_focus = egui::Id::new("original-source-focus");
    ctx.memory_mut(|memory| memory.request_focus(source_focus));
    app.conflict_view.return_focus = Some(source_focus);
    app.conflict_view.drafts.insert(
        "world.wl".into(),
        Draft {
            choice: Some(ReconciliationChoice::Manual {
                text: String::new(),
            }),
            manual: "手工候选🙂".into(),
        },
    );
    let before = app.project.content_baseline();
    settle(&ctx, &mut app, egui::vec2(800.0, 600.0));
    frame(
        &ctx,
        &mut app,
        egui::vec2(800.0, 600.0),
        vec![key(Key::Escape)],
    );
    assert!(!app.conflict_view.open);
    assert_eq!(
        app.conflict_view.drafts[&PathBuf::from("world.wl")].manual,
        "手工候选🙂"
    );
    assert_eq!(app.project.content_baseline(), before);
    assert_eq!(ctx.memory(|memory| memory.focused()), Some(source_focus));
    app.conflict_view.open_or_capture(&app.project);
    assert_eq!(
        app.conflict_view.drafts[&PathBuf::from("world.wl")].manual,
        "手工候选🙂"
    );
    let _ = fs::remove_dir_all(app.project.root);
}

#[test]
fn ime_escape_keeps_candidate_window_open() {
    let (ctx, mut app) = fixture();
    settle(&ctx, &mut app, egui::vec2(800.0, 600.0));
    frame(
        &ctx,
        &mut app,
        egui::vec2(800.0, 600.0),
        vec![
            Event::Ime(egui::ImeEvent::Preedit("中".into())),
            key(Key::Escape),
        ],
    );
    assert!(app.conflict_view.open);
    let _ = fs::remove_dir_all(app.project.root);
}

#[test]
fn side_tabs_show_exact_disk_bytes_and_readonly_render_keeps_all_sides() {
    let (ctx, mut app) = fixture();
    let session = app.conflict_view.session.clone().unwrap();
    for side in 0..3 {
        app.conflict_view.side = side;
        settle(&ctx, &mut app, egui::vec2(800.0, 600.0));
        assert_eq!(app.conflict_view.session.as_ref().unwrap(), &session);
    }
    assert_eq!(
        session.files[0].disk.as_deref(),
        Some("event start\n  磁盘风🙂\n  -> END\n".as_bytes())
    );
    let _ = fs::remove_dir_all(app.project.root);
}

#[test]
fn unapplied_form_does_not_allow_adoption_or_clear_input() {
    let (ctx, mut app) = fixture();
    app.new_file = Some("未提交的文件名称".into());
    let session = app.conflict_view.session.as_ref().unwrap().clone();
    app.conflict_view.drafts.insert(
        "world.wl".into(),
        Draft {
            choice: Some(ReconciliationChoice::Disk),
            manual: String::new(),
        },
    );
    app.conflict_view.preview = Some(
        app.project
            .preview_reconciliation(&session, &app.conflict_view.request())
            .unwrap(),
    );
    let before = app.project.content_baseline();
    settle(&ctx, &mut app, egui::vec2(800.0, 600.0));
    assert!(!app.dirty_draft_names().is_empty());
    assert_eq!(app.new_file.as_deref(), Some("未提交的文件名称"));
    assert_eq!(app.project.content_baseline(), before);
    assert!(app.history.is_empty());
    let _ = fs::remove_dir_all(app.project.root);
}

#[test]
fn reconciliation_preserves_all_five_layouts_and_seventeen_palette_modes() {
    use crate::theme::{PaletteId, PaletteModeSupport, StylePreset, ThemeMode};
    let (ctx, mut app) = fixture();
    let baseline = app.project.content_baseline();
    let size = egui::vec2(800.0, 600.0);
    for style in [
        StylePreset::Studio,
        StylePreset::Manuscript,
        StylePreset::Technical,
        StylePreset::Focus,
        StylePreset::Ledger,
    ] {
        let mut count = 0;
        for palette in PaletteId::ALL {
            let modes = match palette.mode_support() {
                PaletteModeSupport::Both => vec![ThemeMode::Light, ThemeMode::Dark],
                PaletteModeSupport::LightOnly => vec![ThemeMode::Light],
                PaletteModeSupport::DarkOnly => vec![ThemeMode::Dark],
            };
            for mode in modes {
                app.personal.settings.appearance.style = style;
                app.personal.settings.appearance.palette = palette;
                app.personal.settings.appearance.theme = mode;
                let output = settle(&ctx, &mut app, size);
                assert!(visible(
                    &output,
                    "关闭并保留",
                    Rect::from_min_size(egui::Pos2::ZERO, size)
                ));
                assert_eq!(app.project.content_baseline(), baseline);
                count += 1;
            }
        }
        assert_eq!(count, 17);
    }
    let _ = fs::remove_dir_all(app.project.root);
}

fn wait_for_slot() {
    let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !jobs::Job::available() && std::time::Instant::now() < until {
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert!(jobs::Job::available(), "上一个后台任务没有释放槽位");
}

#[test]
fn delayed_prepared_adoption_rechecks_new_unapplied_form_and_retains_everything() {
    let _lock = JOB_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    wait_for_slot();
    let (ctx, mut app) = fixture();
    let session = app.conflict_view.session.clone().unwrap();
    app.conflict_view.drafts.insert(
        "world.wl".into(),
        Draft {
            choice: Some(ReconciliationChoice::Disk),
            manual: "保存的手工备选".into(),
        },
    );
    app.conflict_view.preview = Some(
        app.project
            .preview_reconciliation(&session, &app.conflict_view.request())
            .unwrap(),
    );
    let before = app.project.content_baseline();
    app.conflict_view.start_job(&app.project, true, &ctx);
    assert!(
        app.conflict_view.job.is_some(),
        "{:?}",
        app.conflict_view.error
    );
    app.new_file = Some("后台期间新输入".into());
    let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while app.conflict_view.job.is_some() && std::time::Instant::now() < until {
        frame(&ctx, &mut app, egui::vec2(800.0, 600.0), vec![]);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert!(app.conflict_view.job.is_none());
    assert!(app
        .conflict_view
        .error
        .as_ref()
        .is_some_and(|error| error.contains("未应用输入")));
    assert_eq!(app.project.content_baseline(), before);
    assert_eq!(app.new_file.as_deref(), Some("后台期间新输入"));
    assert_eq!(
        app.conflict_view.drafts[&PathBuf::from("world.wl")].manual,
        "保存的手工备选"
    );
    assert!(app.history.is_empty());
    wait_for_slot();
    let _ = fs::remove_dir_all(app.project.root);
}

#[test]
fn retained_manual_candidates_are_drafts_until_explicit_discard_even_if_project_clean() {
    let (ctx, mut app) = fixture();
    app.conflict_view.drafts.insert(
        "world.wl".into(),
        Draft {
            choice: None,
            manual: "保留的手工稿".into(),
        },
    );
    app.conflict_view.close(&ctx);
    app.project.mark_saved();
    assert!(!app.project.is_dirty());
    assert!(app.conflict_view.has_unsubmitted_work());
    assert!(app.dirty_draft_names().contains(&DRAFT_KIND));
    app.request_action(super::super::Pending::Close, &ctx);
    assert!(app.draft_action.is_some());
    assert!(!app.allow_close);
    app.conflict_view.resume_draft();
    assert_eq!(
        app.conflict_view.drafts[&PathBuf::from("world.wl")].manual,
        "保留的手工稿"
    );
    app.discard_authoring_drafts();
    assert!(!app.conflict_view.has_unsubmitted_work());
    assert!(!app.conflict_view.open);
    let _ = fs::remove_dir_all(app.project.root);
}

#[test]
fn adoption_ignores_only_its_own_draft_category_and_keeps_other_guards() {
    let _lock = JOB_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    wait_for_slot();
    let (ctx, mut app) = fixture();
    let session = app.conflict_view.session.clone().unwrap();
    app.conflict_view.drafts.insert(
        "world.wl".into(),
        Draft {
            choice: Some(ReconciliationChoice::Disk),
            manual: String::new(),
        },
    );
    app.conflict_view.preview = Some(
        app.project
            .preview_reconciliation(&session, &app.conflict_view.request())
            .unwrap(),
    );
    app.frame_dirty_drafts = vec![DRAFT_KIND];
    app.conflict_view.start_job(&app.project, true, &ctx);
    assert!(
        app.conflict_view.job.is_some(),
        "{:?}",
        app.conflict_view.error
    );
    let until = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while app.conflict_view.job.is_some() && std::time::Instant::now() < until {
        frame(&ctx, &mut app, egui::vec2(800.0, 600.0), vec![]);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert!(
        app.conflict_view.error.is_none(),
        "{:?}",
        app.conflict_view.error
    );
    assert!(app
        .project
        .document(&app.active_file)
        .unwrap()
        .contains("磁盘风"));
    assert_eq!(app.history.len(), 1);
    assert!(!app.conflict_view.has_unsubmitted_work());
    wait_for_slot();
    let _ = fs::remove_dir_all(app.project.root);
}

#[test]
fn ime_preedit_blocks_later_escape_until_commit_or_disabled_and_never_loses_manual_text() {
    let (ctx, mut app) = fixture();
    app.conflict_view.drafts.insert(
        "world.wl".into(),
        Draft {
            choice: Some(ReconciliationChoice::Manual {
                text: String::new(),
            }),
            manual: "保留中文候选".into(),
        },
    );
    let size = egui::vec2(800.0, 600.0);
    settle(&ctx, &mut app, size);
    let manual_id = egui::Id::new((
        "reconciliation-manual",
        app.project.root.as_path(),
        &PathBuf::from("world.wl"),
    ));
    ctx.memory_mut(|memory| memory.request_focus(manual_id));
    frame(&ctx, &mut app, size, vec![]);
    frame(
        &ctx,
        &mut app,
        size,
        vec![Event::Ime(egui::ImeEvent::Enabled)],
    );
    frame(
        &ctx,
        &mut app,
        size,
        vec![Event::Ime(egui::ImeEvent::Preedit("候选中".into()))],
    );
    // 本帧没有任何Ime事件；保护必须来自上一帧保留的组合状态。
    frame(&ctx, &mut app, size, vec![key(Key::Escape)]);
    assert!(app.conflict_view.open);
    assert!(app.conflict_view.composition.blocks_actions());
    let release = Event::Key {
        key: Key::Escape,
        physical_key: Some(Key::Escape),
        pressed: false,
        repeat: false,
        modifiers: Modifiers::NONE,
    };
    frame(&ctx, &mut app, size, vec![release.clone()]);
    frame(
        &ctx,
        &mut app,
        size,
        vec![
            Event::Ime(egui::ImeEvent::Commit("候选中".into())),
            key(Key::Escape),
        ],
    );
    assert!(app.conflict_view.open, "Commit同帧的Escape不能成为关闭");
    assert!(
        app.conflict_view.drafts[&PathBuf::from("world.wl")]
            .manual
            .contains("候选中"),
        "真实TextEdit必须接收IME Commit"
    );
    frame(
        &ctx,
        &mut app,
        size,
        vec![release.clone(), Event::Ime(egui::ImeEvent::Disabled)],
    );
    frame(&ctx, &mut app, size, vec![key(Key::Escape)]);
    assert!(!app.conflict_view.open);
    assert!(app.conflict_view.drafts[&PathBuf::from("world.wl")]
        .manual
        .contains("保留中文候选"));
    app.conflict_view.resume_draft();
    frame(&ctx, &mut app, size, vec![release]);
    frame(
        &ctx,
        &mut app,
        size,
        vec![Event::Ime(egui::ImeEvent::Preedit("取消组合".into()))],
    );
    frame(&ctx, &mut app, size, vec![key(Key::Escape)]);
    assert!(app.conflict_view.open);
    frame(
        &ctx,
        &mut app,
        size,
        vec![Event::Ime(egui::ImeEvent::Disabled)],
    );
    assert!(app.conflict_view.open);
    let _ = fs::remove_dir_all(app.project.root);
}
