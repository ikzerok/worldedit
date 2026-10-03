//! 浏览器后台计算的 typed 传输合同；所有提交仍由当前 Project 复核。
use crate::scene_raster::RasterSpec;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::PathBuf;
use worldline_core::presentation_commands::Revision;
use worldline_core::project::SnapshotState;
use worldline_core::reader_export::{
    ReaderExportPreview, ReaderExportSelection, ReaderProfileSavePlan, ReaderPublicationProfile,
};
use worldline_core::vector_scene::{
    MapScene, SceneBatch, SceneEntityRequest, SceneOp, SvgScenePreview,
};

pub(crate) const SCHEMA_VERSION: u32 = 1;
pub(crate) const MAX_JSON_BYTES: usize = 32 * 1024 * 1024;
pub(crate) const MAX_FILES: usize = 10_000;
pub(crate) const MAX_BYTES: usize = 128 * 1024 * 1024;

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WorkRequest {
    pub schema_version: u32,
    pub job_id: String,
    pub generation: u64,
    pub baseline: String,
    pub entry: PathBuf,
    pub snapshot_state: Option<SnapshotState>,
    pub task: WorkTask,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum WorkTask {
    ProblemsReport {
        options: worldline_core::problems::ProblemsOptions,
        source_observation: String,
    },
    SvgPreview {
        source: String,
    },
    ScenePreview {
        revision: Revision,
        batch: SceneBatch,
    },
    SceneEntityPreview {
        revision: Revision,
        request: SceneEntityRequest,
    },
    RenderScene {
        scene: MapScene,
        extent: [f64; 2],
        spec: RasterSpec,
    },
    MapSvgExport {
        map_id: String,
        selected: Option<BTreeSet<String>>,
    },
    ReaderProfileSavePlan {
        selection: ReaderExportSelection,
        profile: Option<ReaderPublicationProfile>,
        id: String,
        title: String,
    },
    ReaderPackage {
        selection: ReaderExportSelection,
        profile: Option<ReaderPublicationProfile>,
    },
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum WorkOutput {
    ProblemsReport {
        report: worldline_core::problems::ProblemsReport,
    },
    SvgPreview {
        preview: SvgScenePreview,
    },
    ScenePreview {
        batch: SceneBatch,
    },
    SceneEntityPreview {
        request: SceneEntityRequest,
    },
    RenderedScene {
        spec: RasterSpec,
    },
    MapSvgExport {
        source: String,
    },
    ReaderProfileSavePlan {
        plan: ReaderProfileSavePlan,
    },
    ReaderPackage {
        preview: ReaderExportPreview,
        paths: Vec<PathBuf>,
        raw_bytes: usize,
    },
}

pub(crate) enum WorkEvent {
    Progress {
        stage: String,
        completed: usize,
        total: usize,
    },
    Done {
        output: Box<WorkOutput>,
        binaries: Vec<Vec<u8>>,
    },
    Error(String),
}

impl WorkRequest {
    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.schema_version != SCHEMA_VERSION
            || self.job_id.is_empty()
            || self.job_id.len() > 160
            || self.generation > 9_007_199_254_740_991
        {
            return Err("后台任务协议版本或身份无效".into());
        }
        let needs_project = matches!(
            self.task,
            WorkTask::ProblemsReport { .. }
                | WorkTask::ScenePreview { .. }
                | WorkTask::SceneEntityPreview { .. }
                | WorkTask::ReaderPackage { .. }
                | WorkTask::MapSvgExport { .. }
                | WorkTask::ReaderProfileSavePlan { .. }
        );
        if needs_project != self.snapshot_state.is_some() {
            return Err("后台任务与工程快照状态不匹配".into());
        }
        if needs_project {
            crate::reader_zip::safe_snapshot_name(&self.entry)?;
        }
        Ok(())
    }

    pub(crate) fn accepts(&self, output: &WorkOutput, binaries: &[Vec<u8>]) -> Result<(), String> {
        self.accepts_lengths(output, &binaries.iter().map(Vec::len).collect::<Vec<_>>())
    }

    pub(crate) fn accepts_lengths(
        &self,
        output: &WorkOutput,
        binaries: &[usize],
    ) -> Result<(), String> {
        match (&self.task, output) {
            (
                WorkTask::ProblemsReport {
                    options,
                    source_observation,
                },
                WorkOutput::ProblemsReport { report },
            ) if binaries.is_empty()
                && report.schema_version == 1
                && report.content_baseline == self.baseline
                && &report.source_observation == source_observation
                && &report.limits == options
                && report.compile_count <= 1 =>
            {
                Ok(())
            }
            (
                WorkTask::ReaderProfileSavePlan {
                    selection,
                    profile,
                    id,
                    title,
                },
                WorkOutput::ReaderProfileSavePlan { plan },
            ) if binaries.is_empty()
                && &plan.profile.selection == selection
                && &plan.profile.id == id
                && &plan.profile.title == title
                && profile.as_ref().is_none_or(|original| {
                    original.id == plan.profile.id
                        && original.schema_version == plan.profile.schema_version
                        && original.required_features == plan.profile.required_features
                        && reader_profile_routes_preserved(original, &plan.profile)
                }) =>
            {
                Ok(())
            }
            (WorkTask::MapSvgExport { .. }, WorkOutput::MapSvgExport { source })
                if binaries.is_empty() && source.len() <= MAX_JSON_BYTES =>
            {
                Ok(())
            }
            (WorkTask::SvgPreview { .. }, WorkOutput::SvgPreview { .. }) if binaries.is_empty() => {
                Ok(())
            }
            (
                WorkTask::ScenePreview { batch, .. },
                WorkOutput::ScenePreview { batch: returned },
            ) if binaries.is_empty() && normalized_intent_matches(batch, returned) => Ok(()),
            (
                WorkTask::SceneEntityPreview { request, .. },
                WorkOutput::SceneEntityPreview { request: returned },
            ) if binaries.is_empty() && same_json(request, returned) => Ok(()),
            (WorkTask::RenderScene { spec, .. }, WorkOutput::RenderedScene { spec: returned }) => {
                let bytes = usize::try_from(spec.width)
                    .ok()
                    .and_then(|w| {
                        usize::try_from(spec.height)
                            .ok()
                            .and_then(|h| w.checked_mul(h))
                    })
                    .and_then(|n| n.checked_mul(4))
                    .ok_or("后台像素尺寸溢出")?;
                if returned != spec
                    || binaries.len() != 1
                    || binaries[0] != bytes
                    || bytes > MAX_BYTES
                {
                    return Err("后台像素规格或负载与请求不一致".into());
                }
                Ok(())
            }
            (
                WorkTask::ReaderPackage { .. },
                WorkOutput::ReaderPackage {
                    paths, raw_bytes, ..
                },
            ) => {
                if paths.len() > MAX_FILES
                    || binaries.len() != paths.len() + 1
                    || *raw_bytes > MAX_BYTES
                {
                    return Err("后台阅读包数量或预算不一致".into());
                }
                let mut unique = BTreeSet::new();
                let mut actual = 0usize;
                for (path, bytes) in paths.iter().zip(binaries) {
                    crate::reader_zip::safe_name(path)?;
                    if !unique.insert(path) {
                        return Err("后台阅读包路径重复".into());
                    }
                    actual = actual.checked_add(*bytes).ok_or("后台阅读包大小溢出")?;
                }
                if actual != *raw_bytes || binaries.last().is_none_or(|zip| *zip > MAX_BYTES) {
                    return Err("后台阅读包字节数与审核结果不一致".into());
                }
                Ok(())
            }
            _ => Err("后台结果类型或请求意图不匹配".into()),
        }
    }
}

type ReaderRouteKey<'a> = (
    Option<&'a worldline_core::catalog::TargetRef>,
    Option<&'a str>,
    Option<&'a str>,
    &'a str,
);
fn reader_route_key(
    route: &worldline_core::reader_export::ReaderProfileRoute,
) -> ReaderRouteKey<'_> {
    (
        route.target.as_ref(),
        route.manuscript_id.as_deref(),
        route.chapter_id.as_deref(),
        route.output_path.as_str(),
    )
}
fn reader_profile_routes_preserved(
    original: &ReaderPublicationProfile,
    returned: &ReaderPublicationProfile,
) -> bool {
    let routes: BTreeSet<_> = returned.routes.iter().map(reader_route_key).collect();
    original
        .routes
        .iter()
        .all(|route| routes.contains(&reader_route_key(route)))
}

