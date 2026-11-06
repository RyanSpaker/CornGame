//! CODEX (GPT-6): CRT appearance isolated from UI construction; samples the fully composited monitor.
use bevy::{
    prelude::*,
    render::render_resource::{AsBindGroup, ShaderType},
    shader::ShaderRef,
};

// CODEX: a compact uniform; values are editable through the Monitor component in lobby.corn.
#[derive(Clone, Copy, Debug, Reflect, ShaderType)]
pub struct CrtSettings {
    pub curvature: Vec2,
    pub scan_lines: f32,
    pub scan_opacity: f32,
    pub boost: f32,
    pub aberration: f32,
    pub vignette: f32,
    pub time: f32,
}

// CODEX: match the original monitor's broad appearance without the obsolete 0.16 render graph.
impl Default for CrtSettings {
    // CODEX: initial CRT tuning, overridable by Monitor.crt in the cornfile.
    fn default() -> Self {
        Self {
            curvature: Vec2::new(6.0, 4.0),
            scan_lines: 180.0,
            scan_opacity: 0.2,
            boost: 1.2,
            aberration: 1.0,
            vignette: 0.2,
            time: 0.0,
        }
    }
}

// CODEX: Bevy-only material, so the CRT applies after both the 2D desktop and the 3D previews.
#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct MonitorMaterial {
    #[uniform(0)]
    pub settings: CrtSettings,
    #[texture(1)]
    #[sampler(2)]
    pub image: Handle<Image>,
}

// CODEX: the shader uses the source mesh's authored UVs.
impl Material for MonitorMaterial {
    // CODEX: the scene has one fixed CRT shader; texture and appearance vary as material data.
    fn fragment_shader() -> ShaderRef {
        "shaders/monitor.wesl".into()
    }
}

// CODEX: keep time on the material, independent of UI input or camera transitions.
pub fn plugin(app: &mut App) {
    // CLAUDE: build its pipelines as meshes using it spawn
    crate::precompile::register::<MonitorMaterial>(app);
    app.add_plugins(MaterialPlugin::<MonitorMaterial>::default())
        .add_systems(Update, tick);
}

// CODEX: material settings remain separate from the original glTF material assets.
fn tick(time: Res<Time>, mut materials: ResMut<Assets<MonitorMaterial>>) {
    for (_, material) in materials.iter_mut() {
        material.settings.time = time.elapsed_secs();
    }
}
