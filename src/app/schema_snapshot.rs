//! 只消费同一次已完成编译；绘制和对象切换不编译，也不重复执行 schema 校验。
use super::WorldeditApp;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use worldline_core::project::Project;
use worldline_core::schemas::{SchemaAuthoringProjection, SchemaIndex};
use worldline_core::{CompileResult, TargetRef};

// 单投影有 core 的 8 MiB 上限；这里只保留最近 16 个，避免对象遍历无界增长。
const MAX_CACHED_PROJECTIONS: usize = 16;
type CachedProjection = (TargetRef, Result<Arc<SchemaAuthoringProjection>, String>);

pub(super) struct SchemaSnapshot {
    root: PathBuf,
    baseline: String,
    index: Arc<SchemaIndex>,
    projections: RefCell<VecDeque<CachedProjection>>,
    #[cfg(test)]
    projection_builds: std::cell::Cell<u64>,
}

impl SchemaSnapshot {
    pub(super) fn new(project: &Project, compiled: &CompileResult) -> Self {
        Self {
            root: project.root.clone(),
            baseline: project.content_baseline(),
            index: Arc::new(SchemaIndex::from_compile(compiled)),
            projections: RefCell::default(),
            #[cfg(test)]
            projection_builds: std::cell::Cell::new(0),
        }
    }

    /// 仅供已确认不改变源码的展示命令使用；事务基线变化不重新语义编译。
    pub(super) fn rebase(&mut self, project: &Project, compiled: &CompileResult) {
        if self.root != project.root
            || compiled.sources != project.sources()
            || compiled.options != project.compile_options()
        {
            return;
        }
        self.baseline = project.content_baseline();
        self.projections.get_mut().clear();
    }

    fn ensure_current(&self, project: &Project) -> Result<(), String> {
        if self.root != project.root || self.baseline != project.content_baseline() {
            return Err("约束快照已过期；输入仍保留，请等待当前工程重建后重新预览。".into());
        }
        Ok(())
    }

    fn index(&self, project: &Project) -> Result<Arc<SchemaIndex>, String> {
        self.ensure_current(project)?;
        Ok(Arc::clone(&self.index))
    }

    fn projection(
        &self,
        project: &Project,
        compiled: &CompileResult,
        target: &TargetRef,
    ) -> Result<Arc<SchemaAuthoringProjection>, String> {
        self.ensure_current(project)?;
        let mut cache = self.projections.borrow_mut();
        if let Some(position) = cache.iter().position(|(key, _)| key == target) {
            let cached = cache.remove(position).expect("缓存位置来自同一队列");
            let result = cached.1.clone();
            cache.push_back(cached);
            return result;
        }
        drop(cache);
        #[cfg(test)]
        self.projection_builds.set(self.projection_builds.get() + 1);
        let result = project
            .schema_authoring_from_compile(compiled, target)
            .map(Arc::new);
        // 构建没有后台投递：不可变 Project 与完整 TargetRef 贯穿整个调用。
        // 旧快照不能发布到新工程，失败结果也按同一身份缓存，避免逐帧重试。
        let mut cache = self.projections.borrow_mut();
        if cache.len() == MAX_CACHED_PROJECTIONS {
            cache.pop_front();
        }
        cache.push_back((target.clone(), result.clone()));
        result
    }
}

impl WorldeditApp {
    pub(super) fn current_schema_index(&self) -> Result<Arc<SchemaIndex>, String> {
        self.snapshot
            .as_ref()
            .ok_or_else(|| "正在建立当前工程的约束快照；输入仍保留。".to_owned())?
            .schema
            .index(&self.project)
    }

    pub(super) fn current_schema_authoring(
        &self,
        target: &TargetRef,
    ) -> Result<SchemaAuthoringProjection, String> {
        self.current_schema_authoring_shared(target)
            .map(|projection| projection.as_ref().clone())
    }

    pub(super) fn current_schema_authoring_shared(
        &self,
        target: &TargetRef,
    ) -> Result<Arc<SchemaAuthoringProjection>, String> {
        let snapshot = self
            .snapshot
            .as_ref()
            .ok_or_else(|| "正在建立当前工程的约束快照；输入仍保留。".to_owned())?;
        snapshot
            .schema
            .projection(&self.project, &snapshot.result, target)
    }
}

#[cfg(test)]
mod tests;
