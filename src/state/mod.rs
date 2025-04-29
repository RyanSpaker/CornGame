//! This module contains all the per state functionality in the App.
//! Mainly consists of OnEnter(state) and OnExit(state) functions and spawning entities that are statescoped
pub mod main_menu;
pub mod lobby;

use bevy::prelude::*;
use crate::app::util::camera::{MainCamera, UICamera};

/// Current stage of the app. Each stage has distinct differences in how the app needs to run.
#[derive(Default, Debug, Clone, PartialEq, Eq, Hash, Reflect, States, SystemSet)]
pub enum AppStage{
    /// State set to this for one frame so that startup systems run before OnEnter(MainMenu)
    #[default] Init,
    /// Initial State of the app
    MainMenu,
    /// 3d environment used for level select
    Lobby,
    /// Actually in a level
    Level
}

/// State that exists when in the lobby or a level.
#[derive(Default, Debug, Clone, PartialEq, Eq, Hash, Reflect, SystemSet)]
pub struct InGame;
impl ComputedStates for InGame{
    type SourceStates = AppStage;
    fn compute(sources: Self::SourceStates) -> Option<Self> {
        match sources{
            AppStage::MainMenu | AppStage::Init => None,
            AppStage::Level | AppStage::Lobby => Some(InGame)
        }
    }
}


#[derive(Default, Debug, Clone)]
pub struct ScenesPlugin;
impl Plugin for ScenesPlugin{
    fn build(&self, app: &mut App) {
        app
            .init_state::<AppStage>()
            .add_computed_state::<InGame>()
            .enable_state_scoped_entities::<AppStage>()
            .enable_state_scoped_entities::<InGame>()
            .add_systems(Startup, spawn_global_entities)
            .add_plugins((
                main_menu::MainMenuPlugin,
                lobby::LobbyPlugin
            ));
    }
}

fn spawn_global_entities(
    mut commands: Commands,
    mut next_state: ResMut<NextState<AppStage>>
){
    MainCamera::spawn_main_camera(&mut commands);
    UICamera::spawn_ui_camera(&mut commands);
    next_state.set(AppStage::MainMenu);
}
