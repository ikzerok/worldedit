use super::*;
use std::time::{Duration, Instant};
use worldline_core::reader_export::*;

fn large_app() -> crate::app::WorldeditApp {
    let mut app = app();
    let mut source = String::from("let score = 7\nalias variable score as \"公开评分 Public score\"\nrelation_type connects as \"连接\"\n");
    let mut choice = app.reader_publish.selection();
    choice.required_features = vec![
        READER_SITE_FEATURE.into(),
        READER_FIELDS_FEATURE.into(),
        READER_STORY_FEATURE.into(),
    ];
    choice.objects.clear();
    choice.fields.clear();
    choice.attachments.clear();
    choice.objects.push(TargetRef::new("variable", "score"));
    for id in 0..1600 {
        source.push_str(&format!("entity place_{id} kind place as \"海港城市 City {id}\"\n  description \"这里记录航海者的公开资料。Public harbor history, trade and travel.\"\n  property climate = \"海洋气候 Mild maritime climate\"\nalias entity place_{id} as \"灯港 Harbor {id}\"\n"));
        let target = TargetRef::new("entity", &format!("place_{id}"));
        choice.objects.push(target.clone());
        choice.fields.push(ReaderFieldSelection {
            target,
            keys: vec!["climate".into()],
        });
    }
    for id in 0..99 {
        let parent = if id == 0 {
            String::new()
        } else {
            " within period_0".into()
        };
        source.push_str(&format!("period period_{id} as \"航海时期 {id}\"{parent}\nalias period period_{id} as \"Era 时期 {id}\"\n"));
        choice
            .objects
            .push(TargetRef::new("period", &format!("period_{id}")));
    }
    for id in 0..100 {
        source.push_str(&format!("relation_def edge_{id} type connects from entity place_{id} to entity place_{}\n  description \"公开商路 Trade route\"\nalias relation edge_{id} as \"航线 Route {id}\"\n",id+1));
        choice
            .objects
            .push(TargetRef::new("relation", &format!("edge_{id}")));
    }
    for id in 0..200 {
        source.push_str(&format!("event event_{id} as \"航海纪事 Voyage {id}\" during period_{}\n  沿海港启程，探索公开世界。A public story about a voyage.\n  choice \"继续航行 Continue\" if score > 0\n    -> END\nalias event event_{id} as \"故事 Story {id}\"\n",id%99));
        choice
            .objects
            .push(TargetRef::new("event", &format!("event_{id}")));
    }
    source.push_str("entity private_canary kind place as \"CANARY_PRIVATE_ENTITY\"\n  property private_note = \"CANARY_PRIVATE_FIELD\"\n");
    assert_eq!(choice.objects.len(), 2000);
    let entry = app.project.entry.clone();
    app.project.set_text(&entry, source).unwrap();
    app.project.set_authoring_document(&app.project.root.join(".world/project.json"), serde_json::to_vec(&serde_json::json!({
        "schema_version":1, "language_version":"1.10", "required_features":["content.object_refs.v1","content.relations.v1"],
        "future_key":{"keep":"preserve"}
    })).unwrap()).unwrap();
    app.project.save().unwrap();
    app.recompile();
    let candidate = ReaderPublicationProfile {
        schema_version: READER_PROFILE_SCHEMA_VERSION,
        required_features: vec![READER_PROFILES_FEATURE.into()],
        id: "performance".into(),
        title: "性能配置".into(),
        selection: choice,
        routes: vec![],
    };
    app.reader_publish.load_profile(candidate);
    app
}

/// 同core固定规模：2000对象/别名、1600字段、200事件、100关系、99时期；五份新工程。
/// 计时包含消费Done、完整core apply、fresh/scope核验、history、版本/缓存与配置UI刷新。
#[test]
#[ignore]
#[allow(clippy::assertions_on_constants)]
fn release_profile_2000_full_ui_apply_five_fresh_fixtures() {
    assert!(
        !cfg!(debug_assertions),
        "必须使用release，不接受debug性能结果"
    );
    use super::super::super::profile_job::{ProfileJob, ProfileMessage};
    use std::sync::{atomic::AtomicBool, mpsc, Arc};
    for round in 0..5 {
        let mut app = large_app();
        for update in 0..2 {
            if update == 1 {
                app.reader_publish.profile_title = "已保存配置更新".into();
            }
            let input = app.reader_profile_input();
            if update == 1 {
                assert_eq!(input.profile.as_ref().unwrap().routes.len(), 2000);
            }
            assert_eq!(input.selection.objects.len(), 2000);
            let fingerprint = app.snapshot.as_ref().unwrap().result.analysis.fingerprint;
            let source = app.project.sources();
            let options = app.project.compile_options();
            let planning = Instant::now();
            let plan = app
                .project
                .preview_save_reader_profile(input.profile.as_ref().unwrap())
                .unwrap();
            let plan_ms = planning.elapsed().as_millis();
            let (sender, receiver) = mpsc::channel();
            sender
                .send(ProfileMessage::Done(Box::new(Ok(plan))))
                .unwrap();
            drop(sender);
            app.reader_publish.profile_job = Some(ProfileJob {
                input,
                cancel: Arc::new(AtomicBool::new(false)),
                receiver: Some(receiver),
            });
            let start = Instant::now();
            app.poll_reader_profile_job();
            let elapsed = start.elapsed();
            assert!(app.io_error.is_none(), "{:?}", app.io_error);
            assert_eq!(
                app.history.len(),
                update + 1,
                "{:?}",
                app.reader_publish.status
            );
            assert_eq!(app.project.sources(), source);
            assert_eq!(app.project.compile_options(), options);
            assert_eq!(
                app.snapshot.as_ref().unwrap().result.analysis.fingerprint,
                fingerprint
            );
            assert!(app.project.is_dirty());
            assert!(!app.project.reader_profile_paths()["performance"].exists());
            println!(
            "reader UI profile round={} update={} planning_ms={} main_apply_ms={} objects=2000 fields=1600",
            round + 1,
            update,
            plan_ms,
            elapsed.as_millis()
        );
            assert!(
                elapsed <= Duration::from_millis(250),
                "完整UI前台提交{}ms超过250ms",
                elapsed.as_millis()
            );
        }
        let root = app.project.root.clone();
        drop(app);
        std::fs::remove_dir_all(root).unwrap();
    }
}
