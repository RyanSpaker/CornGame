use bevy::prelude::*;
use crate::{scenes::lobby::LobbyScene, systems::{
    scenes::{CornScene, CurrentScene, OnSpawnScene, SceneEntity, SceneTransitionApp}, 
    util::button::{on_press_swap_scene, BackgroundSelectedColors}
}, util::observer_ext::*};
use super::{credits::CreditsScene, options::OptionsScene, MainMenuScene};

#[derive(Debug, Default, Clone, PartialEq, Eq, Hash, Reflect, Component)]
pub struct TitleScene;
impl CornScene for TitleScene{
    fn get_bundle(self) -> impl Bundle {
        (self, SceneEntity, Name::from("Title Screen"), Node{
            width: Val::Percent(100.0), height: Val::Percent(100.0), 
            display: Display::Flex, flex_direction: FlexDirection::Column, 
            justify_content: JustifyContent::Center, align_items: AlignItems::Center,
            row_gap: Val::Px(5.0), ..Default::default()
        })
    }
}
impl TitleScene{
    fn spawn_scene(
        mut commands: Commands,
        parent: Res<CurrentScene>
    ){
        commands.entity(parent.0).with_children(|parent| {
            parent.spawn((TitleSceneObservers, TitleSceneObservers.get_name()));
            parent.spawn((
                Text::new("Corn Game"),
                TextColor(bevy::color::palettes::basic::GREEN.into()),
                BackgroundColor(Color::WHITE),
                TextFont{font_size: 100.0, ..Default::default()}
            ));
            parent.spawn((
                Button,
                Text::new("Play"),
                TextColor(Color::BLACK),
                TextFont{font_size: 32.0, ..Default::default()},
                BackgroundColor(Color::WHITE),
                BackgroundSelectedColors{selected: bevy::color::palettes::basic::GRAY.into(), unselected: Color::WHITE},
            )).observe_as(on_press_swap_scene(MainMenuScene, LobbyScene), TitleSceneObservers);
            parent.spawn((
                Button,
                Text::new("Options"),
                TextColor(Color::BLACK),
                TextFont{font_size: 32.0, ..Default::default()},
                BackgroundColor(Color::WHITE),
                BackgroundSelectedColors{selected: bevy::color::palettes::basic::GRAY.into(), unselected: Color::WHITE},
            )).observe_as(on_press_swap_scene(TitleScene, OptionsScene), TitleSceneObservers);
            parent.spawn((
                Button,
                Text::new("Credits"),
                TextColor(Color::BLACK),
                TextFont{font_size: 32.0, ..Default::default()},
                BackgroundColor(Color::WHITE),
                BackgroundSelectedColors{selected: bevy::color::palettes::basic::GRAY.into(), unselected: Color::WHITE},
            )).observe_as(on_press_swap_scene(TitleScene, CreditsScene), TitleSceneObservers);
        });
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Hash, Reflect, Component)]
pub struct TitleSceneObservers;
impl ObserverParent for TitleSceneObservers{fn get_name(&self) -> Name {Name::from("Title Scene Observers")}}

#[derive(Default, Debug, Clone)]
pub struct TitleScreenPlugin;
impl Plugin for TitleScreenPlugin{
    fn build(&self, app: &mut App) {
        app
            .register_type::<TitleScene>()
            .register_type::<TitleSceneObservers>()
            .init_scene::<TitleScene>()
            .add_systems(OnSpawnScene(TitleScene), TitleScene::spawn_scene);
    }
}