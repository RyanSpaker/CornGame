use bevy::prelude::*;
use super::{MainMenuRootNode, MainMenuScreen};
use crate::systems::util::button::{on_press_switch_state, BackgroundSelectedColors};

#[derive(Default, Debug, Clone)]
pub struct CreditScreenPlugin;
impl Plugin for CreditScreenPlugin{
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(MainMenuScreen::Credits), spawn_credits_screen);
    }
}

fn spawn_credits_screen(
    root: Query<Entity, With<MainMenuRootNode>>,
    mut commands: Commands
) {
    commands.entity(root.single()).with_children(|parent| {
        parent.spawn((
            Name::from("Credits Screen"),
            Node{
                display: Display::Flex, flex_direction: FlexDirection::Column,
                width: Val::Percent(100.0), height: Val::Percent(100.0), 
                justify_content: JustifyContent::Center, align_items: AlignItems::Center, row_gap: Val::Percent(5.0),
                ..Default::default()
            }, StateScoped(MainMenuScreen::Credits)
        )).with_children(|parent| {
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
            )).observe(on_press_switch_state(MainMenuScreen::Title));
        });
    });
}