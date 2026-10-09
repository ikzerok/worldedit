//! worker 内的纯计算入口；不保存原稿、打开浏览器或请求下载。
use crate::archive::Files;
use crate::worker_protocol::*;
use js_sys::{Array, Object, Reflect, Uint8Array};
use std::path::Path;
use wasm_bindgen::{prelude::*, JsCast};
use worldline_core::project::Project;

#[wasm_bindgen]
pub fn worker_execute(
    request_json: &str,
    files: JsValue,
    retained: JsValue,
) -> Result<JsValue, JsValue> {
    execute(request_json, files, retained).map_err(|error| JsValue::from_str(&error))
}

fn execute(request_json: &str, files: JsValue, retained: JsValue) -> Result<JsValue, String> {
    if web_sys::window().is_some() {
        return Err("后台入口不能在编辑器Window中执行".into());
    }
    if request_json.len() > MAX_JSON_BYTES {
        return Err("后台请求JSON超过32MiB预算".into());
    }
    let mut request = parse_request_json(request_json)?;
    request.validate()?;
    let mut total = 0usize;
    let files = read_files(files, &mut total)?;
    let retained = read_files(retained, &mut total)?;
    if files.len().saturating_add(retained.len()) > MAX_FILES
        || retained.keys().any(|path| files.contains_key(path))
    {
        return Err("后台文件/墓碑数量超限或身份重叠".into());
    }
    restore_retained(&mut request, retained.into_iter().collect())?;
    let root = Path::new("/world");
    worldline_core::file_access::mount(
        files
            .iter()
            .map(|(path, bytes)| (root.join(path), bytes.clone()))
            .collect(),
    );
    let project = if let Some(state) = &request.snapshot_state {
        let project = Project::from_snapshot_with_state(root, &request.entry, &files, state)?;
        if project.content_baseline() != request.baseline {
            return Err("后台快照内容基线不匹配".into());
        }
        Some(project)
    } else {
        None
    };
    let need_project = || {
        project
            .as_ref()
            .ok_or_else(|| "后台任务缺少工程".to_owned())
    };
    let (output, binaries) = match &request.task {
        WorkTask::CatalogScope {
            query,
            max_candidates,
        } => {
            progress(&request, "构建完整资料查询与地图范围", 0, 1);
            let scope = need_project()?
                .catalog_scope_snapshot(query, *max_candidates)
                .map_err(|error| error.to_string())?;
            (WorkOutput::CatalogScope { scope }, Vec::new())
        }
        WorkTask::CatalogCsvParse { csv } => {
            progress(&request, "读取世界资料 CSV", 0, 1);
            let table = worldline_core::catalog_import::parse_catalog_csv(csv)
                .map_err(|error| format!("{}：{}", error.code, error.message))?;
            (WorkOutput::CatalogCsvParse { table }, Vec::new())
        }
        WorkTask::CatalogImportPreview { request: import } => {
            progress(&request, "检查整批世界资料", 0, 1);
            let plan = need_project()?.preview_catalog_import(import)?;
            (WorkOutput::CatalogImportPreview { plan }, Vec::new())
        }
        WorkTask::ProblemsReport {
            options,
            source_observation,
        } => {
            if need_project()?
                .problems_observation_key()
                .map_err(|error| error.to_string())?
                != *source_observation
            {
                return Err("后台工程来源范围不匹配".into());
            }
            let report = need_project()?
                .problems_report_with_progress(options, &mut |domain| {
                    progress(&request, &format!("检查 {domain:?}"), 0, 12);
                    true
                })
                .map_err(|error| error.to_string())?;
            (WorkOutput::ProblemsReport { report }, Vec::new())
        }
        WorkTask::SvgPreview { source } => {
            let preview = worldline_core::svg_import::preview_scene_with_control(
                source,
                &worldline_core::vector_scene::SceneLimits::default(),
                &mut |value| {
                    progress(&request, &value.stage, value.completed, value.total);
                    true
                },
            )
            .map_err(|error| scene_error(&error))?;
            (WorkOutput::SvgPreview { preview }, Vec::new())
        }
        WorkTask::ScenePreview { revision, batch } => {
            let plan = worldline_core::vector_scene::preview_batch_with_control(
                need_project()?,
                *revision,
                batch.clone(),
                &worldline_core::vector_scene::SceneLimits::default(),
                &mut |value| {
                    progress(&request, &value.stage, value.completed, value.total);
                    true
                },
            )
            .map_err(|error| scene_error(&error))?;
            (
                WorkOutput::ScenePreview {
                    batch: plan.normalized_batch().clone(),
                },
                Vec::new(),
            )
        }
        WorkTask::SceneEntityPreview {
            revision,
            request: binding,
        } => {
            progress(&request, "检查地点与绑定", 0, 1);
            worldline_core::vector_scene::preview_entity_binding(
                need_project()?,
                *revision,
                binding.clone(),
            )
            .map_err(|error| scene_error(&error))?;
            (
                WorkOutput::SceneEntityPreview {
                    request: binding.clone(),
                },
                Vec::new(),
            )
        }
        WorkTask::RenderScene {
            scene,
            extent,
            spec,
        } => {
            progress(&request, "渲染矢量", 0, 1);
            let bytes = crate::scene_raster::render_scene(scene, *extent, spec)?;
            (
                WorkOutput::RenderedScene { spec: spec.clone() },
                vec![bytes],
            )
        }
        WorkTask::MapSvgExport { map_id, selected } => {
            progress(&request, "生成矢量SVG", 0, 1);
            let index = need_project()?.map_index();
            let map = index.maps.get(map_id).ok_or("地图不存在或无法安全读取")?;
            let source = worldline_core::vector_scene::map_to_safe_svg(map, selected.as_ref())
                .map_err(|error| scene_error(&error))?;
            (WorkOutput::MapSvgExport { source }, Vec::new())
        }
        WorkTask::ReaderProfileSavePlan {
            selection,
            profile,
            id,
            title,
        } => {
            progress(&request, "检查发布配置", 0, 2);
            let project = need_project()?;
            let mut candidate = if let Some(profile) = profile {
                if &profile.id != id {
                    return Err("发布配置身份与保存请求不一致".into());
                }
                profile.clone()
            } else {
                project.create_reader_profile(id, selection)?
            };
            candidate.selection = selection.clone();
            candidate.title = title.clone();
            progress(&request, "生成配置保存计划", 1, 2);
            let plan = project.preview_save_reader_profile(&candidate)?;
            (WorkOutput::ReaderProfileSavePlan { plan }, Vec::new())
        }
        WorkTask::ReaderPackage { selection, profile } => {
            let project = need_project()?;
            if profile
                .as_ref()
                .is_some_and(|profile| &profile.selection != selection)
            {
                return Err("后台阅读包profile与选择不一致".into());
            }
            let mut callback = |value: &worldline_core::reader_export::ReaderExportProgress| {
                progress(&request, &value.phase, value.completed, value.total);
                true
            };
            let preview = if let Some(profile) = profile {
                project.preview_reader_profile_with_progress(profile, &mut callback)?
            } else {
                project.preview_reader_export_with_progress(selection, &mut callback)?
            };
            let files = if let Some(profile) = profile {
                project.build_reader_profile_with_progress(
                    profile,
                    &preview.plan_digest,
                    &mut callback,
                )?
            } else {
                project.build_reader_export_with_progress(
                    selection,
                    &preview.plan_digest,
                    &mut callback,
                )?
            };
            let raw_bytes = crate::reader_zip::validate(&files)?;
            let zip = crate::reader_zip::encode(&files, &mut |done, total| {
                progress(&request, "编码并核对ZIP", done, total);
                true
            })?;
            let paths = files.keys().cloned().collect();
            let mut binaries: Vec<_> = files.into_values().collect();
            binaries.push(zip);
            (
                WorkOutput::ReaderPackage {
                    preview,
                    paths,
                    raw_bytes,
                },
                binaries,
            )
        }
    };
    request.accepts(&output, &binaries)?;
    let output_json = serde_json::to_string(&output).map_err(|e| e.to_string())?;
    if output_json.len() > MAX_JSON_BYTES {
        return Err("后台结果JSON超过预算".into());
    }
    let result = Object::new();
    Reflect::set(&result, &"output_json".into(), &output_json.into()).map_err(js_error)?;
    let arrays = Array::new();
    for bytes in binaries {
        arrays.push(&Uint8Array::from(bytes.as_slice()));
    }
    Reflect::set(&result, &"binaries".into(), &arrays).map_err(js_error)?;
    Ok(result.into())
}

