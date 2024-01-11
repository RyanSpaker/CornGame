use bevy::{ecs::component::Component, render::extract_component::ExtractComponent};

pub mod integer_set;
pub mod specialized_material;

#[derive(Component, Clone, ExtractComponent)]
pub struct DebugTag{}