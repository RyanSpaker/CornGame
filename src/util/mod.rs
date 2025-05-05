use bevy::{prelude::*, render::extract_component::ExtractComponent};

pub mod integer_set;
pub mod specialized_material;
pub mod asset_io;
pub mod debug_app;
pub mod scene_set;
pub mod observer_ext;
pub mod clone_entity;

#[derive(Component, Clone, ExtractComponent)]
pub struct DebugTag{}

pub fn lerp(a: f32, b:f32, r: f32) -> f32{
    a + (b-a)*r
}

