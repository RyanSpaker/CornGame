use bevy::prelude::*;

#[derive(Default, Debug, Clone, PartialEq, Eq, Hash, Reflect, Component)]
pub struct MainCamera;
impl MainCamera{
    pub fn spawn_main_camera(commands: &mut Commands) -> Entity{
        commands.spawn((Self, Camera3d::default(), Camera{order: 0, ..Default::default()})).id()
    }
    pub fn disable_main_camera(mut query: Query<&mut Camera, With<Self>>){
        for camera in query.iter_mut(){camera.into_inner().is_active = false;}
    }
    pub fn enable_main_camera(mut query: Query<&mut Camera, With<Self>>){
        for camera in query.iter_mut(){camera.into_inner().is_active = true;}
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Hash, Reflect, Component)]
pub struct UICamera;
impl UICamera{
    pub fn spawn_ui_camera(commands: &mut Commands) -> Entity{
        commands.spawn((Self, Camera2d, Camera{order: 1, ..Default::default()})).id()
    }
    pub fn disable_ui_camera(mut query: Query<&mut Camera, With<Self>>){
        for camera in query.iter_mut(){camera.into_inner().is_active = false;}
    }
    pub fn enable_ui_camera(mut query: Query<&mut Camera, With<Self>>){
        for camera in query.iter_mut(){camera.into_inner().is_active = true;}
    }
}

