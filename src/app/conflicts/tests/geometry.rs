//! 量取实际绘制材料shape的父clip，而非仅检查推算的目标尺寸。
use super::*;

const INTRO: &str = "三方原文 → 明确候选 → 完整预览 → 采纳。保存另行执行。";

fn text_geometry(output: &egui::FullOutput, label: &str) -> Option<(Rect, Rect)> {
    output.shapes.iter().find_map(|shape| match &shape.shape {
        egui::Shape::Text(text) if text.galley.text() == label => Some((
            text.galley.rect.translate(text.pos.to_vec2()),
            shape.clip_rect,
        )),
        _ => None,
    })
}

fn assert_material_geometry(
    output: &egui::FullOutput,
    size: egui::Vec2,
    min_height: f32,
    min_width: f32,
) {
    let screen = Rect::from_min_size(egui::Pos2::ZERO, size);
    let (intro, parent_clip) = text_geometry(output, INTRO).expect("材料首行必须实际绘制");
    let (close, _) = text_geometry(output, "关闭并保留").expect("顶部关闭必须实际绘制");
    let surface = output
        .shapes
        .iter()
        .filter_map(|shape| match &shape.shape {
            egui::Shape::Rect(rect)
                if rect.rect.contains_rect(intro)
                    && rect.rect.contains_rect(close)
                    && rect.rect.area() < screen.area() =>
            {
                Some(rect.rect)
            }
            _ => None,
        })
        .min_by(|left, right| left.area().total_cmp(&right.area()))
        .expect("必须找到包含标题动作和材料的实际面板shape");
    assert!(
        screen.contains_rect(surface),
        "材料面板被真实父视口裁剪：{surface:?} {screen:?}"
    );
    assert!(
        parent_clip.contains_rect(intro),
        "材料首行被父容器裁剪：{intro:?} {parent_clip:?}"
    );
    assert!(
        screen.contains_rect(parent_clip),
        "材料父clip越过真实视口：{parent_clip:?} {screen:?}"
    );
    // clip有时横向继承父级宽度；和真实绘制面板相交后才计算材料可见面积。
    let material_clip = parent_clip.intersect(surface);
    assert!(
        material_clip.height() >= min_height,
        "材料实际可见高度不足：{} < {min_height}，viewport={size:?}",
        material_clip.height()
    );
    assert!(
        material_clip.width() >= min_width,
        "材料实际可见宽度不足：{} < {min_width}，viewport={size:?}",
        material_clip.width()
    );
    assert!(
        material_clip.area() >= min_width * min_height,
        "材料实际可见面积不足"
    );
    for label in ["关闭并保留", "预览候选", "采纳到内存"] {
        assert!(visible(output, label, screen), "{label}在真实小窗中被裁剪");
        let (control, clip) = text_geometry(output, label).unwrap();
        assert!(clip.contains_rect(control));
        assert!(
            control.bottom() < material_clip.top(),
            "{label}不能被挤进材料滚动区"
        );
    }
}

#[test]
fn reconciliation_materials_have_real_viewport_area_at_native_large_and_small_sizes() {
    for (size, min_height, min_width) in [
        (egui::vec2(1188.0, 848.0), 600.0, 1000.0),
        (egui::vec2(800.0, 600.0), 400.0, 700.0),
        (egui::vec2(400.0, 300.0), 130.0, 300.0),
    ] {
        let (ctx, mut app) = fixture();
        let baseline = app.project.content_baseline();
        let output = settle(&ctx, &mut app, size);
        assert_material_geometry(&output, size, min_height, min_width);
        assert_eq!(app.project.content_baseline(), baseline);
        let _ = fs::remove_dir_all(app.project.root);
    }
}

#[test]
fn reconciliation_material_area_and_toolbar_survive_repeated_large_small_resize() {
    let (ctx, mut app) = fixture();
    app.conflict_view.drafts.insert(
        "world.wl".into(),
        Draft {
            choice: Some(ReconciliationChoice::Manual {
                text: String::new(),
            }),
            manual: "保留的中文手工候选🙂".into(),
        },
    );
    let baseline = app.project.content_baseline();
    for (size, min_height, min_width) in [
        (egui::vec2(1188.0, 848.0), 600.0, 1000.0),
        (egui::vec2(400.0, 300.0), 130.0, 300.0),
        (egui::vec2(800.0, 600.0), 400.0, 700.0),
        (egui::vec2(1188.0, 848.0), 600.0, 1000.0),
    ] {
        let output = settle(&ctx, &mut app, size);
        assert_material_geometry(&output, size, min_height, min_width);
        assert_eq!(
            app.conflict_view.drafts[&PathBuf::from("world.wl")].manual,
            "保留的中文手工候选🙂"
        );
        assert_eq!(app.project.content_baseline(), baseline);
    }
    let _ = fs::remove_dir_all(app.project.root);
}
