use bevy::prelude::*;
use crate::{scenes::AppStage, systems::util::button::{on_press_switch_state, BackgroundSelectedColors}};
use super::{MainMenuRootNode, MainMenuScreen};

#[derive(Default, Debug, Clone)]
pub struct TitleScreenPlugin;
impl Plugin for TitleScreenPlugin{
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(MainMenuScreen::Title), spawn_title_screen);
    }
}

pub fn spawn_title_screen(
    root: Query<Entity, With<MainMenuRootNode>>,
    mut commands: Commands
){
    commands.entity(root.single()).with_children(|parent| {
        parent.spawn((
            Name::from("Title Screen"),
            Node{
                width: Val::Percent(100.0), height: Val::Percent(100.0), 
                display: Display::Flex, flex_direction: FlexDirection::Column, 
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(5.0),
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
            )).observe(on_press_switch_state(AppStage::Lobby));
            parent.spawn((
                Button,
                Text::new("Options"),
                TextColor(Color::BLACK),
                TextFont{font_size: 32.0, ..Default::default()},
                BackgroundColor(Color::WHITE),
                BackgroundSelectedColors{selected: bevy::color::palettes::basic::GRAY.into(), unselected: Color::WHITE},
            )).observe(on_press_switch_state(MainMenuScreen::Options));
            parent.spawn((
                Button,
                Text::new("Credits"),
                TextColor(Color::BLACK),
                TextFont{font_size: 32.0, ..Default::default()},
                BackgroundColor(Color::WHITE),
                BackgroundSelectedColors{selected: bevy::color::palettes::basic::GRAY.into(), unselected: Color::WHITE},
            )).observe(on_press_switch_state(MainMenuScreen::Credits));
        });
    });
}
