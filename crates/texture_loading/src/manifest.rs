//! CLAUDE (claude-opus-5-5): the texture set files tools/ktx2_convert writes: `<sets>/index.json` lists the sets
//! in order of preference, `<sets>/<set>/manifest.json` maps asset paths to their low and high files.

use std::collections::HashMap;

use bevy::image::CompressedImageFormats;
use serde::Deserialize;

#[derive(Deserialize)]
pub(crate) struct Index {
    pub sets: Vec<IndexSet>,
}

#[derive(Deserialize)]
pub(crate) struct IndexSet {
    pub name: String,
    /// GPU support the set needs: "bc", "astc", or none (jpg/png)
    pub requires: Option<String>,
}

impl IndexSet {
    pub fn supported(&self, gpu: CompressedImageFormats) -> bool {
        match self.requires.as_deref() {
            None => true,
            Some("bc") => gpu.contains(CompressedImageFormats::BC),
            Some("astc") => gpu.contains(CompressedImageFormats::ASTC_LDR),
            Some("etc2") => gpu.contains(CompressedImageFormats::ETC2),
            Some(_) => false,
        }
    }
}

#[derive(Deserialize)]
pub struct Manifest {
    pub set: String,
    /// largest side of a low file
    pub split: u32,
    /// asset path (relative to the assets root) -> its files
    pub images: HashMap<String, Entry>,
}

/// how a texture is sampled: decides how generated mips are filtered
#[derive(Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// filtered in linear light when the image is sRGB
    #[default]
    Color,
    Linear,
    /// filtered as vectors, renormalized
    Normal,
}

#[derive(Deserialize, Clone, Debug)]
pub struct Entry {
    #[serde(default)]
    pub role: Role,
    /// loaded first, in the original's place; the original's own path means the original is loaded
    pub low: String,
    #[serde(default)]
    pub low_bytes: u64,
    /// loaded once every low file is in, then replaces the low image; none: the low file is all there is
    pub high: Option<String>,
    #[serde(default)]
    pub high_bytes: u64,
    /// load the original itself, as if it weren't in the manifest
    #[serde(default)]
    pub original: bool,
}

impl Entry {
    /// a read of `key` gets another file
    pub fn redirects(&self, key: &str) -> bool {
        !self.original && self.low != key
    }
}
