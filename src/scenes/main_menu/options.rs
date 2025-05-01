use bevy::prelude::*;
use crate::systems::util::button::{on_press_switch_state, BackgroundSelectedColors};
use super::{MainMenuRootNode, MainMenuScreen};

#[derive(Default, Debug, Clone)]
pub struct OptionScreenPlugin;
impl Plugin for OptionScreenPlugin{
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(MainMenuScreen::Options), spawn_options_screen);
    }
}

fn spawn_options_screen(
    root: Query<Entity, With<MainMenuRootNode>>,
    mut commands: Commands
) {
    commands.entity(root.single()).with_children(|parent| {
        parent.spawn((
            Name::from("Options Screen"),
            Node{
                display: Display::Flex, flex_direction: FlexDirection::Column,
                width: Val::Percent(100.0), height: Val::Percent(100.0), 
                justify_content: JustifyContent::Center, align_items: AlignItems::Center, row_gap: Val::Percent(5.0),
                ..Default::default()
            }, StateScoped(MainMenuScreen::Options)
        )).with_children(|parent| {
            parent.spawn((
                Text::new("Options"),
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
            )).observe(on_press_switch_state(MainMenuScreen::Title));
        });
    });
}