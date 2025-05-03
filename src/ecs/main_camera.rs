use bevy::{prelude::*, render::extract_component::{ExtractComponent, ExtractComponentPlugin}};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Reflect, Component, ExtractComponent, Serialize, Deserialize)]
pub struct MainCamera;

pub struct MainCameraPlugin;
impl Plugin for MainCameraPlugin{
    fn build(&self, app: &mut App) {
        app
            .register_type::<MainCamera>()
            .add_plugins(ExtractComponentPlugin::<MainCamera>::default());
    }
}