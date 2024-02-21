pub mod corn;
pub mod main_camera;
pub mod flycam;
pub mod framerate;

use bevy::prelude::*;
use self::{
    corn::CornFieldComponentPlugin, main_camera::MainCameraPlugin, framerate::FrameRatePlugin
};

pub struct CornGameECSPlugin;
impl Plugin for CornGameECSPlugin{
    fn build(&self, app: &mut App) {
        app.add_plugins((CornFieldComponentPlugin, MainCameraPlugin, FrameRatePlugin));
    }
}