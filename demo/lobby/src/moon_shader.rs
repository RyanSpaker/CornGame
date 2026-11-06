//! CLAUDE (claude-sonnet-5-5) 2026-11-05: the moon is lit by the Sun inside its own material, not by a light.
//!
//! WHY: WebGL2 builds of Bevy allow ONE directional light per view and ONE shadow-map layer shared by
//! directional and spot lights. The lobby has two directional lights: the moonlight (lights the cabin)
//! and the Sun (exists only to light the moon's visible face). Render layers cannot fix this: they filter
//! lights by *camera*, not by the meshes a light touches, so a Sun on its own layer either lights nothing
//! or lights the whole cabin from underneath. Instead the Sun stops being a light. Its direction, colour
//! and strength are copied into the moon's material, which adds the Sun's diffuse term after normal PBR
//! lighting. The moonlight is then the only directional light, and the overhead spot gets the one web
//! shadow map.
//!
//! Authoring (lobby.corn): `MoonSun` on the Sun node, `MoonSurface` on the moon node.
use bevy::{
    pbr::{ExtendedMaterial, MaterialExtension},
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
    transform::TransformSystems,
};

// CLAUDE: all authored PBR properties (texture, emissive) are kept; only the fragment shader is extended.
pub(crate) type MoonMaterial = ExtendedMaterial<StandardMaterial, MoonSunlight>;

// CLAUDE: vec4 uniforms (16-byte aligned) keep the layout valid on WebGL2, as PaperGlow does.
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug, PartialEq, Default)]
pub(crate) struct MoonSunlight {
    // CLAUDE: XYZ is the world-space unit vector from the surface toward the Sun.
    #[uniform(100)]
    pub direction: Vec4,
    // CLAUDE: RGB is light colour times illuminance (lux), the same product Bevy uploads for a directional light.
    #[uniform(101)]
    pub color: Vec4,
}

impl MaterialExtension for MoonSunlight {
    fn fragment_shader() -> ShaderRef { "shaders/moon.wesl".into() }
}

// CLAUDE: marks the glTF Sun node. Its DirectionalLight is captured here, then removed.
#[derive(Component, Reflect, Clone, Debug, Default)]
#[reflect(Component, Default)]
pub struct MoonSun {
    pub color: Vec3,
    pub illuminance: f32,
    captured: bool,
}

// CLAUDE: marks the moon root; every StandardMaterial beneath it is swapped for a MoonMaterial.
#[derive(Component, Reflect, Clone, Debug, Default)]
#[reflect(Component, Default)]
pub struct MoonSurface;

// CLAUDE: the latest Sun uniform. New moon materials start from it and live ones are updated from it,
// so a Sun that moves, loads late or reloads never leaves a material with stale values.
#[derive(Resource, Default)]
struct SunState(MoonSunlight);

pub fn plugin(app: &mut App) {
    // CLAUDE: build its pipelines as meshes using it spawn
    crate::precompile::register::<MoonMaterial>(app);
    app.register_type::<MoonSun>()
        .register_type::<MoonSurface>()
        .init_resource::<SunState>()
        .add_plugins(MaterialPlugin::<MoonMaterial>::default())
        .add_systems(Update, (capture_sun, swap_materials))
        .add_systems(PostUpdate, sync_sun.after(TransformSystems::Propagate));
}

// CLAUDE: copy the authored light's colour/strength, then remove it so it no longer counts as a directional
// light. Runs until the light appears, since the glTF scene and the corn patch arrive in either order.
fn capture_sun(mut commands: Commands, mut suns: Query<(Entity, &mut MoonSun, &DirectionalLight)>) {
    for (entity, mut sun, light) in &mut suns {
        let c = light.color.to_linear();
        sun.color = Vec3::new(c.red, c.green, c.blue);
        sun.illuminance = light.illuminance;
        sun.captured = true;
        commands.entity(entity).remove::<DirectionalLight>();
    }
}

// CLAUDE: a light's travel direction is its -Z, so the direction *to* the light is +Z (`back`).
fn sync_sun(
    suns: Query<(&GlobalTransform, &MoonSun)>,
    mut state: ResMut<SunState>,
    mut materials: ResMut<Assets<MoonMaterial>>,
) {
    let Some((global, sun)) = suns.iter().find(|(_, sun)| sun.captured) else { return };
    let next = MoonSunlight {
        direction: global.back().as_vec3().extend(0.0),
        color: (sun.color * sun.illuminance).extend(0.0),
    };
    if state.0 == next { return; }
    state.0 = next;
    for (_, material) in materials.iter_mut() {
        material.extension = state.0.clone();
    }
}

// CLAUDE: scene children appear after their root, so swap when a material is added under a MoonSurface.
fn swap_materials(
    mut commands: Commands,
    added: Query<(Entity, &MeshMaterial3d<StandardMaterial>), Added<MeshMaterial3d<StandardMaterial>>>,
    parents: Query<&ChildOf>,
    surfaces: Query<(), With<MoonSurface>>,
    standard: Res<Assets<StandardMaterial>>,
    state: Res<SunState>,
    mut moon: ResMut<Assets<MoonMaterial>>,
) {
    for (entity, handle) in &added {
        if !parents.iter_ancestors(entity).any(|e| surfaces.contains(e)) { continue; }
        let Some(base) = standard.get(&handle.0).cloned() else { continue };
        let mut base = base;
        base.opaque_render_method = bevy::material::OpaqueRendererMethod::Forward;
        let material = moon.add(MoonMaterial { base, extension: state.0.clone() });
        commands.entity(entity)
            .remove::<MeshMaterial3d<StandardMaterial>>()
            .insert(MeshMaterial3d(material));
    }
}
