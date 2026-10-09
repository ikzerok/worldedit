//! 隔离试演的有状态后台宿主；原生线程与浏览器Worker共用一份真实引擎。
#[cfg(target_arch = "wasm32")]
mod browser;
mod engine;
#[cfg(target_arch = "wasm32")]
mod execute;
#[cfg(not(target_arch = "wasm32"))]
mod native;
mod protocol;
#[cfg(target_arch = "wasm32")]
pub(crate) use browser::SessionWorker;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use native::SessionWorker;
pub(crate) use protocol::*;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod localization_tests;
