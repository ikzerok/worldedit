//! 重开来源的当前 frame/key/B 身份保护；不会将旧字段输入投向新 Buffer。
use super::*;
const RELOAD: &str = "载入已应用原文，保留对白字段";

fn stale(source: bool) -> Workbench {
    let mut app = Workbench::new(mixed());
    app.click("逐句对白");
    app.click("编辑此句");
    app.frame(vec![Event::Text("必须保留的原字段".into())]);
    assert!(app.view.has_dialogue_input());
    if source {
        app.click("源码");
    }
    let path = app.project.entry.clone();
    let current = format!(
        "{}\n// 较新已应用原文\n",
        app.project.document(&path).unwrap()
    );
    app.project.set_text(&path, current).unwrap();
    app.view.invalidate_projection();
    assert!(!app.buffer.is_changed());
    assert_ne!(app.buffer.baseline(), app.project.content_baseline());
    app
}

#[test]
fn dialogue_reload_clean_buffer_keeps_exact_readonly_fields_and_requires_rebind() {
    let mut app = stale(false);
    let fields = app.retained();
    let baseline = app.project.content_baseline();
    app.frame(vec![Event::Text("只读标签不能接收这段文字".into())]);
    assert_eq!(app.retained(), fields);
    app.click(RELOAD);
    assert_eq!(app.buffer.baseline(), baseline);
    assert_eq!(
        app.buffer.source(),
        app.project.document(app.buffer.path()).unwrap()
    );
    assert!(!app.buffer.is_changed());
    assert_eq!(app.retained(), fields, "不得自动重绑旧请求");
    let output = app.frame(vec![]);
    assert!(output
        .shapes
        .iter()
        .any(|shape| point(&shape.shape, "绑定到所选当前位置并重新核对").is_some()));
    assert_eq!(app.project.content_baseline(), baseline);
}

#[test]
fn dialogue_reload_same_frame_source_text_or_paste_rejects_replacement() {
    for event in [
        Event::Text("旧B最后输入".into()),
        Event::Paste("旧B最后输入".into()),
    ] {
        let mut app = stale(true);
        app.focus_body();
        let fields = app.retained();
        let old_baseline = app.buffer.baseline().to_owned();
        let project_baseline = app.project.content_baseline();
        app.batch(RELOAD, event);
        assert_eq!(app.buffer.source().matches("旧B最后输入").count(), 1);
        assert!(app.buffer.is_changed());
        assert_eq!(app.buffer.baseline(), old_baseline);
        assert!(!app.buffer.source().contains("较新已应用原文"));
        assert_eq!(app.retained(), fields);
        assert!(app
            .error
            .as_deref()
            .is_some_and(|error| error.contains("本帧正文输入已变化")));
        assert_eq!(app.project.content_baseline(), project_baseline);
    }
}

#[test]
fn dialogue_reload_form_ime_commit_is_rescued_and_never_reloads_source() {
    let mut app = stale(false);
    let identity = app.buffer.identity();
    app.frame(vec![Event::Ime(ImeEvent::Preedit("待提交".into()))]);
    app.batch(
        RELOAD,
        Event::Ime(ImeEvent::Commit("原字段提交保留".into())),
    );
    assert!(app.retained().contains("原字段提交保留"));
    assert_eq!(app.buffer.identity(), identity);
    assert!(app.view.mode_request.is_none());
}

#[test]
fn dialogue_reload_unavailable_core_source_and_readonly_keep_buffer_and_fields() {
    for readonly in [false, true] {
        let mut app = stale(true);
        if readonly {
            app.enabled = false;
        } else {
            // 实际磁盘删除并 refresh，不能以绕过 setter 的非法对象制造失败。
            app.project.save().unwrap();
            std::fs::remove_file(app.buffer.path()).unwrap();
            app.project.refresh().unwrap();
            assert!(app.project.document(app.buffer.path()).is_err());
        }
        let identity = app.buffer.identity();
        let fields = app.retained();
        let pos = app.point(RELOAD);
        let mut events = pointer(pos, true);
        events.extend(pointer(pos, false));
        app.frame(events);
        assert_eq!(app.buffer.identity(), identity);
        assert_eq!(app.retained(), fields);
        if !readonly {
            assert!(app.error.is_some());
        }
        assert!(app.view.mode_request.is_none());
    }
}

#[test]
fn dialogue_reload_late_wrong_target_and_changed_identity_requests_are_discarded() {
    for invalidation in ["late", "target", "identity"] {
        let mut app = stale(true);
        let fields = app.retained();
        let ctx = app.ctx.clone();
        let mut expected = app.buffer.identity();
        let _ = ctx.run(Default::default(), |ctx| {
            app.view.request_reload(ctx, &app.buffer, &app.target);
            if invalidation == "identity" {
                app.buffer = app
                    .project
                    .open_source_writing_buffer(app.buffer.path())
                    .unwrap();
                expected = app.buffer.identity();
            }
            if invalidation != "late" {
                let target = if invalidation == "target" {
                    TargetRef::new("event", "other")
                } else {
                    app.target.clone()
                };
                app.view.finish_toolbar_request(
                    ctx,
                    &app.project,
                    &mut app.buffer,
                    &target,
                    &mut Action::default(),
                );
            }
        });
        if invalidation == "late" {
            app.frame(vec![]);
        }
        assert!(app.view.mode_request.is_none());
        assert_eq!(app.buffer.identity(), expected);
        assert_eq!(app.retained(), fields);
    }
}
