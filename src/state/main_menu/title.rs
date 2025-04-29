use bevy::prelude::*;

use crate::app::{state::AppStage, util::button::{BackgroundSelectedColors, ButtonEvent}};

use super::{MainMenuRootNode, MainMenuScreen};

#[derive(Default, Debug, Clone)]
pub struct TitleScreenPlugin;
impl Plugin for TitleScreenPlugin{
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(MainMenuScreen::Title), spawn_title_screen);
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Reflect, Component)]
pub struct PlayButton;

pub fn spawn_title_screen(
    root: Query<Entity, With<MainMenuRootNode>>,
    mut commands: Commands
){
    commands.entity(root.single()).with_children(|parent| {
        parent.spawn((
            Node{
                width: Val::Percent(100.0), height: Val::Percent(100.0), 
                display: Display::Flex, flex_direction: FlexDirection::Column, 
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..Default::default()},
            StateScoped(MainMenuScreen::Title)
        )).with_children(|parent| {
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
                PlayButton
            )).observe(on_press_play);
        });
    });
}


pub fn on_press_play(
    trigger: Trigger<ButtonEvent>,
    mut next_state: ResMut<NextState<AppStage>>,
){
    match trigger.1 {
        Interaction::Pressed => {next_state.set(AppStage::Lobby)}
        Interaction::Hovered | Interaction::None => {}
    }
}
    
