use super::navigation::unchanged_state;
use super::*;

#[test]
fn editor_locale_failed_start_notice_clears_only_after_new_real_session_succeeds() {
    let (ctx, mut app) = fixture();
    let path = app.project.root.join(".world/localization/zh-Hant.json");
    let valid = app
        .project
        .authoring_document(&path)
        .unwrap()
        .bytes()
        .to_vec();
    let mut invalid: serde_json::Value = serde_json::from_slice(&valid).unwrap();
    invalid["entries"]["body"]["translation_parts"] = serde_json::json!([
        {"type":"text", "text":"故意缺少 p0/p1/l0 的译文"}
    ]);
    app.project
        .set_authoring_document(&path, serde_json::to_vec(&invalid).unwrap())
        .unwrap();
    app.project.save().unwrap();
    app.recompile();
    app.start_play();
    assert!(app.play.is_none());
    let failed = app.replay_debugger.notice.clone().unwrap();
    assert!(failed.contains("InvalidTranslation"), "{failed}");
    assert!(rendered(&frame(&ctx, &mut app)).contains(&failed));

    // Merely changing the next-run policy or browsing must not erase the actual failed attempt.
    app.replay_debugger.locale.fallback = true;
    frame(&ctx, &mut app);
    assert_eq!(app.replay_debugger.notice.as_deref(), Some(failed.as_str()));
    app.start_play();
    frame(&ctx, &mut app);
    assert!(app.replay_debugger.notice.is_none());
    let output = app.play.as_ref().unwrap().localized_outputs[0]
        .localization
        .as_ref()
        .unwrap();
    assert_eq!(output.status, LocalizationStatus::InvalidTranslation);
    assert_eq!(
        app.play
            .as_ref()
            .unwrap()
            .story
            .as_ref()
            .unwrap()
            .presentation_identity()
            .unwrap()
            .request
            .policy,
        LocalizationPresentationPolicy::SourceFallback
    );
    assert!(!rendered(&frame(&ctx, &mut app)).contains(&failed));

    // Restoring/saving the real translation still does not silently restart or change its identity.
    let old = app
        .play
        .as_ref()
        .unwrap()
        .story
        .as_ref()
        .unwrap()
        .save()
        .unwrap();
    app.project.set_authoring_document(&path, valid).unwrap();
    app.project.save().unwrap();
    app.recompile();
    app.replay_debugger.locale.fallback = false;
    assert_eq!(
        app.play
            .as_ref()
            .unwrap()
            .story
            .as_ref()
            .unwrap()
            .save()
            .unwrap(),
        old
    );
    app.start_play();
    frame(&ctx, &mut app);
    assert!(app.replay_debugger.notice.is_none());
    assert_eq!(
        app.play.as_ref().unwrap().localized_outputs[0]
            .localization
            .as_ref()
            .unwrap()
            .status,
        LocalizationStatus::Translated
    );
    assert_eq!(
        app.play
            .as_ref()
            .unwrap()
            .story
            .as_ref()
            .unwrap()
            .presentation_identity()
            .unwrap()
            .request
            .policy,
        LocalizationPresentationPolicy::Strict
    );
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
    assert!(app.replay_debugger.notice.is_none());
    assert!(!rendered(&frame(&ctx, &mut app)).contains(&failed));

    let established = unchanged_state(&app);
    app.replay_debugger.locale.locale = "fr".into();
    app.replay_debugger.locale.parallel = true;
    frame(&ctx, &mut app);
    assert!(app.replay_debugger.notice.is_none());
    assert_eq!(unchanged_state(&app), established);
    let request = app.replay_debugger.locale.request().unwrap();
    let error = app
        .project
        .prepare_localization_presentation(&request)
        .unwrap_err();
    assert_eq!(error.code, "UNKNOWN_LOCALE");
    app.start_play();
    let failed_again = app.replay_debugger.notice.clone().unwrap();
    assert_eq!(failed_again, error.to_string());
    assert_eq!(failed_again, "工程没有登记此 locale");
    assert_ne!(failed_again, failed);
    assert_eq!(
        unchanged_state(&app),
        established,
        "a failed new attempt keeps the established Story and trace"
    );
    frame(&ctx, &mut app);
    assert_eq!(app.replay_debugger.notice.as_ref(), Some(&failed_again));
    app.replay_debugger.locale.enabled = false;
    frame(&ctx, &mut app);
    assert_eq!(app.replay_debugger.notice.as_ref(), Some(&failed_again));
    app.start_play();
    frame(&ctx, &mut app);
    assert!(app.replay_debugger.notice.is_none());
    assert!(app
        .play
        .as_ref()
        .unwrap()
        .story
        .as_ref()
        .unwrap()
        .presentation_identity()
        .is_none());
    std::fs::remove_dir_all(&app.project.root).unwrap();
}
