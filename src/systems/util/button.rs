
use bevy::prelude::*;
use crate::util::observer_ext::*;

#[derive(Debug, Clone, PartialEq, Eq, Reflect, Event)]
pub struct ButtonEvent(pub Entity, pub Interaction);

#[derive(Debug, Default, Clone, PartialEq, Reflect, Component)]
#[reflect(Component)]
pub struct BackgroundSelectedColors{
    pub selected: Color,
    pub unselected: Color
}

#[derive(Debug, Default, Clone, PartialEq, Reflect, Component)]
#[reflect(Component)]
pub struct BorderSelectedColors{
    pub selected: Color,
    pub unselected: Color
}

#[derive(Debug, Default, Clone, PartialEq, Reflect, Component)]
#[reflect(Component)]
pub struct TextSelectedColors{
    pub selected: Color,
    pub unselected: Color
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Reflect, Component)]
#[reflect(Component)]
pub struct ButtonObservers;
impl ObserverParent for ButtonObservers{
    fn get_name(&self) -> Name {Name::from("Button Observers")}
}

#[derive(Default, Debug, Clone)]
pub struct ButtonPlugin;
impl Plugin for ButtonPlugin{
    fn build(&self, app: &mut App) {
        app
            .register_type::<ButtonEvent>()
            .register_type::<BackgroundSelectedColors>()
            .register_type::<BorderSelectedColors>()
            .register_type::<TextSelectedColors>()
            .register_type::<ButtonObservers>()
            .add_event::<ButtonEvent>()
            .add_systems(Update, send_button_events)
            .add_observer_as(button_event_observer, ButtonObservers);
    }
}

/// Runs a function when pressed
pub fn on_press_run_command<C>(command: C) -> impl FnMut(Trigger<ButtonEvent>, Commands)->() 
where C: Fn(Entity, &mut World) + Send + 'static + Clone
{
    move |
        trigger: Trigger<ButtonEvent>,
        mut commands: Commands, 
    | {
        match trigger.1 {Interaction::Pressed => {
            let entity = trigger.target();
            let func = command.clone();
            commands.queue(move |world: &mut World| {
                func(entity, world);
            });
        } _ => {}}
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
    event_writer.write_batch(events);
}