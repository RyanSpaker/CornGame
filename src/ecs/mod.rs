pub mod corn_field;

use bevy::prelude::*;
use self::corn_field::CornFieldComponentPlugin;

pub struct CustomComponentRenderingPlugin;
impl Plugin for CustomComponentRenderingPlugin{
    fn build(&self, app: &mut App) {
        app.add_plugins(CornFieldComponentPlugin);
    }
}