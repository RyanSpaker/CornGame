use bevy::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq, Reflect, Event)]
pub struct ButtonEvent(pub Entity, pub Interaction);

#[derive(Debug, Default, Clone, PartialEq, Reflect, Component)]
pub struct BackgroundSelectedColors{
    pub selected: Color,
    pub unselected: Color
}

#[derive(Debug, Default, Clone, PartialEq, Reflect, Component)]
pub struct BorderSelectedColors{
    pub selected: Color,
    pub unselected: Color
}

#[derive(Debug, Default, Clone, PartialEq, Reflect, Component)]
pub struct TextSelectedColors{
    pub selected: Color,
    pub unselected: Color
}

#[derive(Default, Debug, Clone)]
pub struct ButtonPlugin;
impl Plugin for ButtonPlugin{
    fn build(&self, app: &mut App) {
        app
            .add_event::<ButtonEvent>()
            .add_systems(Update, send_button_events)
            .add_observer(button_event_observer);
    }
}

fn button_event_observer(
    trigger: Trigger<ButtonEvent>,
    mut background_query: Query<(&mut BackgroundColor, &BackgroundSelectedColors)>,
    mut border_query: Query<(&mut BorderColor, &BorderSelectedColors)>,
    mut text_query: Query<(&mut TextColor, &TextSelectedColors)>
){
    let selected = match trigger.1 {
        Interaction::Hovered => true,
        Interaction::None => false,
        Interaction::Pressed => return
    };
    if let Ok((background, colors)) = background_query.get_mut(trigger.0) {
        background.into_inner().0 = if selected {colors.selected} else {colors.unselected};
    }
    if let Ok((border, colors)) = border_query.get_mut(trigger.0) {
        border.into_inner().0 = if selected {colors.selected} else {colors.unselected};
    }
    if let Ok((text, colors)) = text_query.get_mut(trigger.0) {
        text.into_inner().0 = if selected {colors.selected} else {colors.unselected};
    }
}

/// Triggers buttons on interaction. Designed this way so that other systems can trigger buttons as well, such as controller navigation
fn send_button_events(
    buttons: Query<(Entity, &Interaction), (With<Button>, Changed<Interaction>)>,
    mut event_writer: EventWriter<ButtonEvent>,
    mut commands: Commands
){
    let mut events = vec![];
    for (entity, event) in buttons.iter(){
        events.push(ButtonEvent(entity, event.to_owned()));
        commands.trigger_targets(ButtonEvent(entity, event.to_owned()), entity);
    }
    event_writer.send_batch(events);
}