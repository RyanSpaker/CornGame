use bevy::{app::{App, Plugin}, core_pipeline::experimental::taa::TemporalAntiAliasPlugin, ecs::component::Component, render::extract_component::{ExtractComponent, ExtractComponentPlugin}};

#[derive(Component, Clone, ExtractComponent)]
pub struct MainCamera;

pub struct MainCameraPlugin;
impl Plugin for MainCameraPlugin{
    fn build(&self, app: &mut App) {
        app.add_plugins(ExtractComponentPlugin::<MainCamera>::default());
        app.add_plugins(TemporalAntiAliasPlugin);
    }
}