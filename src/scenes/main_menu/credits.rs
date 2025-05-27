use bevy::prelude::*;
use crate::{
    systems::{
        scenes::{CornScene, CurrentScene, OnSpawnScene, SceneEntity, SceneTransitionApp}, 
        util::button::{on_press_swap_scene, BackgroundSelectedColors}}, 
    util::observer_ext::*};
use super::title::TitleScene;

#[derive(Default, Debug, Clone, PartialEq, Eq, Hash, Reflect, Component)]
pub struct CreditsScene;
impl CornScene for CreditsScene{
    fn get_bundle(self) -> impl Bundle {
        (self, SceneEntity, Name::from("Credits Screen"), Node{
            display: Display::Flex, flex_direction: FlexDirection::Column,
            width: Val::Percent(100.0), height: Val::Percent(100.0), 
            justify_content: JustifyContent::Center, align_items: AlignItems::Center, row_gap: Val::Percent(5.0),
            ..Default::default()
        })
    }
}
impl CreditsScene{
    fn spawn_scene(
        mut commands: Commands,
        parent: Res<CurrentScene>
    ){
        commands.entity(parent.0).with_children(|parent| {
            parent.spawn((CreditsSceneObservers, CreditsSceneObservers.get_name()));
            parent.spawn((
                Text::new("Credits"),
                TextColor(bevy::color::palettes::basic::BLACK.into()),
                BackgroundColor(Color::WHITE),
                TextFont{font_size: 100.0, ..Default::default()}
            ));
            parent.spawn((
                Button,
                Text::new("Back"),
                TextColor(Color::BLACK),
                TextFont{font_size: 32.0, ..Default::default()},
                BackgroundColor(Color::WHITE),
                BackgroundSelectedColors{selected: bevy::color::palettes::basic::GRAY.into(), unselected: Color::WHITE},
            )).observe_as(on_press_swap_scene(CreditsScene, TitleScene), CreditsSceneObservers);
        });
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Hash, Reflect, Component)]
pub struct CreditsSceneObservers;
impl ObserverParent for CreditsSceneObservers{fn get_name(&self) -> Name {Name::from("Options Scene Observers")}}

#[derive(Default, Debug, Clone)]
pub struct CreditScreenPlugin;
impl Plugin for CreditScreenPlugin{
    fn build(&self, app: &mut App) {
        app.register_type::<CreditsScene>()
            .register_type::<CreditsSceneObservers>()
            .init_scene::<CreditsScene>()
            .add_systems(OnSpawnScene(CreditsScene), CreditsScene::spawn_scene);
    }
}