fn same_json<T: Serialize>(first: &T, second: &T) -> bool {
    matches!((serde_json::to_value(first), serde_json::to_value(second)), (Ok(a), Ok(b)) if a == b)
}

fn normalized_intent_matches(expected: &SceneBatch, actual: &SceneBatch) -> bool {
    expected.map_id == actual.map_id
        && expected.expected_revision == actual.expected_revision
        && expected.expected_documents == actual.expected_documents
        && expected.operations.len() == actual.operations.len()
        && expected
            .operations
            .iter()
            .zip(&actual.operations)
            .all(|(a, b)| match (a, b) {
                (
                    SceneOp::ImportSvg {
                        layer_id, title, ..
                    },
                    SceneOp::ImportScene {
                        layer_id: returned_layer,
                        title: returned_title,
                        ..
                    },
                ) => layer_id == returned_layer && title == returned_title,
                _ => a == b,
            })
}

/// 墓碑原字节走二进制侧通道，避免 Vec<u8> 变成巨型 JSON 数字数组。
pub(crate) fn detach_retained(request: &mut WorkRequest) -> Vec<(PathBuf, Vec<u8>)> {
    request
        .snapshot_state
        .as_mut()
        .map(|state| {
            state
                .documents
                .iter_mut()
                .filter_map(|document| {
                    document
                        .retained_bytes
                        .take()
                        .map(|bytes| (document.path.clone(), bytes))
                })
                .collect()
        })
        .unwrap_or_default()
}

