//! 当前全部实际配色×结构经完整App绘制；检查真实导航widget与交互。
use super::list_geometry::geometry;
use super::navigation_input::{click, focus_field, gesture, native_frame, native_settle, setup};
use super::*;
use crate::theme::{AppearancePreferences, PaletteId, PaletteModeSupport, StylePreset, ThemeMode};

fn thirteen_chapters(app: &mut WorldeditApp) {
    let path = app.project.root.join(".world/manuscripts/book.json");
    let mut book: serde_json::Value =
        serde_json::from_slice(app.project.authoring_document(&path).unwrap().bytes()).unwrap();
    let mut source = SOURCE.to_owned();
    for index in 2..13 {
        let id = format!("extra_{index}");
        source.push_str(&format!(
            "event {id} as \"Extra {index}\"\n  额外章正文。\n  -> END\n"
        ));
        book["entries"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({
                "id":id,"kind":"chapter","title":format!("Extra {index}"),
                "target_ref":{"kind":"event","id":id},"status":"draft"
            }));
    }
    app.project
        .set_text(&app.project.entry.clone(), source)
        .unwrap();
    app.project
        .set_authoring_document(&path, serde_json::to_vec(&book).unwrap())
        .unwrap();
    app.project.save().unwrap();
    app.recompile();
}

#[test]
fn navigation_widgets_filter_and_page_in_all_eighty_five_real_palette_style_combinations() {
    let size = vec2(1280.0, 1000.0);
    let mut combinations = 0;
    for palette in PaletteId::ALL {
        let modes = match palette.mode_support() {
            PaletteModeSupport::Both => vec![ThemeMode::Light, ThemeMode::Dark],
            PaletteModeSupport::LightOnly => vec![ThemeMode::Light],
            PaletteModeSupport::DarkOnly => vec![ThemeMode::Dark],
        };
        for mode in modes {
            for style in [
                StylePreset::Studio,
                StylePreset::Manuscript,
                StylePreset::Technical,
                StylePreset::Focus,
                StylePreset::Ledger,
            ] {
                let (ctx, mut app) = setup();
                ctx.data_mut(|data| {
                    data.insert_temp(
                        egui::Id::new("navigation-matrix-context"),
                        format!("{palette:?}/{mode:?}/{style:?}"),
                    )
                });
                thirteen_chapters(&mut app);
                app.personal.settings.appearance = AppearancePreferences {
                    palette,
                    theme: mode,
                    style,
                    reduce_motion: true,
                    ..Default::default()
                };
                app.manuscript.layout = Layout::Cards;
                native_settle(&ctx, &mut app, size);
                if style == StylePreset::Focus {
                    click(&ctx, &mut app, size, "选择章节");
                }
                let before = app.project.content_baseline();
                let output = native_settle(&ctx, &mut app, size);
                geometry(&ctx, "first");
                for label in ["上一页", "下一页", "匹配 13 / 可识别 13 章"] {
                    assert!(
                        visible(&output, label).is_some(),
                        "{palette:?}/{mode:?}/{style:?}: {label}"
                    );
                }
                click(&ctx, &mut app, size, "下一页");
                native_settle(&ctx, &mut app, size);
                assert_eq!(app.manuscript.navigation.session.offset, 12);
                geometry(&ctx, "extra_12");
                click(&ctx, &mut app, size, "上一页");
                native_settle(&ctx, &mut app, size);
                assert_eq!(app.manuscript.navigation.session.offset, 0);
                for field in ["status", "pov", "text"] {
                    focus_field(&ctx, &mut app, size, field);
                }
                native_frame(&ctx, &mut app, size, vec![Event::Text("First".into())]);
                let output = native_settle(&ctx, &mut app, size);
                assert_eq!(app.manuscript.navigation.session.text, "First");
                assert!(visible(&output, "匹配 1 / 可识别 13 章").is_some());
                geometry(&ctx, "first");
                click(&ctx, &mut app, size, "清除筛选");
                for layout in [Layout::Tree, Layout::List] {
                    app.manuscript.layout = layout;
                    native_settle(&ctx, &mut app, size);
                    geometry(&ctx, "first");
                }
                let (_, _, rect, clip) = geometry(&ctx, "first");
                gesture(&ctx, &mut app, size, rect.intersect(clip).center());
                assert_eq!(
                    app.manuscript.books["book"].selected_entry.as_deref(),
                    Some("first")
                );
                assert_eq!(app.project.content_baseline(), before);
                assert!(app.history.is_empty());
                std::fs::remove_dir_all(&app.project.root).unwrap();
                combinations += 1;
            }
        }
    }
    assert_eq!(combinations, 85);
}
