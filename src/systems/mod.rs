pub mod util;
pub mod scenes;

use bevy::prelude::*;

pub struct CornSystemsPlugin;
impl Plugin for CornSystemsPlugin{
    fn build(&self, app: &mut App) {
        app
            .add_plugins((util::AppUtilPlugin, scenes::SceneTransition));
    }
}


