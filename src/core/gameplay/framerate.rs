use bevy::{ecs::{system::{Res, Resource, ResMut}, reflect::ReflectResource}, diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin}, app::{Plugin, Update}, reflect::Reflect};

pub fn update_fps(
    mut res: ResMut<FPS>,
    diagnostics: Res<DiagnosticsStore>
){
    res.0 = diagnostics.get(&bevy::diagnostic::FrameTimeDiagnosticsPlugin::FPS).unwrap().average().unwrap_or(0.0);
}

#[derive(Reflect, Resource, Default)]
#[reflect(Resource)]
pub struct FPS(f64);

pub struct PrintFPSPlugin;
impl Plugin for PrintFPSPlugin{
    fn build(&self, app: &mut bevy::prelude::App) {
        app
            .register_type::<FPS>()
            .init_resource::<FPS>()
            .add_plugins(FrameTimeDiagnosticsPlugin)
            .add_systems(Update, update_fps);
    }
}