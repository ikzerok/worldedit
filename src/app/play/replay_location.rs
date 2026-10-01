//! 重放定位只消费本次实际状态，不把旧轨迹定位套到当前稿件。
use super::super::WorldeditApp;
use serde_json::Value;
use std::path::Path;
use worldline_runtime::{ReplayResult, ReplayStatus};

pub(super) struct ReplayLocation {
    pub node: String,
    pub line: u32,
    file: Option<String>,
}

fn fragment_frame<'a>(result: &'a ReplayResult, node: &str) -> Option<&'a Value> {
    let name = node.strip_prefix("fragment:")?;
    let mut matches = result.current_state["calls"]
        .as_array()?
        .iter()
        .filter(|frame| frame["fragment"].as_str() == Some(name));
    let frame = matches.next()?;
    matches.next().is_none().then_some(frame)
}

pub(super) fn failure_location(result: &ReplayResult) -> Option<ReplayLocation> {
    if result.current_state["ended"].as_bool() == Some(true) {
        return None;
    }
    let (node, line) = match &result.status {
        ReplayStatus::StoryFailed {
            node: Some(node),
            line: Some(line),
            ..
        } => (node.as_str(), *line),
        ReplayStatus::Diverged { actual_choices, .. } => {
            let node = result.current_node.as_deref()?;
            let line = actual_choices
                .iter()
                .find(|choice| choice.node == node)
                .map(|choice| choice.line)
                .or_else(|| {
                    fragment_frame(result, node)?["line"]
                        .as_u64()
                        .and_then(|line| u32::try_from(line).ok())
                })?;
            (node, line)
        }
        _ => return None,
    };
    if line == 0 {
        return None;
    }
    let file = if node.starts_with("fragment:") {
        Some(fragment_frame(result, node)?["file"].as_str()?.to_owned())
    } else {
        None
    };
    Some(ReplayLocation {
        node: node.to_owned(),
        line,
        file,
    })
}

impl WorldeditApp {
    pub(super) fn jump_to_replay_failure(&mut self) {
        if self.replay_debugger.result_version != Some(self.version) {
            self.replay_debugger.notice = Some("编辑稿已变化，请重新重放后定位".into());
            return;
        }
        let location = self
            .replay_debugger
            .result
            .as_ref()
            .and_then(failure_location);
        let Some(location) = location else {
            self.replay_debugger.notice = Some("当前停止位置不可定位，原记录仅供对比".into());
            return;
        };
        let file = location.file.or_else(|| {
            self.snapshot
                .as_ref()?
                .result
                .analysis
                .graph
                .nodes
                .iter()
                .find(|node| node.name == location.node)
                .map(|node| node.file.clone())
        });
        let file = file.and_then(|file| {
            worldline_core::file_access::within(&self.project.root, Path::new(&file)).ok()
        });
        let Some(file) = file.filter(|file| {
            self.project
                .document(file)
                .is_ok_and(|source| source.lines().nth(location.line as usize - 1).is_some())
        }) else {
            self.replay_debugger.notice = Some("当前停止位置的源码文件或行号不可用".into());
            return;
        };
        self.jump_to_file(&file.to_string_lossy(), location.line, 1);
    }
}