pub(crate) fn restore_retained(
    request: &mut WorkRequest,
    retained: Vec<(PathBuf, Vec<u8>)>,
) -> Result<(), String> {
    let mut unique = BTreeSet::new();
    for (path, bytes) in retained {
        crate::reader_zip::safe_snapshot_name(&path)?;
        if !unique.insert(path.clone()) {
            return Err("后台墓碑负载重复".into());
        }
        let state = request
            .snapshot_state
            .as_mut()
            .ok_or("无工程任务不能包含墓碑负载")?;
        let document = state
            .documents
            .iter_mut()
            .find(|item| item.path == path)
            .ok_or("后台墓碑负载没有对应描述")?;
        if !document.deleted || document.retained_bytes.is_some() {
            return Err("后台墓碑负载与状态不一致".into());
        }
        document.retained_bytes = Some(bytes);
    }
    if request.snapshot_state.as_ref().is_some_and(|state| {
        state
            .documents
            .iter()
            .any(|item| item.deleted != item.retained_bytes.is_some())
    }) {
        return Err("后台墓碑原字节缺失或活动文档含保留负载".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests;

/// 只保留最新进度或唯一终态，避免消费者逐帧追赶过期进度。
pub(crate) fn push_event(
    queue: &mut std::collections::VecDeque<WorkEvent>,
    event: WorkEvent,
) -> bool {
    if queue
        .iter()
        .any(|item| matches!(item, WorkEvent::Done { .. } | WorkEvent::Error(_)))
    {
        return false;
    }
    let terminal = matches!(event, WorkEvent::Done { .. } | WorkEvent::Error(_));
    queue.clear();
    queue.push_back(event);
    terminal
}

/// 文本通道也保留 SVG 精确定位，不能在后台错误中丢失 typed 诊断字段。
pub(crate) fn scene_error(error: &worldline_core::vector_scene::SceneError) -> String {
    let mut message = error.to_string();
    if let Some(line) = error.line {
        message.push_str(&format!(" · 行{line}"));
    }
    if let Some(column) = error.column {
        message.push_str(&format!(" 列{column}"));
    }
    if let Some(node) = &error.node_id {
        message.push_str(&format!(" · 节点{node}"));
    }
    if let Some(field) = &error.field {
        message.push_str(&format!(" · 字段{field}"));
    }
    if let Some(operation) = error.operation_index {
        message.push_str(&format!(" · 操作#{operation}"));
    }
    message
}
