pub mod corn;
pub mod main_camera;

use bevy::prelude::*;
use self::corn::CornFieldComponentPlugin;

pub struct CustomComponentRenderingPlugin;
impl Plugin for CustomComponentRenderingPlugin{
    fn build(&self, app: &mut App) {
        app.add_plugins((CornFieldComponentPlugin, main_camera::MainCameraPlugin{}));
    }
}