use super::*;

#[test]
fn dialogue_history_typed_locale_and_json_both_orders_travel_all_shared_edges() {
    // H02/H03/H04：T 是独立 draft-only 边，完整回到事务前需走三条边。
    for imported in [false, true] {
        for locale_first in [false, true] {
            let (ctx, mut app, _cleanup) = fixture();
            let path = app.project.entry.clone();
            let before = project_files(&app.project);
            let disk_before = disk(&app);
            let remote = app.project.root.join("remote.wl");
            let mut other = app.project.open_source_writing_buffer(&remote).unwrap();
            other.replace_source(format!("{}// 跨文件未应用稿😀\n", other.source()));
            app.manuscript.restore_writing_buffers(&[other.clone()]);
            if locale_first {
                if imported {
                    import_locale(&ctx, &mut app);
                } else {
                    typed_locale(&ctx, &mut app);
                }
            }
            stage(&ctx, &mut app, "原来的正式对白", "跨事务台词😀");
            let staged = buffer(&app, &path);
            if !locale_first {
                if imported {
                    import_locale(&ctx, &mut app);
                } else {
                    typed_locale(&ctx, &mut app);
                }
            }
            assert_eq!(counts(&app), (1, 0, 1, 0));
            assert_buffer(&app, &staged, true);
            assert_buffer(&app, &other, true);
            assert_eq!(app.project.sources()[&path].as_bytes(), before[&path]);
            assert_eq!(disk(&app), disk_before);
            assert!(!sidecar(&app).exists());
            apply_body(&ctx, &mut app);
            let applied = project_files(&app.project);
            assert_eq!(app.project.document(&path).unwrap(), staged.source());
            assert_eq!(counts(&app), (2, 0, 1, 0));
            app.edit_undo(false);
            assert!(app.io_error.is_none(), "{:?}", app.io_error);
            assert_buffer(&app, &staged, true);
            assert_translation(&app);
            app.edit_undo(false);
            assert!(app.io_error.is_none(), "{:?}", app.io_error);
            if locale_first {
                // T is the next edge, so L remains reachable below it.
                assert_eq!(counts(&app), (1, 1, 0, 1));
                assert!(!buffer(&app, &path).is_changed());
                assert_translation(&app);
            } else {
                assert_eq!(counts(&app), (0, 2, 1, 0));
                assert_buffer(&app, &staged, true);
                assert!(app
                    .project
                    .authoring_document(&sidecar(&app))
                    .unwrap()
                    .is_deleted());
            }
            app.edit_undo(false);
            assert!(app.io_error.is_none(), "{:?}", app.io_error);
            assert_eq!(project_files(&app.project), before);
            assert_eq!(counts(&app), (0, 2, 0, 1));
            assert!(!buffer(&app, &path).is_changed());
            assert_buffer(&app, &other, true);
            for _ in 0..3 {
                app.edit_undo(true);
                assert!(app.io_error.is_none(), "{:?}", app.io_error);
            }
            assert_eq!(counts(&app), (2, 0, 1, 0));
            assert_eq!(project_files(&app.project), applied);
            assert_eq!(disk(&app), disk_before);
            assert_buffer(&app, &other, true);
            assert_translation(&app);
            save_reopen(&mut app);
            assert_buffer(&app, &other, true);
        }
    }
}
