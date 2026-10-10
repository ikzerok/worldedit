//! 范围投影缓存只复用查询，范围修改仍逐次检查代次与原文字节。
use super::*;
use std::sync::Arc;
use worldline_core::manuscript::WritingProjection;

#[derive(Default)]
pub(super) struct ProjectionCache {
    key: String,
    projection: Option<Result<Arc<WritingProjection>, String>>,
    #[cfg(test)]
    pub(super) builds: usize,
}

impl ViewState {
    pub(in crate::app) fn invalidate_projection(&mut self) {
        self.dialogue.invalidate();
        self.projection_cache.key.clear();
        self.projection_cache.projection = None;
    }
}

impl ProjectionCache {
    pub(super) fn get(
        &mut self,
        project: &Project,
        buffer: &WritingBuffer,
        target: &TargetRef,
    ) -> Result<Arc<WritingProjection>, String> {
        let key = format!(
            "{}|{}:{}",
            project.manuscript_query_key(std::slice::from_ref(buffer), &[]),
            target.kind,
            target.id
        );
        if self.key != key || self.projection.is_none() {
            self.projection = Some(project.project_writing_buffer(buffer, target).map(Arc::new));
            self.key = key;
            #[cfg(test)]
            {
                self.builds += 1;
            }
        }
        self.projection
            .as_ref()
            .expect("已缓存正文投影结果")
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn writing_projection_cache_tracks_every_buffer_generation_and_project_change() {
        let mut project = Project::new(std::path::Path::new("/writing-cache-test"));
        let path = project.entry.clone();
        project
            .set_text(&path, "event start\n  第一段。\n  -> END\n".into())
            .unwrap();
        let target = TargetRef::new("event", "start");
        let mut buffer = project.open_writing_buffer(&target).unwrap();
        let mut cache = ProjectionCache::default();
        let first = cache.get(&project, &buffer, &target).unwrap();
        for _ in 0..12 {
            assert!(Arc::ptr_eq(
                &first,
                &cache.get(&project, &buffer, &target).unwrap()
            ));
        }
        assert_eq!(cache.builds, 1);
        buffer.replace_source(buffer.source().replace("第一段", "当前未应用正文"));
        let next = cache.get(&project, &buffer, &target).unwrap();
        assert_ne!(first.generation, next.generation);
        assert_eq!(cache.builds, 2);
        project
            .set_text(&path, "event start\n  外部载入新稿。\n  -> END\n".into())
            .unwrap();
        let baseline = project.content_baseline();
        let retained_text = buffer.source().to_owned();
        let retained_projection = cache.get(&project, &buffer, &target).unwrap();
        assert!(
            !Arc::ptr_eq(&next, &retained_projection),
            "Project变化必须重新执行core投影"
        );
        assert!(retained_projection.source.contains("当前未应用正文"));
        assert!(!retained_projection.source.contains("外部载入新稿"));
        assert_eq!(cache.builds, 3);
        assert!(Arc::ptr_eq(
            &retained_projection,
            &cache.get(&project, &buffer, &target).unwrap()
        ));
        assert_eq!(cache.builds, 3);
        // 只读查看保留稿不授权覆盖较新工程；两种应用路径都必须拒绝。
        assert!(project
            .preview_writing_buffer(&buffer)
            .unwrap_err()
            .contains("过期"));
        assert!(project
            .apply_writing_buffer(&buffer)
            .unwrap_err()
            .contains("过期"));
        assert!(project
            .apply_source_writing_buffer(&buffer)
            .unwrap_err()
            .contains("过期"));
        assert_eq!(project.content_baseline(), baseline);
        assert_eq!(buffer.source(), retained_text);
        assert!(project.document(&path).unwrap().contains("外部载入新稿"));
    }
}
