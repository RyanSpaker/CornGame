use bevy::{app::{Plugin, Update}, diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin}, ecs::{component::Component, system::{Query, Res}}, reflect::Reflect};

pub fn update_fps(
    mut query: Query<&mut FPS>,
    diagnostics: Res<DiagnosticsStore>
){
    for mut item in query.iter_mut(){
        item.fps = diagnostics.get(&bevy::diagnostic::FrameTimeDiagnosticsPlugin::FPS).unwrap().average().unwrap_or(0.0);
    }
}

#[derive(Clone, Debug, Reflect, Component, Default)]
pub struct FPS{pub fps: f64}

pub struct PrintFPSPlugin;
impl Plugin for PrintFPSPlugin{
    fn build(&self, app: &mut bevy::prelude::App) {
        app
            .register_type::<FPS>()
            .add_plugins(FrameTimeDiagnosticsPlugin)
            .add_systems(Update, update_fps);
    }
}