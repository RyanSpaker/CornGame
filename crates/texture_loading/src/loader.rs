//! CLAUDE (claude-opus-5-5): the image loader for png, jpg and ktx2, registered after Bevy's so it's chosen for
//! them (glTF textures included: it takes Bevy's `ImageLoaderSettings`, so glTF's sRGB flag and sampler apply).
//! It reads the format from the bytes, since a manifest texture's path names the original but it gets the low
//! file, and gives jpg/png a full mip chain.

use std::sync::Arc;

use bevy::{
    asset::{io::Reader, AssetLoader, LoadContext},
    image::{CompressedImageFormats, Image, ImageLoaderSettings, TextureError},
    log::warn,
    platform::time::Instant,
    reflect::TypePath,
    render::render_resource::TextureFormat,
};

use crate::{
    manifest::Role,
    mips,
    state::{image_type, key, Shared},
};

#[derive(TypePath)]
pub(crate) struct TextureLoader {
    pub formats: CompressedImageFormats,
    pub shared: Arc<Shared>,
}

#[derive(Debug, thiserror::Error)]
pub enum TextureLoadError {
    #[error("could not read: {0}")]
    Io(#[from] std::io::Error),
    #[error("{path}: {error}")]
    Decode { path: String, error: TextureError },
}

impl AssetLoader for TextureLoader {
    type Asset = Image;
    type Settings = ImageLoaderSettings;
    type Error = TextureLoadError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        settings: &ImageLoaderSettings,
        load_context: &mut LoadContext<'_>,
    ) -> Result<Image, Self::Error> {
        let key = key(load_context.path().path());
        let entry = self.shared.entry(&key);
        let redirected = entry.as_ref().is_some_and(|e| e.redirects(&key));
        let mut bytes = Vec::new();
        if let Err(e) = reader.read_to_end(&mut bytes).await {
            self.failed(redirected);
            return Err(e.into());
        }
        let served = entry.as_ref().filter(|_| redirected).map_or(key.as_str(), |e| e.low.as_str());
        let mut image = match Image::from_buffer(
            &bytes,
            image_type(&bytes, served),
            self.formats,
            settings.is_srgb,
            settings.sampler.clone(),
            settings.asset_usage,
        ) {
            Ok(image) => image,
            Err(error) => {
                self.failed(redirected);
                return Err(TextureLoadError::Decode { path: served.to_string(), error });
            }
        };
        if to_8_bit(&mut image, settings.is_srgb) {
            warn!(
                "texture_loading: {served} is 16 bits per channel; converted to 8 at load. \
                 Re-save it as 8-bit: 16-bit formats need TEXTURE_FORMAT_16BIT_NORM, which WebGL2 \
                 often lacks (Firefox), and a missing format kills the renderer"
            );
        }
        let role = entry.as_ref().map_or(Role::Color, |e| e.role);
        mips::generate(&mut image, role);
        if let Some(format) = settings.texture_format {
            image.texture_descriptor.format = format;
        }

        let mut log = self.shared.log.lock().unwrap();
        if redirected {
            log.low.done += 1;
            log.low.bytes += bytes.len() as u64;
            log.low.last = Some(Instant::now());
            log.describe(&key, &image);
        } else {
            log.direct.insert(key, bytes.len() as u64);
        }
        Ok(image)
    }

    fn extensions(&self) -> &[&str] {
        &["png", "jpg", "jpeg", "ktx2"]
    }
}

// CLAUDE (claude-opus-5-5): 16-bit PNGs decode to R/Rg/Rgba16Unorm, which WebGL2 needs EXT_texture_norm16
// for (Chromium has it, Firefox doesn't). PNG encodes 16-bit samples like 8-bit ones, so sRGB still applies.
/// narrow a 16-bit-unorm `image` to 8 bits per channel. Returns whether it did
fn to_8_bit(image: &mut Image, is_srgb: bool) -> bool {
    let format = match image.texture_descriptor.format {
        TextureFormat::R16Unorm => TextureFormat::R8Unorm,
        TextureFormat::Rg16Unorm => TextureFormat::Rg8Unorm,
        TextureFormat::Rgba16Unorm if is_srgb => TextureFormat::Rgba8UnormSrgb,
        TextureFormat::Rgba16Unorm => TextureFormat::Rgba8Unorm,
        _ => return false,
    };
    let Some(data) = image.data.as_mut() else { return false };
    // samples are native-endian u16 (Bevy casts the decoded buffer)
    *data = data
        .chunks_exact(2)
        .map(|b| ((u16::from_ne_bytes([b[0], b[1]]) as u32 * 255 + 32767) / 65535) as u8)
        .collect();
    image.texture_descriptor.format = format;
    true
}

impl TextureLoader {
    fn failed(&self, redirected: bool) {
        if redirected {
            self.shared.log.lock().unwrap().low.failed += 1;
        }
    }
}
