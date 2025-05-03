//! This module contains all the per state functionality in the App.
//! Mainly consists of OnEnter(state) and OnExit(state) functions and spawning entities that are statescoped
pub mod main_menu;
pub mod lobby;

use bevy::prelude::*;
use crate::systems::{scenes::CornScene, util::camera::{MainCamera, UICamera}};

#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Reflect, Component)]
pub struct CharacterScene;
impl CornScene for CharacterScene{}

#[derive(Default, Debug, Clone)]
pub struct CornScenesPlugin;
impl Plugin for CornScenesPlugin{
    fn build(&self, app: &mut App) {
        app
            .add_systems(Startup, spawn_global_entities)
            .add_plugins((
                main_menu::MainMenuPlugin,
                lobby::LobbyPlugin
            ));
    }
}

fn spawn_global_entities(mut commands: Commands) {
    MainCamera::spawn_main_camera(&mut commands);
    UICamera::spawn_ui_camera(&mut commands);
    commands.spawn(main_menu::MainMenuScene.get_bundle());
}
