use super::*;

#[test]
fn form_state_keyboard_production_empty_error_valid_invalidated_and_cancel() {
    let _serial = serial();
    let mut h = Keyboard::new(3, false, false);
    let initial = h.author_state();
    let disk = h.flow.disk();
    assert!(h.flow.app.manuscript.production.snapshot.is_none());
    assert!(h.flow.app.manuscript.production.artifact.is_none());
    h.tab_field(ROLE_HINT);
    h.type_value("a");
    h.tab_field(LOCALE_HINT);
    h.type_value("unknown-locale");
    h.generate_keyboard();
    assert!(h.flow.app.manuscript.production.snapshot.is_none());
    assert!(h.flow.app.manuscript.production.artifact.is_none());
    assert!(h.flow.notice().contains("UNKNOWN_LOCALE"));
    let notice = h.flow.notice().to_owned();
    let mut out = h.tab_to("角色台本阅读区 · ↑↓滚动");
    let reader = h.response.as_ref().unwrap().id;
    let mut seen = visible(&out, &notice).is_some();
    for _ in 0..80 {
        if seen {
            break;
        }
        out = h.key(Key::ArrowDown, Modifiers::NONE);
        assert_eq!(h.flow.ctx.memory(|m| m.focused()), Some(reader));
        if visible(&out, &notice).is_some() {
            seen = true;
            break;
        }
    }
    assert!(
        seen,
        "existing production reader must expose exact error: {notice}"
    );
    let out = h.frame(vec![]);
    assert!(
        visible(&out, &notice).is_some(),
        "notice must remain readable on idle"
    );
    h.tab_field(LOCALE_HINT);
    h.type_value("");
    assert!(h.flow.app.manuscript.production.locale.is_empty());
    h.generate_keyboard();
    assert!(h.flow.app.production_is_current());
    let snapshot = h.flow.snapshot();
    h.enter("准备私密交付");
    h.enter("预览完整交付字节");
    let bytes = h.flow.artifact();
    h.enter(CONFIRM);
    assert!(h.flow.app.manuscript.production.confirmed);
    assert!(h.flow.app.checked_production_artifact(&h.flow.ctx).is_ok());
    h.enter(DIRECTION);
    assert!(!h.flow.app.manuscript.production.confirmed);
    assert!(h.flow.app.checked_production_artifact(&h.flow.ctx).is_err());
    assert_eq!(
        h.flow.artifact(),
        bytes,
        "old reviewed bytes remain available but not deliverable"
    );
    h.enter("预览完整交付字节");
    assert_eq!(
        h.flow.artifact(),
        snapshot
            .export(&ProductionExportOptions {
                schema_version: 1,
                format: ProductionFormat::Json,
                include_direction: true
            })
            .unwrap()
            .bytes()
    );
    h.enter(CONFIRM);
    assert!(h.flow.app.manuscript.production.confirmed);
    h.enter(CONFIRM);
    assert!(!h.flow.app.manuscript.production.confirmed);
    h.enter("准备私密交付");
    assert!(!h.flow.app.manuscript.production.export_open);
    h.enter("返回正文");
    assert!(!h.flow.app.manuscript.production.open);
    let after = h.author_state();
    for key in [
        "baseline", "sources", "fields", "history", "version", "dirty",
    ] {
        assert_eq!(after[key], initial[key]);
    }
    for (path, identity) in initial["buffers"].as_object().unwrap() {
        assert_eq!(&after["buffers"][path], identity);
    }
    assert_eq!(h.flow.disk(), disk);
}
