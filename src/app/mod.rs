pub mod state;
pub mod util;

use bevy::prelude::*;

pub struct CornAppPlugin;
impl Plugin for CornAppPlugin{
    fn build(&self, app: &mut App) {
        app
            .add_plugins((
                state::AppStatePlugin,
                util::AppUtilPlugin
            ));
    }
}


