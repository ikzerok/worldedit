//! 只登记当前可见的起笔、编排与正文输入框；上层窗口不能继承书稿的键盘过滤。
#[derive(Clone, Default)]
struct InputRegistry {
    frame: u64,
    ids: std::collections::HashSet<egui::Id>,
}

fn key() -> egui::Id {
    egui::Id::new("manuscript-owned-text-inputs")
}

pub(in crate::app) fn register_input(response: &egui::Response) {
    let ctx = &response.ctx;
    let frame = ctx.cumulative_frame_nr();
    ctx.data_mut(|data| {
        let registry = data.get_temp_mut_or_default::<InputRegistry>(key());
        if registry.frame != frame {
            registry.frame = frame;
            registry.ids.clear();
        }
        registry.ids.insert(response.id);
    });
}

pub(super) fn owns_focus(ctx: &egui::Context) -> bool {
    let Some(id) = ctx.memory(|memory| memory.focused()) else {
        return false;
    };
    let frame = ctx.cumulative_frame_nr();
    ctx.data(|data| {
        data.get_temp::<InputRegistry>(key())
            .is_some_and(|registry| {
                registry.frame.saturating_add(1) >= frame && registry.ids.contains(&id)
            })
    })
}

pub(super) fn drawn_this_frame(ctx: &egui::Context, id: egui::Id) -> bool {
    let frame = ctx.cumulative_frame_nr();
    ctx.data(|data| {
        data.get_temp::<InputRegistry>(key())
            .is_some_and(|registry| registry.frame == frame && registry.ids.contains(&id))
    })
}
