//! 有限同步调用样本；不把 headless egui 或缓存耗时冒充原生输入/显示延迟。
use super::*;
use std::time::Instant;
use worldline_core::ast::PropertyValue;
use worldline_core::schemas::{SchemaAuthoringRequest, SchemaBindingMutation, SchemaFieldMutation};

#[test]
#[ignore = "仅 release 的有限 1k/10k 样本；使用 --release --ignored --nocapture 运行"]
#[allow(clippy::assertions_on_constants)]
fn release_schema_snapshot_1k_10k_latency_and_120_frames() {
    assert!(!cfg!(debug_assertions), "性能记录必须使用 release 构建");
    println!("entities,source_bytes,schemas,fields,bindings,load_ms,cold_projection_ms,warm_120_switches_ms,field_preview_ms,field_apply_rebuild_ms,external_refresh_rebuild_ms,idle_120_frames_ms,view_compiles");
    for count in [1_000usize, 10_000] {
        let (ctx, mut app) = fixture(count);
        app.project.save().unwrap();
        let root = app.project.root.clone();
        let source_bytes: usize = app.project.sources().values().map(String::len).sum();
        let started = Instant::now();
        app.load_project(root.clone());
        let load_ms = started.elapsed().as_secs_f64() * 1000.0;
        let index = app.current_schema_index().unwrap();
        let schema_count = index.schemas.len();
        let field_count: usize = index.schemas.iter().map(|schema| schema.fields.len()).sum();
        let binding_count = index.bindings.len();
        assert_eq!(binding_count, count + 1);
        let first = TargetRef::new("entity", "object_0");
        let last = TargetRef::new("entity", &format!("object_{}", count - 1));
        let started = Instant::now();
        app.current_schema_authoring(&first).unwrap();
        let cold_projection_ms = started.elapsed().as_secs_f64() * 1000.0;
        app.current_schema_authoring(&last).unwrap();
        let view_compiles = app.semantic_compile_count;
        let started = Instant::now();
        for round in 0..120 {
            app.current_schema_authoring_shared(if round % 2 == 0 { &first } else { &last })
                .unwrap();
        }
        let warm_switches_ms = started.elapsed().as_secs_f64() * 1000.0;
        assert_eq!(app.semantic_compile_count, view_compiles);
        assert_eq!(
            app.snapshot
                .as_ref()
                .unwrap()
                .schema
                .projection_builds
                .get(),
            2
        );

        let request = SchemaAuthoringRequest {
            schema_version: 1,
            expected_baseline: app.project.content_baseline(),
            target: first.clone(),
            binding: SchemaBindingMutation::Keep,
            fields: vec![SchemaFieldMutation::Set {
                field_id: "population_id".into(),
                key: "population".into(),
                value: PropertyValue::Num(1.0),
            }],
        };
        let started = Instant::now();
        let preview = app.project.preview_schema_authoring(&request).unwrap();
        let field_preview_ms = started.elapsed().as_secs_f64() * 1000.0;
        let started = Instant::now();
        let before = app.project.clone();
        app.project
            .apply_schema_authoring(&request, &preview.plan_digest)
            .unwrap();
        app.remember(before);
        app.recompile();
        app.current_schema_authoring(&first).unwrap();
        let field_rebuild_ms = started.elapsed().as_secs_f64() * 1000.0;
        app.project.save().unwrap();

        let source = app.project.document(&app.active_file).unwrap().to_owned();
        std::fs::write(
            &app.active_file,
            source.replacen("population = 1", "population = 2", 1),
        )
        .unwrap();
        let started = Instant::now();
        assert!(app.project.refresh().unwrap().is_empty());
        app.recompile();
        app.current_schema_authoring(&first).unwrap();
        let external_ms = started.elapsed().as_secs_f64() * 1000.0;
        app.open_schema_editor(&ctx);
        // 首帧布局和字形缓存建立不计入暖空闲帧，但首次工程加载在上面单独记录。
        frame(&ctx, &mut app, vec![]);
        let before_frames = app.semantic_compile_count;
        let started = Instant::now();
        for _ in 0..120 {
            frame(&ctx, &mut app, vec![]);
        }
        let frames_ms = started.elapsed().as_secs_f64() * 1000.0;
        let view_compiles = app.semantic_compile_count - before_frames;
        assert_eq!(view_compiles, 0);
        println!("{count},{source_bytes},{schema_count},{field_count},{binding_count},{load_ms:.3},{cold_projection_ms:.3},{warm_switches_ms:.3},{field_preview_ms:.3},{field_rebuild_ms:.3},{external_ms:.3},{frames_ms:.3},{view_compiles}");
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
#[ignore = "仅 release 的大引用候选探针；不以数字字段代表 ref 最坏形状"]
#[allow(clippy::assertions_on_constants)]
fn release_schema_snapshot_large_reference_projection() {
    assert!(!cfg!(debug_assertions), "性能记录必须使用 release 构建");
    println!("entities,source_bytes,reference_candidates,projection_bytes,cold_reference_ms,warm_120_reference_reads_ms,view_compiles");
    for count in [1_000usize, 10_000] {
        let (_, mut app) = fixture(count);
        let source = app.project.document(&app.active_file).unwrap().replace(
            "field population_id population number required",
            "field population_id population number required\n  field neighbor_id neighbor ref entity entity_type place",
        );
        app.project.set_text(&app.active_file, source).unwrap();
        app.project.set_authoring_document(&app.project.root.join(".world/project.json"), br#"{"schema_version":1,"language_version":"1.13","required_features":["content.object_refs.v1"],"maps":{},"graph_views":{}}"#.to_vec()).unwrap();
        app.recompile();
        let source_bytes: usize = app.project.sources().values().map(String::len).sum();
        let compiles = app.semantic_compile_count;
        let target = TargetRef::new("entity", "object_0");
        let started = Instant::now();
        let projection = app.current_schema_authoring_shared(&target).unwrap();
        let cold_ms = started.elapsed().as_secs_f64() * 1000.0;
        let candidates: usize = projection
            .fields
            .iter()
            .map(|field| field.reference_candidates.len())
            .sum();
        assert_eq!(candidates, count);
        let projection_bytes = serde_json::to_vec(projection.as_ref()).unwrap().len();
        let started = Instant::now();
        for _ in 0..120 {
            let repeated = app.current_schema_authoring_shared(&target).unwrap();
            assert!(Arc::ptr_eq(&projection, &repeated));
        }
        let warm_ms = started.elapsed().as_secs_f64() * 1000.0;
        let view_compiles = app.semantic_compile_count - compiles;
        assert_eq!(view_compiles, 0);
        assert_eq!(
            app.snapshot
                .as_ref()
                .unwrap()
                .schema
                .projection_builds
                .get(),
            1
        );
        println!("{count},{source_bytes},{candidates},{projection_bytes},{cold_ms:.3},{warm_ms:.3},{view_compiles}");
    }
}
