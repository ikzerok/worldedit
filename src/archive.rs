//! 浏览器工程包：保留目录与二进制素材，限制展开大小，拒绝越界路径。
use std::collections::BTreeMap;
use std::path::PathBuf;

mod browser;
mod codec;
mod paths;
#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) use browser::DecodedBrowserSnapshot;
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) use browser::{
    decode_browser_recovery_bundle, decode_browser_snapshot, encode_browser_recovery_bundle,
    encode_browser_snapshot,
};
#[cfg(any(target_arch = "wasm32", test))]
pub use codec::prepare_import;
pub use codec::{decode, encode};
#[cfg(any(target_arch = "wasm32", test))]
pub use paths::relative_path;
pub use paths::{ensure_legacy_manifest, entry, validate_files};

pub type Files = BTreeMap<PathBuf, Vec<u8>>;
pub const MAX_BYTES: u64 = 64 * 1024 * 1024;
const MAX_FILES: usize = 4096;
pub const MANIFEST: &str = "worldedit-project.json";
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const MAX_BROWSER_CHECKPOINT_SNAPSHOT_BASE64: usize = 4 * 1024 * 1024;
#[cfg(any(target_arch = "wasm32", test))]
const MAX_BROWSER_SNAPSHOT_ENVELOPE_BYTES: usize =
    MAX_BYTES as usize * 4 / 3 + MAX_BROWSER_CHECKPOINT_SNAPSHOT_BASE64 + 16 * 1024;
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) const MAX_BROWSER_RECOVERY_BYTES: u64 =
    MAX_BYTES + (MAX_BROWSER_CHECKPOINT_SNAPSHOT_BASE64 as u64 * 3 / 4) + 1024 * 1024;
#[cfg(any(target_arch = "wasm32", test))]
const RECOVERY_MANIFEST: &str = "worldedit-recovery.json";
#[cfg(any(target_arch = "wasm32", test))]
const RECOVERY_PROJECT: &str = "worldedit-project.zip";
#[cfg(any(target_arch = "wasm32", test))]
const RECOVERY_CHECKPOINTS: &str = "worldedit-checkpoints.bin";
