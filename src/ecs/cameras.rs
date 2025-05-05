use bevy::{prelude::*, render::extract_component::{ExtractComponent, ExtractComponentPlugin}};
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, PartialEq, Eq, Hash, Reflect, Component, ExtractComponent, Serialize, Deserialize)]
#[reflect(Component)]
pub struct MainCamera;
impl MainCamera{
    pub fn spawn_main_camera(commands: &mut Commands) -> Entity{
        commands.spawn((
            Self, 
            Camera3d::default(), 
            Camera{order: 0, hdr: true, ..Default::default()},
            Name::from("Main Camera")
        )).id()
    }
    pub fn disable_main_camera(mut query: Query<&mut Camera, With<Self>>){
        for camera in query.iter_mut(){camera.into_inner().is_active = false;}
    }
    pub fn enable_main_camera(mut query: Query<&mut Camera, With<Self>>){
        for camera in query.iter_mut(){camera.into_inner().is_active = true;}
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Hash, Reflect, Component, ExtractComponent, Serialize, Deserialize)]
#[reflect(Component)]
pub struct UICamera;
impl UICamera{
    pub fn spawn_ui_camera(commands: &mut Commands) -> Entity{
        commands.spawn((
            Self, 
            Camera2d, 
            Camera{order: 1, ..Default::default()},
            Name::from("UI Camera")
        )).id()
    }
    pub fn disable_ui_camera(mut query: Query<&mut Camera, With<Self>>){
        for camera in query.iter_mut(){camera.into_inner().is_active = false;}
    }
    pub fn enable_ui_camera(mut query: Query<&mut Camera, With<Self>>){
        for camera in query.iter_mut(){camera.into_inner().is_active = true;}
    }
}

pub struct CamerasPlugin;
impl Plugin for CamerasPlugin{
    fn build(&self, app: &mut App) {
        app
            .register_type::<MainCamera>()
            .register_type::<UICamera>()
            .add_plugins((
                ExtractComponentPlugin::<MainCamera>::default(),
                ExtractComponentPlugin::<UICamera>::default()
            ));
    }
}