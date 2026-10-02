//! 所有 scene 图层与 SVG 预览共享的逻辑 RGBA 预算及实际执行许可。
//! 不计 resvg 树、字体和驱动延迟回收；渲染后端另检查隔离表面峰值。
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

const MAX_ACTIVE: usize = 2;
const WORK_COPIES: usize = 3;
const CACHE_COPIES: usize = 2;

#[derive(Clone)]
pub(super) struct RenderBudget(Arc<Mutex<BudgetStats>>);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct BudgetStats {
    pub(super) limit: usize,
    pub(super) cached: usize,
    pub(super) reserved: usize,
    pub(super) active: usize,
    pub(super) generation: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AdmissionError {
    InvalidDimensions,
    Oversized { required: usize, limit: usize },
    Waiting { required: usize, stats: BudgetStats },
    Capacity { required: usize, stats: BudgetStats },
}

impl AdmissionError {
    pub(super) fn message(self, layer: &str) -> String {
        match self {
            Self::InvalidDimensions => format!("图层「{layer}」的视口尺寸无效或字节数溢出"),
            Self::Oversized { required, limit } => format!(
                "图层「{layer}」当前视口需预留 {required} bytes，超过 scene RGBA 预算 {limit} bytes；请缩小窗口，不会降低 DPI"
            ),
            Self::Waiting { required, stats } => format!(
                "图层「{layer}」排队等待：{} 个任务执行中，待预留 {required} bytes（已占 {} / {} bytes）",
                stats.active, stats.cached + stats.reserved, stats.limit
            ),
            Self::Capacity { required, stats } => format!(
                "图层「{layer}」资源不足：需 {required} bytes，当前可见缓存 {} / {} bytes；请临时隐藏图层或缩小窗口，该层尚未完成显示",
                stats.cached, stats.limit
            ),
        }
    }
}

struct PermitState {
    running: bool,
    settled: bool,
}

/// 此账户须与结果像素一起存活，直到上传/丢弃；取消 UI 不提前归还。
pub(super) struct RenderPermit {
    budget: RenderBudget,
    reservation: usize,
    output_limit: usize,
    state: Mutex<PermitState>,
}

/// 单独由实际 native 线程或 Worker 生命周期持有，不能由 UI 提前结束。
pub(super) struct ExecutionGuard(Arc<RenderPermit>);

/// 两份逻辑 RGBA：纹理及上传数据。不声称驱动立即释放物理内存。
pub(super) struct TextureLease {
    budget: RenderBudget,
    bytes: usize,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl RenderBudget {
    pub(super) fn shared() -> Self {
        static SHARED: OnceLock<RenderBudget> = OnceLock::new();
        SHARED
            .get_or_init(|| {
                let mib = if cfg!(target_arch = "wasm32") {
                    64
                } else {
                    128
                };
                Self::with_limit(mib * 1024 * 1024)
            })
            .clone()
    }

    pub(super) fn with_limit(limit: usize) -> Self {
        Self(Arc::new(Mutex::new(BudgetStats {
            limit,
            cached: 0,
            reserved: 0,
            active: 0,
            generation: 0,
        })))
    }

    pub(super) fn stats(&self) -> BudgetStats {
        *lock(&self.0)
    }

    pub(super) fn start(
        &self,
        size: [u32; 2],
    ) -> Result<(Arc<RenderPermit>, ExecutionGuard), AdmissionError> {
        let output_limit = rgba_bytes(size).ok_or(AdmissionError::InvalidDimensions)?;
        let required = output_limit
            .checked_mul(WORK_COPIES)
            .ok_or(AdmissionError::InvalidDimensions)?;
        let mut stats = lock(&self.0);
        if required > stats.limit {
            return Err(AdmissionError::Oversized {
                required,
                limit: stats.limit,
            });
        }
        let available = stats.limit - stats.cached - stats.reserved;
        if stats.active >= MAX_ACTIVE || required > available {
            // 已完成但尚在结果通道中的像素仍持 reservation；它们也会释放。
            return Err(if stats.active > 0 || stats.reserved > 0 {
                AdmissionError::Waiting {
                    required,
                    stats: *stats,
                }
            } else {
                AdmissionError::Capacity {
                    required,
                    stats: *stats,
                }
            });
        }
        stats.active += 1;
        stats.reserved += required;
        stats.generation = stats.generation.wrapping_add(1);
        let permit = Arc::new(RenderPermit {
            budget: self.clone(),
            reservation: required,
            output_limit,
            state: Mutex::new(PermitState {
                running: true,
                settled: false,
            }),
        });
        Ok((permit.clone(), ExecutionGuard(permit)))
    }
}

pub(super) fn rgba_bytes([width, height]: [u32; 2]) -> Option<usize> {
    if width == 0 || height == 0 {
        return None;
    }
    usize::try_from(width)
        .ok()?
        .checked_mul(usize::try_from(height).ok()?)?
        .checked_mul(4)
}

impl RenderPermit {
    /// 必须先停止实际执行，再于所有临时原始像素副本释放后转换缓存费用。
    pub(super) fn cache(&self, rgba_bytes: usize) -> Result<TextureLease, String> {
        let mut state = lock(&self.state);
        if state.running || state.settled {
            return Err("渲染任务尚未结束或资源账户已结算".into());
        }
        if rgba_bytes > self.output_limit || !rgba_bytes.is_multiple_of(4) {
            return Err("渲染返回的 RGBA 字节数超出请求范围".into());
        }
        let bytes = rgba_bytes
            .checked_mul(CACHE_COPIES)
            .ok_or_else(|| "纹理字节数溢出".to_owned())?;
        if bytes > self.reservation {
            return Err("纹理缓存超出预留额度".into());
        }
        let mut stats = lock(&self.budget.0);
        stats.reserved -= self.reservation;
        stats.cached += bytes;
        stats.generation = stats.generation.wrapping_add(1);
        state.settled = true;
        Ok(TextureLease {
            budget: self.budget.clone(),
            bytes,
        })
    }
}

impl Drop for ExecutionGuard {
    fn drop(&mut self) {
        let mut state = lock(&self.0.state);
        if state.running {
            state.running = false;
            let mut stats = lock(&self.0.budget.0);
            stats.active -= 1;
            stats.generation = stats.generation.wrapping_add(1);
        }
    }
}

impl Drop for RenderPermit {
    fn drop(&mut self) {
        if !lock(&self.state).settled {
            let mut stats = lock(&self.budget.0);
            stats.reserved -= self.reservation;
            stats.generation = stats.generation.wrapping_add(1);
        }
    }
}

impl Drop for TextureLease {
    fn drop(&mut self) {
        let mut stats = lock(&self.budget.0);
        stats.cached -= self.bytes;
        stats.generation = stats.generation.wrapping_add(1);
    }
}

#[cfg(test)]
#[path = "tests/render_budget.rs"]
mod tests;
