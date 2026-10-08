//! 缓存只读浏览快照；提交继续直接使用 Project 的完整预检。
use std::sync::Arc;
use worldline_core::{
    manuscript::{ManuscriptQueryDraft, ManuscriptQuerySnapshot, WritingBuffer},
    project::Project,
};

#[derive(Default)]
struct CachedSnapshot {
    key: String,
    value: Option<Result<Arc<ManuscriptQuerySnapshot>, String>>,
    #[cfg(test)]
    builds: usize,
}

impl CachedSnapshot {
    fn get(
        &mut self,
        project: &Project,
        buffers: &[&WritingBuffer],
        drafts: &[ManuscriptQueryDraft],
    ) -> Result<Arc<ManuscriptQuerySnapshot>, String> {
        let key = project.manuscript_query_key_refs(buffers.iter().copied(), drafts);
        if self.key != key || self.value.is_none() {
            let owned: Vec<_> = buffers.iter().copied().cloned().collect();
            self.value = Some(
                project
                    .manuscript_query_snapshot(&owned, drafts)
                    .map(Arc::new)
                    .map_err(|error| error.to_string()),
            );
            self.key = key;
            #[cfg(test)]
            {
                self.builds += 1;
            }
        }
        self.value.as_ref().expect("已缓存只读快照结果").clone()
    }
}

#[derive(Default)]
pub(super) struct QueryCache {
    applied: CachedSnapshot,
    current: CachedSnapshot,
}

impl QueryCache {
    pub(super) fn invalidate(&mut self) {
        self.applied.key.clear();
        self.applied.value = None;
        self.current.key.clear();
        self.current.value = None;
    }
    pub(super) fn applied(
        &mut self,
        project: &Project,
    ) -> Result<Arc<ManuscriptQuerySnapshot>, String> {
        self.applied.get(project, &[], &[])
    }

    pub(super) fn current<'a>(
        &mut self,
        project: &Project,
        buffers: impl IntoIterator<Item = &'a WritingBuffer>,
        drafts: &[ManuscriptQueryDraft],
    ) -> Result<Arc<ManuscriptQuerySnapshot>, String> {
        let buffers: Vec<_> = buffers
            .into_iter()
            .filter(|buffer| buffer.is_changed())
            .collect();
        if buffers.is_empty() && drafts.is_empty() {
            self.applied(project)
        } else {
            self.current.get(project, &buffers, drafts)
        }
    }

    #[cfg(test)]
    pub(super) fn builds(&self) -> usize {
        self.applied.builds + self.current.builds
    }
}
