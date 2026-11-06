//! CODEX (GPT-6): isolated paper shader extension; readability assistance uses actual lit brightness.
use bevy::{
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
};

// CODEX: retain all authored PBR properties while adding a forward-pass readability adjustment.
pub(crate) type PaperMaterial = ExtendedMaterial<StandardMaterial, PaperGlow>;

// CODEX: vec4 uniforms keep the Rust/WESL layout portable, including browser renderers.
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug, PartialEq)]
pub(crate) struct PaperGlow {
    // CODEX: RGB is the maximum exposure-independent boost; W enables it only during editing.
    #[uniform(100)]
    pub glow: Vec4,
    // CODEX: X is the full-assistance threshold, Y the soft fade width, Z the assisted brightness ceiling.
    #[uniform(101)]
    pub darkness: Vec4,
}

// CODEX: emission starts disabled for held notes, before text entry begins.
impl Default for PaperGlow {
    // CODEX: actual live tuning is supplied from PaperSettings by editor_glow.
    fn default() -> Self {
        Self { glow: Vec4::ZERO, darkness: Vec4::new(0.001, 0.002, 0.0046, 0.0) }
    }
}

// CODEX: use the standard vertex/shadow paths and change only forward fragment lighting.
impl MaterialExtension for PaperGlow {
    // CODEX: forward rendering is required to inspect the completed per-pixel lighting result.
    fn fragment_shader() -> ShaderRef { "shaders/post_it.wesl".into() }
}

// CODEX: white RGB carries only glyph coverage; the authored ink material supplies the navy color.
// This prevents multiplying two dark colors together and making the copied ink almost black.
pub(crate) fn ink_mask(mut image: Image) -> Image {
    if let Some(pixels) = image.data.as_mut() {
        for pixel in pixels.chunks_exact_mut(4) { pixel[..3].fill(255); }
    }
    image
}

// CLAUDE: the held paper's material from an authored sheet material. Shared by pickup (copy_paper) and the
// pipeline warm-up (post_it_warmup.rs), so both request the very same pipelines.
pub(crate) fn paper_material(mut base: StandardMaterial) -> PaperMaterial {
    // CODEX: force forward lighting so the shader can inspect the lit pixel before postprocessing.
    base.opaque_render_method = bevy::material::OpaqueRendererMethod::Forward;
    PaperMaterial { base, extension: PaperGlow::default() }
}

// CLAUDE: typed-ink material from the authored handwriting material, shared by render_text and the warm-up.
// The authored glyphs are geometry; typed glyphs instead need an alpha coverage mask.
pub(crate) fn ink_material(mut material: StandardMaterial, image: Handle<Image>) -> StandardMaterial {
    material.base_color_texture = Some(image);
    material.alpha_mode = AlphaMode::Blend;
    material
}
