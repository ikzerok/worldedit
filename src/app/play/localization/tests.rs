use super::*;
use crate::app::SavedReplayPath;
use worldline_core::localization::{LocalizationPart, LocalizationSelection, LocalizationStatus};
use worldline_runtime::{ReplayBudget, ReplayCancellation};

const SOURCE: &str = concat!(
    "let n = 0\nevent start\n",
    "  Hello {rnd(1, 1000)} / {rnd(1, 1000)} [[event:start|Harbor]] #wl-localization:body\n",
    "  choice \"Continue\" #wl-localization:go\n    set n = 7\n    -> END\n",
);
fn fixture() -> (egui::Context, WorldeditApp) {
    let ctx = egui::Context::default();
    let mut app = WorldeditApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()), None);
    let root = std::env::temp_dir().join(format!(
        "editor-locale-runtime-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    app.project = Project::new(&root);
    let entry = app.project.entry.clone();
    app.project.set_text(&entry, SOURCE.into()).unwrap();
    app.project
        .create_authoring_document(
            &root.join(".world/project.json"),
            br#"{
        "schema_version":1,"language_version":"1.10","required_features":["content.localization.v1"]
    }"#
            .to_vec(),
        )
        .unwrap();
    let selection = LocalizationSelection {
        schema_version: 1,
        source_locale: "en".into(),
        target_locale: "zh-Hant".into(),
        string_ids: vec!["body".into(), "go".into()],
    };
    let mut exchange = app
        .project
        .preview_localization_export(&selection)
        .unwrap()
        .exchange;
    for entry in &mut exchange.entries {
        entry.translation_parts = Some(if entry.id == "body" {
            vec![
                LocalizationPart::Link {
                    token: "l0".into(),
                    label: "港口🌙".into(),
                },
                LocalizationPart::Text {
                    text: " 译文 ".into(),
                },
                LocalizationPart::Placeholder { token: "p1".into() },
                LocalizationPart::Text { text: " / ".into() },
                LocalizationPart::Placeholder { token: "p0".into() },
            ]
        } else {
            vec![LocalizationPart::Text {
                text: "继续 👋".into(),
            }]
        });
    }
    let plan = app
        .project
        .preview_localization_import_candidate(&selection, &exchange)
        .unwrap();
    assert!(plan.can_apply, "{:?}", plan.diagnostics);
    app.project
        .apply_localization_import_candidate(&selection, &exchange, &plan.plan_digest)
        .unwrap();
    app.project.save().unwrap();
    app.reset_views();
    app.recompile();
    app.tab = Tab::Play;
    app.replay_debugger.seed = 314159;
    app.replay_debugger.locale.enabled = true;
    app.replay_debugger.locale.locale = "zh-Hant".into();
    (ctx, app)
}
fn frame(ctx: &egui::Context, app: &mut WorldeditApp) -> egui::FullOutput {
    ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1500.0, 1000.0),
            )),
            ..Default::default()
        },
        |ctx| app.play_tab(ctx),
    )
}
fn text(shape: &egui::Shape, into: &mut String) {
    match shape {
        egui::Shape::Text(t) => {
            into.push_str(t.galley.text());
            into.push('\n');
        }
        egui::Shape::Vec(shapes) => {
            for s in shapes {
                text(s, into);
            }
        }
        _ => {}
    }
}
fn rendered(output: &egui::FullOutput) -> String {
    let mut result = String::new();
    for shape in &output.shapes {
        text(&shape.shape, &mut result);
    }
    result
}

#[test]
fn editor_locale_real_story_parallel_display_preserves_rng_and_stable_choices() {
    let (ctx, mut app) = fixture();
    app.start_play();
    frame(&ctx, &mut app);
    let play = app.play.as_ref().unwrap();
    assert!(play.error.is_none(), "{:?}", play.error);
    assert_eq!(play.localized_outputs.len(), 1);
    let output = &play.localized_outputs[0];
    let metadata = output.localization.as_ref().unwrap();
    assert_eq!(metadata.id.as_deref(), Some("body"));
    assert_eq!(metadata.status, LocalizationStatus::Translated);
    assert_eq!(metadata.source.line, 3);
    assert_eq!(
        &output.content[output.links[0].start..output.links[0].end],
        "港口🌙"
    );
    assert!(metadata.source_content.starts_with("Hello "));
    let before = play.story.as_ref().unwrap().save().unwrap();
    app.replay_debugger.locale.parallel = true;
    let display = rendered(&frame(&ctx, &mut app));
    assert!(display.contains("源文 · 同次求值"), "{display}");
    assert!(display.contains("港口🌙"), "{display}");
    assert_eq!(
        app.play
            .as_ref()
            .unwrap()
            .story
            .as_ref()
            .unwrap()
            .save()
            .unwrap(),
        before
    );
    let snapshot = app.snapshot.as_ref().unwrap();
    let mut source = worldline_runtime::OwnedStory::new_with_seed(
        snapshot.result.program.clone(),
        snapshot.result.analysis.clone(),
        314159,
    )
    .unwrap();
    source
        .continue_story_bounded(ReplayBudget::new(100_000, 250), &ReplayCancellation::new())
        .unwrap();
    let translated = app.play.as_ref().unwrap().story.as_ref().unwrap();
    assert_eq!(source.state_view(), translated.state_view());
    let a: serde_json::Value = serde_json::from_str(&source.save().unwrap()).unwrap();
    let b: serde_json::Value = serde_json::from_str(&translated.save().unwrap()).unwrap();
    assert_eq!(a["rng"], b["rng"]);
    assert_eq!(
        source.choice_presentations()[0].id,
        translated.choice_presentations()[0].id
    );
    assert_eq!(translated.choice_presentations()[0].label, "继续 👋");
    std::fs::remove_dir_all(&app.project.root).unwrap();
}

