use super::*;

#[test]
fn real_path_field_and_load_button_read_the_snapshot_and_confirm_repeated_loads() {
    let (ctx, mut app) = app();
    let path = app.project.root.with_extension("csv");
    std::fs::write(&path, CSV).unwrap();
    let before = app.project.sources();
    click(&ctx, &mut app, "按完整文件路径读取 CSV（备用）");
    click(&ctx, &mut app, "完整 CSV 文件路径");
    frame(
        &ctx,
        &mut app,
        egui::vec2(1188.0, 848.0),
        vec![Event::Text(path.display().to_string())],
    );
    assert_eq!(app.catalog_import.native_path, path.display().to_string());
    click(&ctx, &mut app, "读取此路径 CSV");
    wait(&ctx, &mut app);
    assert_eq!(app.catalog_import.table.as_ref().unwrap().rows.len(), 2);
    assert_eq!(app.catalog_import.source_name, path.display().to_string());
    assert_eq!(app.project.sources(), before);
    assert!(app.history.is_empty());
    click(&ctx, &mut app, "读取此路径 CSV");
    assert!(app.catalog_import.replacement.is_some());
    click(&ctx, &mut app, "保留当前快照");
    assert!(app.catalog_import.replacement.is_none());
    assert_eq!(app.catalog_import.csv, CSV);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn invalid_native_paths_keep_exact_user_input_and_the_existing_snapshot() {
    let (ctx, mut app) = app();
    load(&ctx, &mut app, CSV);
    map(&mut app);
    let columns = app.catalog_import.columns.clone();
    let path = app.project.root.with_extension("csv");
    let directory = app.project.root.with_extension("directory");
    std::fs::create_dir_all(&directory).unwrap();
    for (input, expected) in [
        ("relative.csv".into(), "完整"),
        (path.display().to_string(), "不存在"),
        (directory.display().to_string(), "普通"),
    ] {
        app.catalog_import.native_path = input.clone();
        app.catalog_import.load_native_path(&ctx);
        assert!(app
            .catalog_import
            .error
            .as_ref()
            .unwrap()
            .contains(expected));
        assert_eq!(app.catalog_import.native_path, input);
        assert_eq!(app.catalog_import.csv, CSV);
        assert_eq!(app.catalog_import.columns, columns);
    }
    std::fs::write(&path, [0xff]).unwrap();
    app.catalog_import.native_path = path.display().to_string();
    app.catalog_import.load_native_path(&ctx);
    assert!(app.catalog_import.error.as_ref().unwrap().contains("UTF-8"));
    std::fs::File::create(&path)
        .unwrap()
        .set_len(worldline_core::catalog_import::MAX_CSV_BYTES as u64 + 1)
        .unwrap();
    app.catalog_import.load_native_path(&ctx);
    assert!(app.catalog_import.error.as_ref().unwrap().contains("2 MiB"));
    assert_eq!(app.catalog_import.csv, CSV);
    assert_eq!(app.catalog_import.columns, columns);
    assert!(app.history.is_empty());
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(directory).unwrap();
}

#[cfg(unix)]
#[test]
fn native_path_entry_rejects_links_without_reading_the_target() {
    let (ctx, mut app) = app();
    let path = app.project.root.with_extension("csv");
    let link = app.project.root.with_extension("link.csv");
    std::fs::write(&path, CSV).unwrap();
    std::os::unix::fs::symlink(&path, &link).unwrap();
    app.catalog_import.native_path = link.display().to_string();
    app.catalog_import.load_native_path(&ctx);
    assert!(app.catalog_import.table.is_none());
    assert!(app.catalog_import.error.as_ref().unwrap().contains("普通"));
    std::fs::remove_file(link).unwrap();
    std::fs::remove_file(path).unwrap();
}
