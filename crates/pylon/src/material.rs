//! CLAUDE (claude-opus-5-5): one material type for every custom shader. The shader is data (a field),
//! so a new animated material is a `.wesl` file and no rust. Shaders read time from bevy's
//! `globals.time`, may sample `texture` (bindings 1 and 2), and declare their params as a struct of
//! `vec4<f32>`s at binding 0. `params` is by those field names, so the shader is where they're named:
//!
//! ```text
//! struct Chase { center: vec4<f32>, color: vec4<f32> }   // the shader
//! MeshMaterial3d<ShaderMaterial>: { fragment: "shaders/chase_ring.wesl"; params: { color: [1, 0.7, 0.3, 0] }; }
//! asset(handle, "ShaderMaterial").params.color.w = 0.5;  // a script
//! ```
//!
//! A first step to a runtime shader descriptor: the shader's inputs are read from the shader when it
//! loads ([`param_names`]); the GPU side is still 8 vec4 rows, in the struct's order.
//!
//! The shader goes in the pipeline key, so each shader gets its own pipeline, as separate types would.

use bevy::{
    asset::AssetEvent,
    mesh::MeshVertexBufferLayoutRef,
    pbr::{MaterialPipeline, MaterialPipelineKey},
    platform::collections::{HashMap, HashSet},
    prelude::*,
    render::render_resource::{AsBindGroup, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError},
    shader::{Shader, Source},
};

/// the rows of the params uniform
pub const MAX_PARAMS: usize = 8;

pub fn plugin(app: &mut App) {
    app.add_plugins(MaterialPlugin::<ShaderMaterial>::default())
        .register_type::<ShaderMaterial>()
        .register_asset_reflect::<ShaderMaterial>()
        .add_systems(PostUpdate, lay_out_params);
    corn_assets::register::<Shader>(app);
    corn_assets::register_inline::<ShaderMaterial>(app);
    crate::precompile::register::<ShaderMaterial>(app);
}

#[derive(Asset, Reflect, AsBindGroup, Clone, Debug, Default)]
#[reflect(Default)]
#[bind_group_data(ShaderKey)]
#[uniform(0, ParamRows)]
pub struct ShaderMaterial {
    /// the fragment shader (`.wesl`, bevy 0.20 only preprocesses wesl)
    pub fragment: Handle<Shader>,
    /// by the names of the shader's binding 0 struct fields. a missing one is zero
    pub params: HashMap<String, Vec4>,
    #[texture(1)]
    #[sampler(2)]
    pub texture: Option<Handle<Image>>,
    pub double_sided: bool,
    /// the shader's param names in order, once it has loaded (see [`lay_out_params`])
    #[reflect(ignore)]
    pub layout: Vec<String>,
}

/// `params` as the shader sees them
#[derive(ShaderType, Clone)]
pub struct ParamRows {
    rows: [Vec4; MAX_PARAMS],
}

impl From<&ShaderMaterial> for ParamRows {
    fn from(material: &ShaderMaterial) -> Self {
        let mut rows = [Vec4::ZERO; MAX_PARAMS];
        for (row, name) in rows.iter_mut().zip(&material.layout) {
            if let Some(value) = material.params.get(name) {
                *row = *value;
            }
        }
        Self { rows }
    }
}

#[derive(Eq, PartialEq, Hash, Clone)]
pub struct ShaderKey {
    fragment: Handle<Shader>,
    double_sided: bool,
}

impl From<&ShaderMaterial> for ShaderKey {
    fn from(material: &ShaderMaterial) -> Self {
        Self { fragment: material.fragment.clone(), double_sided: material.double_sided }
    }
}

impl Material for ShaderMaterial {
    fn specialize(
        _: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _: &MeshVertexBufferLayoutRef,
        key: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        if key.bind_group_data.double_sided {
            descriptor.primitive.cull_mode = None;
        }
        // prepass and shadow pipelines specialize through here too: they keep bevy's shaders
        if descriptor.label.as_deref() == Some("prepass_pipeline") {
            return Ok(());
        }
        if let Some(fragment) = descriptor.fragment.as_mut() {
            fragment.shader = key.bind_group_data.fragment.clone();
        }
        Ok(())
    }
}