#[test]
fn editor_locale_exact_translation_navigation_retains_story_and_restarts_after_revision() {
    let (ctx, mut app) = fixture();
    app.start_play();
    frame(&ctx, &mut app);
    let metadata = app.play.as_ref().unwrap().localized_outputs[0]
        .localization
        .clone()
        .unwrap();
    let before = app
        .play
        .as_ref()
        .unwrap()
        .story
        .as_ref()
        .unwrap()
        .save()
        .unwrap();
    app.open_localized_item(
        &ctx,
        Navigation::Entry {
            presentation: metadata.clone(),
            translation: true,
        },
        "zh-Hant",
        false,
    );
    assert_eq!(app.tab, Tab::Localization);
    assert_eq!(app.localization_ui.target_locale, "zh-Hant");
    assert_eq!(
        app.play
            .as_ref()
            .unwrap()
            .story
            .as_ref()
            .unwrap()
            .save()
            .unwrap(),
        before
    );
    app.tab = Tab::Play;
    let entry = app.project.entry.clone();
    app.project
        .set_text(&entry, SOURCE.replace("Hello ", "Changed "))
        .unwrap();
    app.recompile();
    app.open_localized_item(
        &ctx,
        Navigation::Entry {
            presentation: metadata,
            translation: true,
        },
        "zh-Hant",
        false,
    );
    assert_eq!(app.tab, Tab::Play);
    assert!(app.message.as_ref().unwrap().contains("过期"));
    app.start_play();
    assert!(app
        .replay_debugger
        .notice
        .as_ref()
        .is_some_and(|n| !n.is_empty()));
    assert_eq!(
        app.play
            .as_ref()
            .unwrap()
            .story
            .as_ref()
            .unwrap()
            .save()
            .unwrap(),
        before,
        "failed strict restart must preserve old story"
    );
    app.replay_debugger.locale.fallback = true;
    app.start_play();
    frame(&ctx, &mut app);
    let metadata = app.play.as_ref().unwrap().localized_outputs[0]
        .localization
        .as_ref()
        .unwrap();
    assert_eq!(metadata.status, LocalizationStatus::StaleSource);
    assert!(app.play.as_ref().unwrap().transcript.contains("Changed "));
    std::fs::remove_dir_all(&app.project.root).unwrap();
}

