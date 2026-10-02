//! 对 resvg 0.48.1 的实际离屏 Pixmap / alpha mask 做保守峰值预检。
//! 依据官方 render.rs、clip.rs、lib.rs；后端升级时必须重新审计。
//! 不估算树/字体/路径扫描器等非表面内存，不以作者 extra 声称作为许可。
use resvg::{tiny_skia::Transform, usvg};

pub(super) fn preflight(
    tree: &usvg::Tree,
    transform: Transform,
    viewport: [u32; 2],
) -> Result<usize, String> {
    let limit = if cfg!(target_arch = "wasm32") { 32 } else { 64 } * 1024 * 1024;
    peak_with_limit(tree, transform, viewport, limit)
}

pub(super) fn peak_with_limit(
    tree: &usvg::Tree,
    transform: Transform,
    viewport: [u32; 2],
    limit: usize,
) -> Result<usize, String> {
    if !tree.filters().is_empty()
        || !tree.masks().is_empty()
        || !tree.patterns().is_empty()
        || !tree.linear_gradients().is_empty()
        || !tree.radial_gradients().is_empty()
    {
        return Err("渲染树包含不在受控 scene 表面预算内的效果".into());
    }
    // resvg::render 直接绘制 root.children，不为 tree.root 自身另建表面。
    children_peak(tree.root(), transform, viewport, limit, 0)
}

fn children_peak(
    parent: &usvg::Group,
    transform: Transform,
    viewport: [u32; 2],
    limit: usize,
    depth: usize,
) -> Result<usize, String> {
    if depth > 128 {
        return Err("渲染树过深，无法证明内部表面峰值".into());
    }
    let mut peak = 0;
    for node in parent.children() {
        let bytes = match node {
            usvg::Node::Group(group) => group_peak(group, transform, viewport, limit, depth + 1)?,
            usvg::Node::Text(text) => {
                group_peak(text.flattened(), transform, viewport, limit, depth + 1)?
            }
            usvg::Node::Path(_) => 0,
            usvg::Node::Image(_) => return Err("scene 渲染不允许外部或内嵌图像节点".into()),
        };
        peak = peak.max(bytes);
    }
    Ok(peak)
}

fn group_peak(
    group: &usvg::Group,
    parent: Transform,
    viewport: [u32; 2],
    limit: usize,
    depth: usize,
) -> Result<usize, String> {
    if group.mask().is_some() || !group.filters().is_empty() {
        return Err("scene 渲染不允许 mask/filter 离屏路径".into());
    }
    let transform = parent.pre_concat(group.transform());
    let child = children_peak(group, transform, viewport, limit, depth)?;
    if !group.should_isolate() {
        return Ok(child);
    }
    let bbox = group
        .layer_bounding_box()
        .transform(transform)
        .ok_or_else(|| "隔离组变换超出可表示的渲染范围".to_owned())?;
    // resvg 扩展抗锯齿边界后，限定到宽/高分别最多 viewport 的 5 倍。
    // 父隔离层的平移会影响交集位置，但不会扩大此处计算的宽高上界。
    let width = (f64::from(bbox.width()).ceil() + 4.0).min(f64::from(viewport[0]) * 5.0);
    let height = (f64::from(bbox.height()).ceil() + 4.0).min(f64::from(viewport[1]) * 5.0);
    if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
        return Err("隔离组表面尺寸无效".into());
    }
    let pixels = (width as u64)
        .checked_mul(height as u64)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| "隔离组表面字节数溢出".to_owned())?;
    let surface = pixels
        .checked_mul(4)
        .ok_or_else(|| "隔离组 RGBA 字节数溢出".to_owned())?;
    let clip_work = if let Some(clip) = group.clip_path() {
        check_rect_clip_topology(clip)?;
        // clip::apply 同时持有一个同尺寸 RGBA 和一个 alpha mask。
        pixels
            .checked_mul(5)
            .ok_or_else(|| "裁剪表面字节数溢出".to_owned())?
    } else {
        0
    };
    // 子节点绘制结束后才 apply clip，因此二者取 max；本组表面始终存活。
    let peak = surface
        .checked_add(child.max(clip_work))
        .ok_or_else(|| "嵌套表面峰值溢出".to_owned())?;
    if peak > limit {
        return Err(format!(
            "scene 内部合成表面预计需 {peak} bytes，超过单任务 {limit} bytes；请减少同时可见的复杂组或缩小窗口，不会降低 DPI"
        ));
    }
    Ok(peak)
}

fn check_rect_clip_topology(clip: &usvg::ClipPath) -> Result<(), String> {
    // 安全 serializer 的矩形 clip 经 usvg 后仅产生一个 path。
    // 不接受 resvg 可能额外分配未计费表面的 chained/nested clip。
    if clip.clip_path().is_some()
        || clip.root().children().len() != 1
        || !matches!(clip.root().children().first(), Some(usvg::Node::Path(_)))
    {
        return Err("无法证明该裁剪渲染树的表面上界".into());
    }
    Ok(())
}
