use bevy::{ecs::schedule::ScheduleLabel, prelude::*};

/// Adds a app method to configure a state set in a schedule. The system set will run when in state during schedule.
/// This trait makes it so that we can configure a state set anywhere in the app and it will only configure once, and the same system set will be used everywhere.
pub trait AppStateSet{
    /// Configures a set to run in a schedule only during the state
    fn configure_state_set<P: SystemSet+Clone, S: States+Clone, L:ScheduleLabel+Clone>(&mut self, schedule: L, state: S, set: P) -> &mut Self;
}
impl AppStateSet for App{
    fn configure_state_set<P: SystemSet+Clone, S: States+Clone, L:ScheduleLabel+Clone>(&mut self, schedule: L, state: S, set: P) -> &mut Self {
        self.add_plugins(StateSetPlugin::new(schedule, state, set))
    }
}

/// Plugin to configure a set to run during a state in a schedule. Used to ensure configuration is done only once when used in multiple places
pub struct StateSetPlugin<P, S, L> where P: SystemSet+Clone, S: States+Clone, L: ScheduleLabel+Clone{
    set: P,
    state: S,
    schedule: L,
    name: String
}
impl<P: SystemSet+Clone, S: States+Clone, L: ScheduleLabel+Clone> StateSetPlugin<P, S, L>{pub fn new(schedule: L, state: S, set: P)->Self{Self { 
    name: format!("StateSetPlugin: {:?} {:?}, {:?}", set, state, schedule),
    set,
    state,
    schedule
}}}
impl<P: SystemSet+Clone, S: States+Clone, L: ScheduleLabel+Clone> Plugin for StateSetPlugin<P, S, L>{
    fn build(&self, app: &mut App) {
        app.configure_sets(
            self.schedule.clone(),
            self.set.clone().run_if(in_state(self.state.clone()))
        );
    }
    fn name(&self) -> &str {&self.name}
}