#[test]
fn editor_locale_replay_uses_recorded_identity_and_rejects_new_translation() {
    let (ctx, mut app) = fixture();
    app.start_play();
    frame(&ctx, &mut app);
    let trace = app
        .play
        .as_ref()
        .unwrap()
        .story
        .as_ref()
        .unwrap()
        .replay_trace();
    assert!(trace.presentation.is_some());
    app.replay_debugger.saved_paths.push(SavedReplayPath {
        name: "中文路径".into(),
        trace: trace.clone(),
    });
    app.replay_debugger.selected_path = Some(0);
    app.replay_debugger.locale.enabled = false;
    app.begin_replay(&ctx);
    let end = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while app.replay_debugger.job.is_some() {
        app.poll_replay(&ctx);
        assert!(std::time::Instant::now() < end);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert!(
        app.replay_debugger.result.is_some(),
        "{:?}",
        app.replay_debugger.notice
    );
    let path = app.project.root.join(".world/localization/zh-Hant.json");
    let bytes = app.project.authoring_document(&path).unwrap().bytes();
    let replacement = String::from_utf8(bytes.to_vec())
        .unwrap()
        .replace("继续 👋", "另一份译文");
    app.project
        .set_authoring_document(&path, replacement.into_bytes())
        .unwrap();
    assert!(prepare_trace(&app.project, &trace)
        .unwrap_err()
        .contains("译文已变化"));
    std::fs::remove_dir_all(&app.project.root).unwrap();
}

#[test]
fn editor_locale_source_only_default_has_no_metadata_or_presentation_identity() {
    let (ctx, mut app) = fixture();
    app.replay_debugger.locale = LocaleUi::default();
    app.start_play();
    frame(&ctx, &mut app);
    let play = app.play.as_ref().unwrap();
    assert!(play.localized_outputs.is_empty());
    assert!(play
        .story
        .as_ref()
        .unwrap()
        .presentation_identity()
        .is_none());
    assert!(play.transcript.starts_with("Hello "));
    assert!(play
        .story
        .as_ref()
        .unwrap()
        .replay_trace()
        .presentation
        .is_none());
    std::fs::remove_dir_all(&app.project.root).unwrap();
}

#[test]
fn editor_locale_glue_empty_translation_and_utf8_links_are_one_reading_flow() {
    let (ctx, mut app) = fixture();
    app.start_play();
    frame(&ctx, &mut app);
    let original = app.play.as_ref().unwrap().localized_outputs[0].clone();
    let mut outputs = Vec::new();
    for (index, (target, source)) in [
        ("甲", "源甲"),
        ("", "源空"),
        ("港🌙", "Harbor"),
        ("尾", "源尾"),
    ]
    .into_iter()
    .enumerate()
    {
        let mut output = original.clone();
        output.content = target.into();
        output.new_line = index == 0;
        output.links.clear();
        let metadata = output.localization.as_mut().unwrap();
        metadata.source_content = source.into();
        metadata.source_links.clear();
        if index == 2 {
            let target_ref = worldline_core::TargetRef {
                kind: "event".into(),
                id: "start".into(),
            };
            output.links.push(worldline_core::navigation::RenderedLink {
                target: target_ref.clone(),
                start: 0,
                end: target.len(),
            });
            metadata
                .source_links
                .push(worldline_core::navigation::RenderedLink {
                    target: target_ref,
                    start: 0,
                    end: source.len(),
                });
        }
        outputs.push(output);
    }
    let (target, target_links) = transcript(&outputs, false);
    let (source, source_links) = transcript(&outputs, true);
    assert_eq!(target, "甲港🌙尾");
    assert_eq!(source, "源甲源空Harbor源尾");
    assert_eq!(&target[target_links[0].start..target_links[0].end], "港🌙");
    assert_eq!(
        &source[source_links[0].start..source_links[0].end],
        "Harbor"
    );
    let output = ctx.run(
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1400.0, 800.0),
            )),
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                super::outputs(ui, &outputs, false);
            });
        },
    );
    let prose = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(t) if t.galley.text() == "甲港🌙尾" => Some(t),
            _ => None,
        })
        .unwrap_or_else(|| panic!("{}", rendered(&output)));
    assert_eq!(
        prose.galley.rows.len(),
        1,
        "glued segments share one reading row"
    );
    let glyphs = &prose.galley.rows[0].glyphs;
    assert_eq!(glyphs.iter().map(|g| g.chr).collect::<String>(), "甲港🌙尾");
    assert!(glyphs.windows(2).all(|pair| pair[0].pos.x < pair[1].pos.x));
    std::fs::remove_dir_all(&app.project.root).unwrap();
}

#[test]
fn editor_locale_multiline_utf8_link_keeps_line_position_and_exact_click_target() {
    let ctx = egui::Context::default();
    let content = "第一行\n甲港🌙尾\n第三行";
    let start = "第一行\n甲".len();
    let target = worldline_core::TargetRef {
        kind: "event".into(),
        id: "start".into(),
    };
    let links = vec![worldline_core::navigation::RenderedLink {
        target: target.clone(),
        start,
        end: start + "港🌙".len(),
    }];
    let frame = |events: Vec<egui::Event>| {
        let mut selected = None;
        let output = ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(500.0, 400.0),
                )),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    selected = linked_text(ui, content, &links);
                });
            },
        );
        (output, selected)
    };
    frame(vec![]);
    let (output, _) = frame(vec![]);
    let prose = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(t) if t.galley.text() == content => Some(t),
            _ => None,
        })
        .unwrap_or_else(|| panic!("{}", rendered(&output)));
    assert_eq!(prose.galley.rows.len(), 3);
    assert_eq!(
        prose.galley.rows[1]
            .glyphs
            .iter()
            .map(|g| g.chr)
            .collect::<String>(),
        "甲港🌙尾"
    );
    let row = &prose.galley.rows[1];
    let glyph = row.glyphs.iter().find(|glyph| glyph.chr == '港').unwrap();
    let point = prose.pos + row.pos.to_vec2() + glyph.logical_rect().center().to_vec2();
    frame(vec![
        egui::Event::PointerMoved(point),
        egui::Event::PointerButton {
            pos: point,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        },
    ]);
    let (_, selected) = frame(vec![egui::Event::PointerButton {
        pos: point,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    }]);
    assert_eq!(selected, Some(target));
}

#[path = "navigation_tests.rs"]
mod navigation;
#[path = "navigation_guards_tests.rs"]
mod navigation_guards;

#[path = "short_viewport_tests.rs"]
mod short_viewport;

#[path = "startup_notice_tests.rs"]
mod startup_notice;

#[path = "keyboard_viewport_tests.rs"]
mod keyboard_viewport;
