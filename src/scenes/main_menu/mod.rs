pub mod title;
pub mod credits;
pub mod options;

use bevy::prelude::*;
use crate::systems::util::camera::MainCamera;
use super::AppStage;

#[derive(Default, Debug, Clone, PartialEq, Eq, Hash, Reflect, States, SystemSet)]
pub enum MainMenuScreen{
    #[default] Title,
    Credits,
    Options
}
impl SubStates for MainMenuScreen{
    type SourceStates = AppStage;
    fn should_exist(sources: Self::SourceStates) -> Option<Self> {
        match sources{
            AppStage::MainMenu => Some(MainMenuScreen::default()),
            AppStage::Level | AppStage::Lobby | AppStage::Init => None
        }
    }
}

#[derive(Default, Debug, Clone)]
pub struct MainMenuPlugin;
impl Plugin for MainMenuPlugin{
    fn build(&self, app: &mut App) {
        app
            .add_sub_state::<MainMenuScreen>()
            .enable_state_scoped_entities::<MainMenuScreen>()
            .add_plugins((
                title::TitleScreenPlugin, 
                credits::CreditScreenPlugin, 
                options::OptionScreenPlugin
            ))
            .add_systems(OnEnter(AppStage::MainMenu), (
                MainCamera::disable_main_camera,
                setup_main_menu
            ))
            .add_systems(OnExit(AppStage::MainMenu), MainCamera::enable_main_camera);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Reflect, Component)]
pub struct MainMenuRootNode;

pub fn setup_main_menu(
    mut commands: Commands
){
    commands.spawn((
        Name::from("Main Menu Scene"),
        Node{display: Display::Block, width: Val::Percent(100.0), height: Val::Percent(100.0), ..Default::default()},
        MainMenuRootNode, 
        StateScoped(AppStage::MainMenu)
    ));
}