// CLAUDE
/// gives a material its shader's param names (their order on the GPU) when the material is made or
/// changed, or its shader (re)loads. names the shader doesn't have are warned about
fn lay_out_params(
    mut shader_events: MessageReader<AssetEvent<Shader>>,
    mut material_events: MessageReader<AssetEvent<ShaderMaterial>>,
    shaders: Res<Assets<Shader>>,
    mut materials: ResMut<Assets<ShaderMaterial>>,
) {
    let loaded: HashSet<AssetId<Shader>> = shader_events
        .read()
        .filter_map(|e| match e {
            AssetEvent::LoadedWithDependencies { id } | AssetEvent::Modified { id } => Some(*id),
            _ => None,
        })
        .collect();
    let mut todo: HashSet<AssetId<ShaderMaterial>> = material_events
        .read()
        .filter_map(|e| match e {
            AssetEvent::Added { id } | AssetEvent::Modified { id } | AssetEvent::LoadedWithDependencies { id } => Some(*id),
            _ => None,
        })
        .collect();
    if !loaded.is_empty() {
        todo.extend(materials.iter().filter(|(_, m)| loaded.contains(&m.fragment.id())).map(|(id, _)| id));
    }
    for id in todo {
        let Some(material) = materials.get(id) else { continue };
        let Some(shader) = shaders.get(&material.fragment) else { continue };
        let (Source::Wgsl(source) | Source::Wesl(source)) = &shader.source else { continue };
        let layout = match param_names(source) {
            Ok(layout) => layout,
            Err(e) => {
                error!("{}: {e}", shader.path);
                continue;
            }
        };
        for name in material.params.keys().filter(|name| !layout.contains(name)) {
            warn!("{} has no param `{name}` (it has {})", shader.path, layout.join(", "));
        }
        // only when it differs: setting it changes the material, which brings it back here
        if material.layout != layout {
            materials.get_mut(id).unwrap().layout = layout;
        }
    }
}

// CLAUDE
/// the field names of the struct a shader declares at `@binding(0) var<uniform>`, in order. Each field
/// is one `vec4<f32>` row, at most [`MAX_PARAMS`]. The struct must be in the shader itself (not imported)
pub fn param_names(source: &str) -> Result<Vec<String>, String> {
    let code: String = source.lines().map(|line| line.split("//").next().unwrap_or("")).collect::<Vec<_>>().join("\n");
    let binding = code.find("@binding(0)").ok_or("no `@binding(0)` for params")?;
    let declaration = code[binding..].split(';').next().unwrap_or("");
    let ty = match declaration.contains("var<uniform>").then(|| declaration.rsplit_once(':')).flatten() {
        Some((_, ty)) => ty.trim(),
        None => return Err("binding 0 isn't a `var<uniform> name: Struct`".into()),
    };
    let body = code
        .match_indices("struct")
        .find_map(|(at, _)| code[at + "struct".len()..].trim_start().strip_prefix(ty)?.trim_start().strip_prefix('{'))
        .and_then(|rest| rest.split_once('}'))
        .map(|(body, _)| body)
        .ok_or_else(|| format!("no `struct {ty}` in the shader (params are read from the shader's own source)"))?;
    let mut names = vec![];
    for field in body.split(',').map(str::trim).filter(|f| !f.is_empty()) {
        let (name, field_ty) = field.split_once(':').ok_or_else(|| format!("`{field}` isn't a field"))?;
        let field_ty: String = field_ty.split_whitespace().collect();
        if field_ty != "vec4<f32>" && field_ty != "vec4f" {
            return Err(format!("param `{}` is a `{field_ty}`: params are `vec4<f32>`s", name.trim()));
        }
        names.push(name.trim().to_string());
    }
    if names.len() > MAX_PARAMS {
        return Err(format!("{} params, at most {MAX_PARAMS}", names.len()));
    }
    Ok(names)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn params_are_named_by_the_shader() {
        let names = param_names(include_str!("../assets/shaders/chase_ring.wesl")).unwrap();
        assert_eq!(names, ["center", "color", "timing", "envelope"]);
        for shader in [include_str!("../assets/shaders/window.wesl"), include_str!("../assets/shaders/portal.wesl")] {
            param_names(shader).unwrap();
        }
        let material = ShaderMaterial {
            params: [("timing".to_string(), Vec4::ONE), ("nope".to_string(), Vec4::ONE)].into_iter().collect(),
            layout: names,
            ..default()
        };
        let rows = ParamRows::from(&material).rows;
        assert_eq!((rows[0], rows[2]), (Vec4::ZERO, Vec4::ONE));

        let bad = |src: &str| param_names(src).unwrap_err();
        assert!(bad("@binding(0) var<uniform> p: array<vec4<f32>, 8>;").contains("no `struct"));
        assert!(bad("struct P { a: f32 }\n@binding(0) var<uniform> p: P;").contains("params are `vec4<f32>`s"));
        assert!(bad("@binding(0) var t: texture_2d<f32>;").contains("isn't a `var<uniform>"));
    }
}
