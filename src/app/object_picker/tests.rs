use super::*;
fn matches(object: &CatalogObject, query: &str, allowed: &[&str]) -> bool {
    Catalog {
        objects: vec![object.clone()],
        ..Default::default()
    }
    .search_objects_filtered_page(query, &filter(allowed, None), Default::default())
    .is_ok_and(|page| page.total == 1)
}
#[test]
fn duplicate_names_never_collapse_kind_or_source() {
    let a = CatalogObject {
        target: TargetRef::new("entity", "same"),
        display: "林".into(),
        file: "a.wl".into(),
        line: 1,
    };
    let b = CatalogObject {
        target: TargetRef::new("character", "same"),
        display: "林".into(),
        file: "b.wl".into(),
        line: 2,
    };
    assert!(matches(&a, "林", &[]));
    assert!(matches(&b, "林", &[]));
    assert!(!matches(&a, "same", &["character"]));
    assert!(matches(&b, "b.wl", &["character"]));
}
#[test]
fn same_basename_sources_remain_distinguishable_in_the_visible_candidate() {
    let make = |file: &str| CatalogObject {
        target: TargetRef::new("character", "same"),
        display: "林".into(),
        file: file.into(),
        line: 3,
    };
    let left = candidate_label(&make("/project/甲/人物.wl"));
    let right = candidate_label(&make("/project/乙/人物.wl"));
    assert_ne!(left, right);
    assert!(left.contains("/project/甲/人物.wl:3"));
    assert!(right.contains("/project/乙/人物.wl:3"));
}

#[test]
fn relative_caption_preserves_directories_identity_and_full_path_search() {
    let object = CatalogObject {
        target: TargetRef::new("entity", "record_294"),
        display: "潮汐档案第295号".into(),
        file: "/project/深层 目录/档案/人物.wl".into(),
        line: 17,
    };
    let caption = candidate_caption(&object, Some(Path::new("/project")));
    assert_eq!(
        caption,
        "潮汐档案第295号 · 实体:record_294\n深层 目录/档案/人物.wl:17"
    );
    assert!(matches(&object, "/project/深层 目录/档案/人物.wl", &[]));
    assert!(candidate_label(&object).contains(&object.file));
    assert!(candidate_caption(&object, Some(Path::new("/another"))).contains(&object.file));
    assert!(candidate_caption(&object, None).contains(&object.file));
}

#[test]
fn candidate_row_uses_two_text_levels_and_a_relative_source() {
    let ctx = egui::Context::default();
    let object = CatalogObject {
        target: TargetRef::new("character", "lin"),
        display: "林舟".into(),
        file: "/project/人物/林舟.wl".into(),
        line: 3,
    };
    let output = ctx.run(egui::RawInput::default(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            candidate_row(ui, &object, Some(Path::new("/project")), true);
        });
    });
    let job = output
        .shapes
        .iter()
        .find_map(|shape| match &shape.shape {
            egui::Shape::Text(text) if text.galley.job.text.contains("林舟") => {
                Some(&text.galley.job)
            }
            _ => None,
        })
        .expect("候选应实际绘制");
    assert_eq!(job.text, "林舟 · 人物:lin\n人物/林舟.wl:3");
    assert!(job.sections[0].format.font_id.size > job.sections[1].format.font_id.size);
}
