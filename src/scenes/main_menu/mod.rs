pub mod title;
pub mod credits;
pub mod options;

use bevy::prelude::*;
use title::TitleScene;
use crate::systems::{scenes::{CornScene, CurrentScene, OnDespawnScene, OnSpawnScene, SceneEntity, SceneTransitionApp}, util::camera::{MainCamera, UICamera}};

#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Reflect, Component)]
pub struct MainMenuScene;
impl CornScene for MainMenuScene{
    fn get_bundle(self) -> impl Bundle {
        (self, SceneEntity, Name::from("Main Menu Scene"), Node{
            display: Display::Block, width: Val::Percent(100.0), height: Val::Percent(100.0), ..Default::default()
        })
    }
}
impl MainMenuScene{
    pub fn spawn_scene(
        mut commands: Commands,
        parent: Res<CurrentScene>
    ){
        commands.entity(parent.0).with_children(|parent|{
            parent.spawn(TitleScene.get_bundle());
        });
    }
}

#[derive(Default, Debug, Clone)]
pub struct MainMenuPlugin;
impl Plugin for MainMenuPlugin{
    fn build(&self, app: &mut App) {
        app
            .register_type::<MainMenuScene>()
            .init_scene::<MainMenuScene>()
            .add_systems(OnSpawnScene(MainMenuScene), (
                MainMenuScene::spawn_scene,
                MainCamera::disable_main_camera,
                UICamera::enable_ui_camera
            )).add_systems(OnDespawnScene(MainMenuScene), MainCamera::enable_main_camera);
        app.add_plugins((title::TitleScreenPlugin, credits::CreditScreenPlugin, options::OptionScreenPlugin));
    }
}