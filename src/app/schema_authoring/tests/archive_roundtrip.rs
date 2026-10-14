//! 机器级完整 ZIP 往返：真实 core 事务、editor archive 与磁盘 Project::open。
//! 不代表浏览器挂载、GUI 导出确认或系统文件选择器验收。
use super::*;
use crate::archive::{self, Files};
use std::path::{Path, PathBuf};
use worldline_core::project::Project;

const KINDS: [&str; 4] = ["world", "character", "entity", "relation"];
const MANIFEST: &[u8] = br#"{"schema_version":1,"entry":"world.wl","language_version":"1.13","required_features":["content.entities.v1","content.relations.v1","content.object_refs.v1","content.character_refs.v1"],"maps":{},"graph_views":{}}"#;

fn exclusive_directory(label: &str) -> PathBuf {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let root = std::env::temp_dir().join(format!(
        "schema-zip-{label}-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&root).expect("夹具目录必须独占，不能复用或删除既有路径");
    root
}

fn write_files(root: &Path, files: &Files) {
    use std::io::Write;
    for (relative, bytes) in files {
        let path = root.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .unwrap()
            .write_all(bytes)
            .unwrap();
    }
}

fn original_properties(kind: &str) -> String {
    format!(
        "  property label = \"旧{kind}\"\n  property count = 7\n  property public = true\n  property note = \"旧备注\"\n  property category = \"city\"\n  property unknown = \"{kind}\\n保留\" // 原注释\n"
    )
}

fn fixture_files() -> Files {
    let schema_fields = "  field label_id label text required\n  field count_id count number required\n  field public_id public boolean required\n  field note_id note text required\n  field category_id category enum \"city\" \"town\"\n  field owner_id owner ref entity entity_type organization\n  field spare_id spare text\n";
    let schemas = KINDS
        .iter()
        .map(|kind| {
            let subtype = if *kind == "entity" {
                " entity_type place"
            } else {
                ""
            };
            format!("schema {kind}_data for {kind}{subtype}\n{schema_fields}")
        })
        .collect::<String>();
    Files::from([
        (".world/project.json".into(), MANIFEST.to_vec()),
        ("schema.wl".into(), schemas.into_bytes()),
        ("world.wl".into(), format!(
            "include \"schema.wl\"\ninclude \"资料/characters.wl\"\ninclude \"资料/entities.wl\"\ninclude \"资料/relations.wl\"\nworld shared as \"共同世界\"\n{}event start\n  ZIP往返正文。\n  -> END\n", original_properties("world")
        ).into_bytes()),
        ("资料/characters.wl".into(), format!(
            "character shared as \"人物资料\"\n{}", original_properties("character")
        ).into_bytes()),
        ("资料/entities.wl".into(), format!(
            "entity shared kind place as \"实体资料\"\n{}entity council kind organization as \"议会\"\n", original_properties("entity")
        ).into_bytes()),
        ("资料/relations.wl".into(), format!(
            "relation_type knows as \"认识\"\nrelation_def shared type knows from character shared to entity shared\n{}", original_properties("relation")
        ).into_bytes()),
        ("未选择/旁稿.wl".into(), "// 独立未选来源\r\nentity untouched kind place as \"未选对象\"\r\n  property raw = \"保持\" // 原字节\r\n".as_bytes().to_vec()),
        ("notes/opaque.json".into(), b"ordinary JSON bytes\r\n\0\xff".to_vec()),
        ("assets/unreferenced.bin".into(), vec![0, 9, 128, 255, 13, 10]),
    ])
}

fn expected_values(kind: &str) -> Vec<(&'static str, PropertyValue)> {
    vec![
        ("label", PropertyValue::Str(format!("已改{kind}资料"))),
        ("count", PropertyValue::Num(0.0)),
        ("public", PropertyValue::Bool(false)),
        ("note", PropertyValue::Str(String::new())),
        ("category", PropertyValue::Str("town".into())),
        (
            "owner",
            PropertyValue::Ref(TargetRef::new("entity", "council")),
        ),
    ]
}

fn assert_four_projections(project: &Project) {
    assert_eq!(project.language_version(), "1.13");
    for kind in KINDS {
        let target = TargetRef::new(kind, "shared");
        let projection = project.schema_authoring(&target).unwrap();
        assert!(projection.complete && projection.writable, "{kind}");
        assert_eq!(projection.target, target);
        assert_eq!(
            projection.binding.as_ref().unwrap().schema_id,
            format!("{kind}_data")
        );
        assert!(
            projection.diagnostics.is_empty(),
            "{kind}: {:?}",
            projection.diagnostics
        );
        assert_eq!(projection.fields.len(), 7);
        for (key, value) in expected_values(kind) {
            let field = projection
                .fields
                .iter()
                .find(|field| field.key == key)
                .unwrap();
            assert!(field.present, "{kind}/{key}");
            assert_eq!(field.value.as_ref(), Some(&value), "{kind}/{key}");
        }
        let missing = projection
            .fields
            .iter()
            .find(|field| field.key == "spare")
            .unwrap();
        assert!(!missing.present);
        assert_eq!(missing.value, None);
        let source = project
            .document(Path::new(&projection.object.file))
            .unwrap();
        assert!(source.contains(&format!("  property unknown = \"{kind}\\n保留\" // 原注释")));
    }
}

#[test]
fn schema_four_kinds_applied_facts_survive_complete_editor_zip_import_save_reopen() {
    let root = exclusive_directory("original");
    let originals = fixture_files();
    write_files(&root, &originals);
    let mut project = Project::open(&root).unwrap();
    assert!(!project.compile().has_errors());
    for kind in KINDS {
        let request = SchemaAuthoringRequest {
            schema_version: 1,
            expected_baseline: project.content_baseline(),
            target: TargetRef::new(kind, "shared"),
            binding: SchemaBindingMutation::Bind {
                schema_id: format!("{kind}_data"),
            },
            fields: expected_values(kind)
                .into_iter()
                .map(|(key, value)| SchemaFieldMutation::Set {
                    field_id: format!("{key}_id"),
                    key: key.into(),
                    value,
                })
                .collect(),
        };
        let before = project.content_baseline();
        let preview = project.preview_schema_authoring(&request).unwrap();
        assert!(preview.changed && !preview.has_errors);
        assert_eq!(project.content_baseline(), before);
        project
            .apply_schema_authoring(&request, &preview.plan_digest)
            .unwrap();
    }
    assert!(project.is_dirty());
    assert_four_projections(&project);
    // 导出的必须是已应用但未保存的新稿；不能偶然读取原盘冒充成功。
    assert_eq!(
        std::fs::read(root.join("world.wl")).unwrap(),
        originals[Path::new("world.wl")]
    );
    let exported = project.export_files().unwrap();
    assert_ne!(
        exported[Path::new("world.wl")],
        originals[Path::new("world.wl")]
    );
    for name in [
        ".world/project.json",
        "schema.wl",
        "未选择/旁稿.wl",
        "notes/opaque.json",
        "assets/unreferenced.bin",
    ] {
        assert_eq!(
            exported[Path::new(name)],
            originals[Path::new(name)],
            "{name}"
        );
    }
    assert!(!exported.contains_key(Path::new(archive::MANIFEST)));
    let mut packaged = exported.clone();
    archive::ensure_legacy_manifest(&mut packaged, "world.wl").unwrap();
    assert_eq!(packaged.len(), exported.len() + 1);
    for (path, bytes) in &exported {
        assert_eq!(&packaged[path], bytes);
    }
    let zip = archive::encode(&packaged).unwrap();
    let decoded = archive::decode(&zip).unwrap();
    assert_eq!(decoded, packaged);
    let prepared = archive::prepare_import(decoded).unwrap();
    let prepared_from_zip =
        archive::prepare_import(Files::from([("工程.zip".into(), zip)])).unwrap();
    assert_eq!(prepared_from_zip, prepared);
    assert_eq!(prepared, packaged);
    assert_eq!(archive::entry(&prepared).unwrap(), Path::new("world.wl"));
    // 与既有 archive 测试一致：物化已验证 Files，再走真实磁盘 Project 打开路径。
    let imported_root = exclusive_directory("imported");
    write_files(&imported_root, &prepared);
    let mut imported = Project::open(&imported_root).unwrap();
    assert_four_projections(&imported);
    assert_eq!(imported.export_files().unwrap(), packaged);
    imported.save().unwrap();
    let reopened = Project::open(&imported_root).unwrap();
    assert_four_projections(&reopened);
    assert_eq!(reopened.export_files().unwrap(), packaged);
    for (path, bytes) in &prepared {
        assert_eq!(&std::fs::read(imported_root.join(path)).unwrap(), bytes);
    }
    for (path, bytes) in &originals {
        assert_eq!(&std::fs::read(root.join(path)).unwrap(), bytes);
    }
}
