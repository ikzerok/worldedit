use super::navigation::{
    assert_source_selection, back, click_label, included_fixture, navigation_frame, observed,
    unchanged_state,
};
use super::*;

fn open_source(ctx: &egui::Context, app: &mut WorldeditApp, metadata: &LocalizedPresentation) {
    app.open_localized_item(
        ctx,
        Navigation::Entry {
            presentation: metadata.clone(),
            translation: false,
        },
        "zh-Hant",
        false,
    );
}

fn rejected_without_navigation(
    ctx: &egui::Context,
    app: &mut WorldeditApp,
    metadata: &LocalizedPresentation,
    reason: &str,
) {
    let before = unchanged_state(app);
    let location = app.author_location(Some(ctx));
    let history = app.personal.history.len();
    app.message = None;
    open_source(ctx, app, metadata);
    assert_eq!(app.tab, Tab::Play);
    assert!(app.author_location(Some(ctx)) == location);
    assert_eq!(app.personal.history.len(), history);
    assert!(
        app.message.as_deref().unwrap().contains(reason),
        "{:?}",
        app.message
    );
    assert_eq!(unchanged_state(app), before);
}

#[test]
fn editor_locale_source_rejects_unapplied_changed_ime_and_external_sources_without_losing_input() {
    for reason in ["draft", "changed", "ime", "external"] {
        let (ctx, mut app) = included_fixture(false);
        let metadata = observed(&app)
            .into_iter()
            .find(|p| p.id.as_deref() == Some("body"))
            .unwrap();
        let path = app.project.root.join("chapters/body.wl");
        let source = app.project.document(&path).unwrap().to_owned();
        let diagnostic = match reason {
            "draft" => {
                let mut buffer = app.project.open_source_writing_buffer(&path).unwrap();
                buffer.replace_source(format!("{source}// 保留未应用正文\n"));
                app.manuscript.restore_writing_buffers(&[buffer]);
                "未应用"
            }
            "changed" => {
                app.project.set_text(&path, format!("\n{source}")).unwrap();
                app.recompile();
                "过期"
            }
            "ime" => {
                app.ime_composing = true;
                "输入法"
            }
            "external" => {
                std::fs::write(&path, format!("{source}// 外部写入\n")).unwrap();
                "来源"
            }
            _ => unreachable!(),
        };
        let buffers = app
            .manuscript
            .writing_buffers()
            .iter()
            .map(|buffer| {
                (
                    buffer.path().to_owned(),
                    buffer.source().to_owned(),
                    buffer.generation(),
                )
            })
            .collect::<Vec<_>>();
        let disk = std::fs::read(&path).unwrap();
        for _ in 0..2 {
            rejected_without_navigation(&ctx, &mut app, &metadata, diagnostic);
            assert_eq!(
                app.manuscript
                    .writing_buffers()
                    .iter()
                    .map(|buffer| (
                        buffer.path().to_owned(),
                        buffer.source().to_owned(),
                        buffer.generation()
                    ))
                    .collect::<Vec<_>>(),
                buffers
            );
            assert_eq!(std::fs::read(&path).unwrap(), disk);
        }
        std::fs::remove_dir_all(&app.project.root).unwrap();
    }
}

#[test]
fn editor_locale_source_rejects_unobserved_paths_revisions_and_obsolete_choices() {
    let (ctx, mut app) = included_fixture(false);
    let actual = observed(&app);
    let metadata = actual
        .iter()
        .find(|p| p.id.as_deref() == Some("body"))
        .unwrap();
    for alteration in 0..7 {
        let mut forged = metadata.clone();
        match alteration {
            0 => forged.source.file = "../outside.wl".into(),
            1 => {
                forged.source.file = app
                    .project
                    .root
                    .join("chapters/body.wl")
                    .to_string_lossy()
                    .into()
            }
            2 => forged.source.line = 1,
            3 => forged.source.kind = "choice".into(),
            4 => forged.id = Some("root_body".into()),
            5 => forged.source_revision.push_str("-changed"),
            6 => forged.source_baseline.push_str("-changed"),
            _ => unreachable!(),
        }
        rejected_without_navigation(&ctx, &mut app, &forged, "不属于本次");
    }
    let old_choice = actual
        .into_iter()
        .find(|p| p.source.kind == "choice")
        .unwrap();
    app.play
        .as_mut()
        .unwrap()
        .story
        .as_mut()
        .unwrap()
        .choose_presentation(0)
        .unwrap();
    frame(&ctx, &mut app);
    assert!(app.play.as_ref().unwrap().ended);
    rejected_without_navigation(&ctx, &mut app, &old_choice, "不属于本次");
    std::fs::remove_dir_all(&app.project.root).unwrap();
}