fn read_files(value: JsValue, total: &mut usize) -> Result<Files, String> {
    if !Array::is_array(&value) {
        return Err("后台文件负载不是数组".into());
    }
    let array = Array::from(&value);
    if array.length() as usize > MAX_FILES {
        return Err("后台文件数量超过预算".into());
    }
    let mut files = Files::new();
    for item in array.iter() {
        let path = Reflect::get(&item, &"path".into())
            .map_err(js_error)?
            .as_string()
            .ok_or("后台文件缺少路径")?;
        crate::reader_zip::safe_snapshot_name(Path::new(&path))?;
        let data = Reflect::get(&item, &"bytes".into())
            .map_err(js_error)?
            .dyn_into::<Uint8Array>()
            .map_err(|_| "后台文件不是Uint8Array")?;
        *total = total
            .checked_add(data.length() as usize)
            .ok_or("后台文件字节数溢出")?;
        if *total > MAX_BYTES {
            return Err("后台快照超过128MiB预算".into());
        }
        if files.insert(path.into(), data.to_vec()).is_some() {
            return Err("后台文件路径重复".into());
        }
    }
    Ok(files)
}

fn progress(request: &WorkRequest, stage: &str, completed: usize, total: usize) {
    let payload = serde_json::json!({"job_id":request.job_id,"generation":request.generation.to_string(),
        "baseline":request.baseline,"event":"progress","stage":stage,"completed":completed,"total":total});
    if let Ok(value) = js_sys::JSON::parse(&payload.to_string()) {
        let scope: web_sys::DedicatedWorkerGlobalScope = js_sys::global().unchecked_into();
        let _ = scope.post_message(&value);
    }
}

fn js_error(value: JsValue) -> String {
    value.as_string().unwrap_or_else(|| format!("{value:?}"))
}
