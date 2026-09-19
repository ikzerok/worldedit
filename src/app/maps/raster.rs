#![allow(dead_code)]

use egui::{ColorImage, TextureHandle, TextureOptions};
use image::{ImageFormat, ImageReader};
use std::collections::HashMap;
use std::collections::VecDeque;
use std::io::Cursor;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RasterDecodeConfig {
    pub(super) max_width: u32,
    pub(super) max_height: u32,
    pub(super) max_pixels: u64,
    pub(super) max_decoded_bytes: usize,
    pub(super) texture_budget_bytes: usize,
}

impl RasterDecodeConfig {
    pub const fn platform() -> Self {
        #[cfg(target_arch = "wasm32")]
        {
            Self {
                max_width: 2048,
                max_height: 2048,
                max_pixels: 4_194_304,
                max_decoded_bytes: 64 * 1024 * 1024,
                texture_budget_bytes: 64 * 1024 * 1024,
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            Self {
                max_width: 4096,
                max_height: 4096,
                max_pixels: 16_777_216,
                max_decoded_bytes: 128 * 1024 * 1024,
                texture_budget_bytes: 128 * 1024 * 1024,
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodedRaster {
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) rgba: Vec<u8>,
}

impl DecodedRaster {
    pub fn bytes(&self) -> usize {
        self.rgba.len()
    }

    pub fn color_image(&self) -> ColorImage {
        ColorImage::from_rgba_unmultiplied([self.width as usize, self.height as usize], &self.rgba)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RasterError {
    UnsupportedFormat,
    InvalidImage(String),
    DimensionsExceeded { width: u32, height: u32 },
    PixelsExceeded { width: u32, height: u32 },
    AllocationExceeded { bytes: usize },
}

pub fn decode_rgba(bytes: &[u8], config: RasterDecodeConfig) -> Result<DecodedRaster, RasterError> {
    let probe = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|error| RasterError::InvalidImage(error.to_string()))?;
    if !matches!(probe.format(), Some(ImageFormat::Png | ImageFormat::Jpeg)) {
        return Err(RasterError::UnsupportedFormat);
    }
    let (width, height) = probe
        .into_dimensions()
        .map_err(|error| RasterError::InvalidImage(error.to_string()))?;
    validate_dimensions(width, height, config)?;

    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|error| RasterError::InvalidImage(error.to_string()))?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(config.max_width);
    limits.max_image_height = Some(config.max_height);
    limits.max_alloc = Some((config.max_decoded_bytes as u64).saturating_add(bytes.len() as u64));
    reader.limits(limits);
    let image = reader
        .decode()
        .map_err(|error| RasterError::InvalidImage(error.to_string()))?;
    let rgba = image.into_rgba8();
    let (decoded_width, decoded_height) = rgba.dimensions();
    validate_dimensions(decoded_width, decoded_height, config)?;
    let rgba = rgba.into_raw();
    if rgba.len() > config.max_decoded_bytes {
        return Err(RasterError::AllocationExceeded { bytes: rgba.len() });
    }
    Ok(DecodedRaster {
        width: decoded_width,
        height: decoded_height,
        rgba,
    })
}

fn validate_dimensions(
    width: u32,
    height: u32,
    config: RasterDecodeConfig,
) -> Result<(), RasterError> {
    if width > config.max_width || height > config.max_height {
        return Err(RasterError::DimensionsExceeded { width, height });
    }
    let pixels = u64::from(width).saturating_mul(u64::from(height));
    if pixels > config.max_pixels {
        return Err(RasterError::PixelsExceeded { width, height });
    }
    let bytes = pixels.saturating_mul(4);
    if bytes > config.max_decoded_bytes as u64 {
        return Err(RasterError::AllocationExceeded {
            bytes: usize::try_from(bytes).unwrap_or(usize::MAX),
        });
    }
    Ok(())
}

#[derive(Default)]
pub struct RasterTextureCache {
    textures: HashMap<String, TextureHandle>,
    bytes: HashMap<String, usize>,
    order: VecDeque<String>,
    used_bytes: usize,
    budget_bytes: usize,
}

impl RasterTextureCache {
    pub fn with_budget(budget_bytes: usize) -> Self {
        Self {
            budget_bytes,
            ..Self::default()
        }
    }

    pub fn insert(
        &mut self,
        ctx: &egui::Context,
        key: impl Into<String>,
        raster: &DecodedRaster,
    ) -> bool {
        let key = key.into();
        let bytes = raster.bytes();
        if bytes > self.budget_bytes {
            return false;
        }
        if let Some(previous) = self.bytes.remove(&key) {
            self.used_bytes = self.used_bytes.saturating_sub(previous);
            self.textures.remove(&key);
            self.order.retain(|entry| entry != &key);
        }
        while self.used_bytes.saturating_add(bytes) > self.budget_bytes {
            let Some(oldest) = self.order.pop_front() else {
                break;
            };
            if let Some(previous) = self.bytes.remove(&oldest) {
                self.used_bytes = self.used_bytes.saturating_sub(previous);
            }
            self.textures.remove(&oldest);
        }
        if self.used_bytes.saturating_add(bytes) > self.budget_bytes {
            return false;
        }
        let texture = ctx.load_texture(
            format!("worldedit-map-{key}"),
            raster.color_image(),
            TextureOptions::LINEAR,
        );
        self.used_bytes = self.used_bytes.saturating_add(bytes);
        self.bytes.insert(key.clone(), bytes);
        self.textures.insert(key.clone(), texture);
        self.order.push_back(key);
        true
    }

    pub fn get(&mut self, key: &str) -> Option<TextureHandle> {
        let texture = self.textures.get(key).cloned();
        if texture.is_some() {
            self.order.retain(|entry| entry != key);
            self.order.push_back(key.to_string());
        }
        texture
    }

    pub fn contains(&self, key: &str) -> bool {
        self.textures.contains_key(key)
    }

    pub fn clear(&mut self) {
        self.textures.clear();
        self.bytes.clear();
        self.order.clear();
        self.used_bytes = 0;
    }

    pub fn used_bytes(&self) -> usize {
        self.used_bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ONE_PIXEL_PNG: &[u8] = &[
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6,
        0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120, 156, 99, 248, 207, 192, 240,
        31, 0, 5, 0, 1, 255, 137, 153, 61, 29, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
    ];

    #[test]
    fn decodes_png_and_rejects_non_image_bytes() {
        let config = RasterDecodeConfig {
            max_width: 1,
            max_height: 1,
            max_pixels: 1,
            max_decoded_bytes: 4,
            texture_budget_bytes: 4,
        };
        let decoded = decode_rgba(ONE_PIXEL_PNG, config).expect("one pixel PNG is valid");
        assert_eq!((decoded.width, decoded.height), (1, 1));
        assert_eq!(decoded.rgba.len(), 4);
        assert_eq!(
            decode_rgba(b"GIF89a", config),
            Err(RasterError::UnsupportedFormat)
        );
    }

    #[test]
    fn decodes_jpeg_with_the_same_budget_path() {
        let mut jpeg = Vec::new();
        image::codecs::jpeg::JpegEncoder::new(&mut jpeg)
            .encode(&[220, 90, 40], 1, 1, image::ExtendedColorType::Rgb8)
            .expect("test JPEG encoding should succeed");
        let config = RasterDecodeConfig {
            max_width: 1,
            max_height: 1,
            max_pixels: 1,
            max_decoded_bytes: 4,
            texture_budget_bytes: 4,
        };
        let decoded = decode_rgba(&jpeg, config).expect("one pixel JPEG is valid");
        assert_eq!((decoded.width, decoded.height), (1, 1));
        assert_eq!(decoded.rgba.len(), 4);
    }

    #[test]
    fn checks_pixel_budget_before_rgba_allocation() {
        let config = RasterDecodeConfig {
            max_width: 4096,
            max_height: 4096,
            max_pixels: 0,
            max_decoded_bytes: 128,
            texture_budget_bytes: 128,
        };
        assert!(matches!(
            decode_rgba(ONE_PIXEL_PNG, config),
            Err(RasterError::PixelsExceeded {
                width: 1,
                height: 1
            })
        ));
    }

    #[test]
    fn cache_rejects_texture_larger_than_budget() {
        let ctx = egui::Context::default();
        let raster = DecodedRaster {
            width: 1,
            height: 1,
            rgba: vec![0, 0, 0, 255],
        };
        let mut cache = RasterTextureCache::with_budget(3);
        assert!(!cache.insert(&ctx, "too-large", &raster));
        assert_eq!(cache.used_bytes(), 0);
    }

    #[test]
    fn cache_evicts_the_least_recently_used_texture() {
        let ctx = egui::Context::default();
        let raster = DecodedRaster {
            width: 1,
            height: 1,
            rgba: vec![0, 0, 0, 255],
        };
        let mut cache = RasterTextureCache::with_budget(8);
        assert!(cache.insert(&ctx, "first", &raster));
        assert!(cache.insert(&ctx, "second", &raster));
        assert!(cache.get("first").is_some());
        assert!(cache.insert(&ctx, "third", &raster));
        assert!(cache.get("first").is_some());
        assert!(cache.get("second").is_none());
        assert!(cache.get("third").is_some());
    }
}
