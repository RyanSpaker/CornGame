pub mod corn;
pub mod cameras;
pub mod flycam;
pub mod framerate;
pub mod test_cube;
pub mod sunlight;
pub mod menu_lobby;
pub mod menu_crt_effect;
pub mod ambient_light;
pub mod menu_main;
pub mod sound_crickets;
pub mod npc;

use bevy::prelude::*;
use corn::CornFieldComponentPlugin;
use test_cube::TestCube;

use crate::{ecs::menu_lobby::spawn_diagetic_interface, scenes::main_menu};

use self::{cameras::CamerasPlugin, framerate::FrameRatePlugin, flycam::FlyCamPlugin};

pub struct CornECSPlugin;
impl Plugin for CornECSPlugin{
    fn build(&self, app: &mut App) {
        app.add_plugins((
            CamerasPlugin, 
            FrameRatePlugin, 
            FlyCamPlugin, 
            CornFieldComponentPlugin,
            TestCube,
            sunlight::SunPlugin,
            menu_main::plugin,
            menu_lobby::plugin,
            menu_crt_effect::PostProcessPlugin,
            ambient_light::plugin,

            sound_crickets::CricketsPlugin,
        ));
    }
}