use bevy::prelude::*;
use super::state::AppStage;

#[derive(Debug, Clone, Reflect, Resource)]
pub struct MainMenuConfiguration{
    title_font_size: f32,
    title_font_color: Color
}
impl Default for MainMenuConfiguration{fn default() -> Self {Self{
    title_font_size: 100.0,
    title_font_color: bevy::color::palettes::basic::GREEN.into()
}}}
impl MainMenuConfiguration{
    pub fn spawn_entities(&self, mut commands: Commands){
        commands.spawn((
            Node{
                display: Display::Flex,
                justify_self: JustifySelf::Center,
                align_self: AlignSelf::Center,
                justify_content: JustifyContent::Center,
                flex_direction: FlexDirection::Column,
                width: Val::Percent(90.0),
                height: Val::Percent(90.0),
                ..Default::default()
            },
            BackgroundColor(Color::WHITE),
            StateScoped(AppStage::MainMenu)
        )).with_children(|parent| {
            parent.spawn((
                Node{
                    width: Val::Auto,
                    height: Val::Auto,
                    align_self: AlignSelf::Center,
                    justify_self: JustifySelf::Center,
                    ..Default::default()
                },
                BackgroundColor(Color::BLACK),
                TextLayout{justify: JustifyText::Center, linebreak: LineBreak::NoWrap},
                Text::new("CORN GAME"),
                TextFont{font_size: self.title_font_size, ..Default::default()},
                TextColor(self.title_font_color),
            ));
        });
    }
}


pub struct MainMenuPlugin;
impl Plugin for MainMenuPlugin{
    fn build(&self, app: &mut App) {
        app
            .init_resource::<MainMenuConfiguration>()
            .add_systems(OnEnter(AppStage::MainMenu), spawn_menu)
            .enable_state_scoped_entities::<AppStage>();
    }
}

/// System which spawns main menu entities
pub fn spawn_menu(
    commands: Commands,
    conf: Res<MainMenuConfiguration>
){
    conf.spawn_entities(commands);
}