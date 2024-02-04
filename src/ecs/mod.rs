pub mod corn;
pub mod main_camera;
pub mod flycam;

use bevy::prelude::*;
use self::{
    corn::CornFieldComponentPlugin, flycam::{FlyCamPlugin, FlyCamState}, main_camera::MainCameraPlugin
};

pub struct CornGameECSPlugin;
impl Plugin for CornGameECSPlugin{
    fn build(&self, app: &mut App) {
        app.add_plugins((CornFieldComponentPlugin, MainCameraPlugin, FlyCamPlugin::new(FlyCamState::Unfocused)));
    }
}