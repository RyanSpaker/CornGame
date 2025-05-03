//pub mod corn;
pub mod main_camera;
pub mod flycam;
pub mod framerate;
//pub mod loading;

use bevy::prelude::*;
use self::{main_camera::MainCameraPlugin, framerate::FrameRatePlugin};

pub struct CornECSPlugin;
impl Plugin for CornECSPlugin{
    fn build(&self, app: &mut App) {
        app.add_plugins((MainCameraPlugin, FrameRatePlugin));
    }
}