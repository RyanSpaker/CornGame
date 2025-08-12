use std::{any::type_name, hash::Hash, marker::PhantomData};

use bevy::{ecs::schedule::ScheduleLabel, prelude::*};

#[derive(Debug, Clone, PartialEq, Eq, SystemSet)]
pub struct EventSet<E: Event>(pub PhantomData<E>);
impl<E: Event> Default for EventSet<E>{fn default() -> Self {Self(PhantomData::default())}}
impl<E: Event> Hash for EventSet<E> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

/// Adds a app method to configure a state set in a schedule. The system set will run when in state during schedule.
/// This trait makes it so that we can configure a state set anywhere in the app and it will only configure once, and the same system set will be used everywhere.
pub trait AppEventSet{
    /// Configures a set to run in a schedule only during the state
    fn configure_event_set<E: Event>(&mut self, schedule: impl ScheduleLabel+Clone, set: impl SystemSet+Clone) -> &mut Self;
}
impl AppEventSet for App{
    fn configure_event_set<E: Event>(&mut self, schedule: impl ScheduleLabel+Clone, set: impl SystemSet+Clone) -> &mut Self {
        let plugin = EventSetPlugin::<_, E, _>::new(schedule, set);
        if !self.get_added_plugins::<EventSetPlugin<_, E, _>>().contains(&&plugin){self.add_plugins(plugin);}
        self
    }
}

/// Plugin to configure a set to run during a state in a schedule. Used to ensure configuration is done only once when used in multiple places
pub struct EventSetPlugin<P, E, L> where P: SystemSet+Clone, E: Event, L: ScheduleLabel+Clone{
    set: P,
    schedule: L,
    _phantom_data: PhantomData<E>,
    name: String
}
impl<P: SystemSet+Clone, E: Event, L: ScheduleLabel+Clone> PartialEq for EventSetPlugin<P, E, L>{
    fn eq(&self, other: &Self) -> bool {self.name==other.name}
    fn ne(&self, other: &Self) -> bool {self.name!=other.name}
}
impl<P: SystemSet+Clone, E: Event, L: ScheduleLabel+Clone> EventSetPlugin<P, E, L>{pub fn new(schedule: L, set: P)->Self{Self { 
    name: format!("EventSetPlugin: {:?} {:?} {:?}", type_name::<E>(), set, schedule),
    set,
    _phantom_data: PhantomData::default(),
    schedule
}}}
impl<P: SystemSet+Clone, E: Event, L: ScheduleLabel+Clone> Plugin for EventSetPlugin<P, E, L>{
    fn build(&self, app: &mut App) {
        app
        .configure_sets(
            self.schedule.clone(),
            self.set.clone().run_if(on_event::<E>)
        );
    }
    fn name(&self) -> &str {&self.name}
}