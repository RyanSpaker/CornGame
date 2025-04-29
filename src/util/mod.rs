use bevy::{ecs::component::Component, render::extract_component::ExtractComponent};

pub mod integer_set;
//pub mod specialized_material;
//pub mod asset_io;
pub mod debug_app;
pub mod state_set;

#[derive(Component, Clone, ExtractComponent)]
pub struct DebugTag{}

pub fn lerp(a: f32, b:f32, r: f32) -> f32{
    a + (b-a)*r
}