#[test]
fn editor_locale_source_wrong_locale_is_visible_and_source_only_restart_cannot_reuse_metadata() {
    let (ctx, mut app) = included_fixture(false);
    let metadata = observed(&app)[0].clone();
    let before = unchanged_state(&app);
    app.open_localized_item(
        &ctx,
        Navigation::Entry {
            presentation: metadata.clone(),
            translation: false,
        },
        "fr",
        false,
    );
    assert_eq!(app.tab, Tab::Play);
    assert!(app.message.as_deref().unwrap().contains("语言"));
    assert_eq!(unchanged_state(&app), before);
    app.replay_debugger.locale.enabled = false;
    app.start_play();
    frame(&ctx, &mut app);
    rejected_without_navigation(&ctx, &mut app, &metadata, "语言");
    std::fs::remove_dir_all(&app.project.root).unwrap();
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn editor_locale_draft_source_still_uses_worker_guard_and_keeps_same_isolated_locale() {
    let (ctx, mut app) = included_fixture(false);
    let regular = unchanged_state(&app);
    let metadata = observed(&app)
        .into_iter()
        .find(|p| p.id.as_deref() == Some("go"))
        .unwrap();
    let path = app.project.entry.clone();
    let mut buffer = app.project.open_source_writing_buffer(&path).unwrap();
    buffer.replace_source(format!("{}// 尚未应用的草稿\n", buffer.source()));
    app.manuscript.restore_writing_buffers(&[buffer.clone()]);
    app.request_draft_rehearsal(&ctx);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    loop {
        let output = navigation_frame(&ctx, &mut app, vec![]);
        let text = rendered(&output);
        if text.contains("纳入未应用正文") {
            break;
        }
        assert!(std::time::Instant::now() < deadline, "{text}");
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    click_label(&ctx, &mut app, "明确开始这份草稿试演");
    let initial = loop {
        let output = navigation_frame(&ctx, &mut app, vec![]);
        let text = rendered(&output);
        if text.contains("译文 · Included go") {
            break text;
        }
        assert!(std::time::Instant::now() < deadline, "{text}");
        std::thread::sleep(std::time::Duration::from_millis(2));
    };
    assert!(app.verify_rehearsal_localization_navigation());
    assert!(app.draft_rehearsal.active);
    // Real draft button must bypass the ordinary-play guard, which rejects this open draft.
    app.replay_debugger.locale.enabled = false;
    app.replay_debugger.locale.locale = "fr".into();
    click_label(&ctx, &mut app, "查看此源文");
    while app.tab == Tab::Play {
        navigation_frame(&ctx, &mut app, vec![]);
        assert!(std::time::Instant::now() < deadline, "{:?}", app.message);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert_source_selection(&ctx, &mut app, &metadata);
    back(&ctx, &mut app);
    assert!(app.draft_rehearsal.active);
    assert!(app.verify_rehearsal_localization_navigation());
    assert_eq!(unchanged_state(&app), regular);
    let returned = rendered(&navigation_frame(&ctx, &mut app, vec![]));
    for line in initial.lines().filter(|line| line.starts_with("译文 · ")) {
        assert!(
            returned.contains(line),
            "same evaluated locale text must survive return: {line}"
        );
    }
    buffer.replace_source(format!("{}// 更晚的未应用输入\n", buffer.source()));
    app.manuscript.restore_writing_buffers(&[buffer.clone()]);
    assert!(!app.verify_rehearsal_localization_navigation());
    let history = app.personal.history.len();
    app.open_localized_item(
        &ctx,
        Navigation::Entry {
            presentation: metadata,
            translation: false,
        },
        "zh-Hant",
        true,
    );
    assert_eq!(app.tab, Tab::Play);
    assert_eq!(app.personal.history.len(), history);
    assert!(app
        .message
        .as_deref()
        .unwrap()
        .contains("草稿或工作区已变化"));
    assert!(app.draft_rehearsal.has_session());
    assert_eq!(
        app.manuscript.writing_buffers()[0].source(),
        buffer.source()
    );
    assert_eq!(unchanged_state(&app), regular);
    std::fs::remove_dir_all(&app.project.root).unwrap();
}
