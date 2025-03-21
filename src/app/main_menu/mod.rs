mod load;

use bevy::prelude::*;
use super::loading::load_entity;

pub struct MainMenuPlugin;
impl Plugin for MainMenuPlugin{
    fn build(&self, app: &mut App) {
        //app.add_plugins(load::MainMenuLoadingStage.get_plugin(true));
    }
}