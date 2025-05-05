pub mod corn;
pub mod cameras;
pub mod flycam;
pub mod framerate;
pub mod test_cube;

use bevy::prelude::*;
use test_cube::TestCube;
use self::{cameras::CamerasPlugin, framerate::FrameRatePlugin, flycam::FlyCamPlugin, corn::CornFieldComponentPlugin};

pub struct CornECSPlugin;
impl Plugin for CornECSPlugin{
    fn build(&self, app: &mut App) {
        app.add_plugins((
            CamerasPlugin, 
            FrameRatePlugin, 
            FlyCamPlugin, 
            CornFieldComponentPlugin,
            TestCube
        ));
    }
}