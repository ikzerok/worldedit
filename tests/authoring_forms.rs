#[path = "../src/app/authoring_forms.rs"]
mod authoring_forms;
use authoring_forms::{EntityForm, FormGuard, RelationForm, RelationTypeForm};
use std::sync::atomic::{AtomicUsize, Ordering};
use worldline_core::project::Project;
use worldline_core::{RelationDirection, TargetRef};

fn project() -> Project {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let root = std::env::temp_dir().join(format!(
        "worldedit-forms-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let mut project = Project::new(&root);
    let entry = project.entry.clone();
    project.documents.retain(|path, _| path == &entry);
    project
        .set_text(&project.entry.clone(), String::new())
        .unwrap();
    project.create_authoring_document(&root.join(".world/project.json"), br#"{
        "schema_version":1,"language_version":"1.10",
        "required_features":["content.entities.v1","content.relations.v1"],"maps":{},"graph_views":{}
    }"#.to_vec()).unwrap();
    project
}
#[test]
fn entity_form_roundtrip_preserves_identity_and_runtime_fingerprint() {
    let mut p = project();
    let before = p.compile().analysis.fingerprint;
    let catalog = p.compile().analysis.catalog;
    let mut form = EntityForm::open(&p, &catalog, 10, None, p.entry.clone()).unwrap();
    form.draft.display = "雾港灯塔".into();
    form.draft.description = "第一行\n第二行，中文与 \\\" 引号".into();
    form.draft.properties.push((
        "custom".into(),
        worldline_core::ast::PropertyValue::Bool(false),
    ));
    form.apply(&mut p, 10).unwrap();
    let catalog = p.compile().analysis.catalog;
    let id = form.draft.id.clone();
    let mut edit =
        EntityForm::open(&p, &catalog, 11, Some(&id), p.root.join("different.wl")).unwrap();
    assert_eq!(edit.path, p.entry);
    assert_eq!(edit.draft.description, form.draft.description);
    edit.draft.display = "旧灯塔".into();
    edit.draft.entity_type = "landmark".into();
    edit.apply(&mut p, 11).unwrap();
    assert_eq!(p.compile().analysis.catalog.entities[&id].display, "旧灯塔");
    assert_eq!(p.compile().analysis.fingerprint, before);
    assert_eq!(edit.draft.properties.len(), 1);
}
#[test]
fn cancelled_and_stale_forms_leave_project_unchanged() {
    let mut p = project();
    let catalog = p.compile().analysis.catalog;
    let before = p.content_baseline();
    let mut form = EntityForm::open(&p, &catalog, 4, None, p.entry.clone()).unwrap();
    form.draft.display = "未提交".into();
    assert_eq!(p.content_baseline(), before);
    assert!(form.apply(&mut p, 5).is_err());
    assert_eq!(p.content_baseline(), before);
    let path = p.entry.clone();
    p.set_text(&path, "# 新来源\n".into()).unwrap();
    let changed = p.content_baseline();
    assert!(form.apply(&mut p, 4).is_err());
    assert_eq!(p.content_baseline(), changed);
    assert_eq!(form.draft.display, "未提交");
    let guard = FormGuard::capture(&p, 7);
    assert!(guard.is_current(&p, 7));
}
#[test]
fn relation_form_requires_explicit_type_and_endpoints_and_keeps_parallel_ids() {
    let mut p = project();
    let catalog = p.compile().analysis.catalog;
    let empty = RelationForm::open(&p, &catalog, 0, None, None).unwrap();
    assert!(empty.from_search.is_empty() && empty.to_search.is_empty());
    let before = p.content_baseline();
    assert!(empty.apply(&mut p, 0).is_err());
    assert_eq!(p.content_baseline(), before);
    for display in ["甲", "乙"] {
        let catalog = p.compile().analysis.catalog;
        let mut form = EntityForm::open(&p, &catalog, 1, None, p.entry.clone()).unwrap();
        form.draft.display = display.into();
        form.apply(&mut p, 1).unwrap();
    }
    let catalog = p.compile().analysis.catalog;
    let mut kind = RelationTypeForm::open(&p, &catalog, 2, None).unwrap();
    kind.draft.display = "照看".into();
    kind.draft.inverse_display = Some("由其照看".into());
    kind.draft.direction = RelationDirection::Directed;
    kind.apply(&mut p, 2).unwrap();
    for note in ["甲记录", "乙记录"] {
        let catalog = p.compile().analysis.catalog;
        let mut form = RelationForm::open(
            &p,
            &catalog,
            3,
            None,
            Some(TargetRef::new("entity", "entity_1")),
        )
        .unwrap();
        form.draft.to = TargetRef::new("entity", "entity_2");
        form.draft.relation_type = kind.draft.id.clone();
        form.draft.source_note = Some(note.into());
        form.apply(&mut p, 3).unwrap();
    }
    let catalog = p.compile().analysis.catalog;
    assert_eq!(catalog.relations.len(), 2);
    assert_ne!(
        catalog.relations["relation_1"].source_note,
        catalog.relations["relation_2"].source_note
    );
    let mut edit = RelationForm::open(&p, &catalog, 4, Some("relation_1"), None).unwrap();
    edit.draft.source_note = None;
    edit.apply(&mut p, 4).unwrap();
    assert_eq!(
        p.compile().analysis.catalog.relations["relation_1"].source_note,
        None
    );
}

#[test]
fn deletion_requires_confirmation_and_rechecks_current_baseline() {
    use authoring_forms::DeleteForm;
    let mut p = project();
    let catalog = p.compile().analysis.catalog;
    let mut entity = EntityForm::open(&p, &catalog, 0, None, p.entry.clone()).unwrap();
    entity.draft.display = "可删除资料".into();
    entity.apply(&mut p, 0).unwrap();
    let before = p.clone();
    let mut form = DeleteForm::open(&p, 1, TargetRef::new("entity", &entity.draft.id));
    assert!(form.impact.can_delete());
    assert!(form.apply(&mut p, 1).is_err());
    assert_eq!(p.content_baseline(), before.content_baseline());
    form.confirmed = true;
    assert!(form.apply(&mut p, 2).is_err());
    assert_eq!(p.content_baseline(), before.content_baseline());
    form.apply(&mut p, 1).unwrap();
    assert!(!p
        .compile()
        .analysis
        .catalog
        .entities
        .contains_key(&entity.draft.id));
    assert!(p.restore(before));
    assert!(p
        .compile()
        .analysis
        .catalog
        .entities
        .contains_key(&entity.draft.id));
}
