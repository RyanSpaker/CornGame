//mod main_menu;
mod state;
pub mod loading;
use bevy::prelude::*;

/// State of loading for the current [`AppStage`]
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect, States)]
pub enum LoadingState{
    #[default] Loading,
    Loaded
}
/// Current stage of the app. Each stage has distinct differences in how the app needs to run.
#[derive(Default, Debug, Clone, PartialEq, Eq, Hash, Reflect, States)]
pub enum AppStage{
    #[default] MainMenu,
    Lobby,
    Level
}

pub struct CornAppPlugin;
impl Plugin for CornAppPlugin{
    fn build(&self, app: &mut App) {
        app
            .init_state::<LoadingState>()
            .init_state::<AppStage>();
            //.add_plugins(main_menu::MainMenuPlugin);
    }
}



