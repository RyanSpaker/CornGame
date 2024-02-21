use bevy::{app::{App, Plugin}, ecs::component::Component, render::extract_component::{ExtractComponent, ExtractComponentPlugin}};

#[derive(Component, Clone, ExtractComponent)]
pub struct MainCamera;

pub struct MainCameraPlugin;
impl Plugin for MainCameraPlugin{
    fn build(&self, app: &mut App) {
        app.add_plugins(ExtractComponentPlugin::<MainCamera>::default());
    }
}