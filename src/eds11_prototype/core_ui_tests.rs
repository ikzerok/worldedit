use super::tests::{click, click_last, frame};
use super::Prototype;

#[test]
fn core_buffers_runtime_and_review_remain_scoped_across_actual_project_switches() {
    let ctx = egui::Context::default();
    let mut app = Prototype::default();
    let size = [1024.0, 640.0];
    let baselines = [
        app.demos[0].project.content_baseline(),
        app.demos[1].project.content_baseline(),
    ];
    app.preview_position = 65.0;
    click(&ctx, &mut app, size, "应用一次（真实 core）");
    assert_eq!(app.demos[0].position(), 65.0);
    assert_ne!(app.demos[0].project.content_baseline(), baselines[0]);
    click(&ctx, &mut app, size, "J3 演练");
    click(&ctx, &mut app, size, "临时预览 E");
    assert!(app.demos[0].runtime.is_none(), "临时预览不得创建Story会话");
    click(&ctx, &mut app, size, "显式运行 E（真实 runtime）");
    assert_eq!(app.demos[0].runtime.as_ref().unwrap().start_visits, 1);
    click(&ctx, &mut app, size, "J4 审阅");
    let before_compare = app.demos[0].project.content_baseline();
    click(&ctx, &mut app, size, "重新比较（不采纳）");
    assert_eq!(app.demos[0].review.as_ref().unwrap().conflicts.len(), 1);
    assert_eq!(app.demos[0].project.content_baseline(), before_compare);
    click(&ctx, &mut app, size, "样例作品 1");
    click_last(&ctx, &mut app, size, "样例作品 2");
    assert_eq!(app.project, 1);
    assert_eq!(app.placement_position, 30.0);
    assert!(app.undo_position.is_none());
    assert!(!app.runtime_event && !app.preview_event);
    assert!(app.demos[1].runtime.is_none() && app.demos[1].review.is_none());
    assert_eq!(app.demos[1].project.content_baseline(), baselines[1]);
    click(&ctx, &mut app, size, "样例作品 2");
    click_last(&ctx, &mut app, size, "样例作品 1");
    assert_eq!(app.placement_position, 65.0);
    assert!(app.runtime_event);
    click(&ctx, &mut app, size, "J1 世界资料");
    click(&ctx, &mut app, size, "撤销一次（真实 core）");
    assert_eq!(app.demos[0].project.content_baseline(), baselines[0]);
    assert_eq!(app.demos[0].applied_commands, 2);
    assert_eq!(app.demos[1].applied_commands, 0);
    for demo in &app.demos {
        assert!(!demo.project.root.exists(), "虚构工程不能落盘");
    }
    let _ = frame(&ctx, &mut app, vec![], size);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn prototype_entry_is_explicit_and_never_persists_layout_or_drafts() {
    use eframe::App;
    #[derive(Default)]
    struct Store {
        writes: Vec<(String, String)>,
    }
    impl eframe::Storage for Store {
        fn get_string(&self, _: &str) -> Option<String> {
            None
        }
        fn set_string(&mut self, key: &str, value: String) {
            self.writes.push((key.into(), value));
        }
        fn flush(&mut self) {}
    }
    assert!(!super::requested_native(std::iter::empty()));
    assert!(!super::requested_native(
        ["/ordinary/workspace".into()].into_iter()
    ));
    assert!(super::requested_native(
        ["--eds11-prototype".into()].into_iter()
    ));
    assert!(super::requested_web_query("?eds11=1"));
    assert!(!super::requested_web_query("?not-eds11=1"));
    let options = super::native_options();
    assert!(!options.persist_window);
    assert_eq!(
        options.viewport.app_id.as_deref(),
        Some("worldedit-eds11-prototype")
    );
    let mut app = Prototype::default();
    app.drafts[0] = "isolated unsaved draft".into();
    assert!(!app.persist_egui_memory());
    let mut storage = Store::default();
    app.save(&mut storage);
    assert!(storage.writes.is_empty());
}
