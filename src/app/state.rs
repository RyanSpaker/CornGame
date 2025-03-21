use bevy::prelude::*;

/// Current stage of the app. Each stage has distinct differences in how the app needs to run.
#[derive(Default, Debug, Clone, PartialEq, Eq, Hash, Reflect, States)]
pub enum AppStage{
    #[default] MainMenu,
    Lobby,
    Level
}

/// Adds app state enum
#[derive(Default, Debug, Clone, Copy)]
pub struct AppStatePlugin;
impl Plugin for AppStatePlugin{
    fn build(&self, app: &mut App) {
        app.init_state::<AppStage>();
    }
}

/// Functionality corresponding to system sets that run during states
pub mod state_set{
    use bevy::{ecs::schedule::ScheduleLabel, prelude::*};
    
    /// A system set that runs only when in state 'S'
    #[derive(Default, Debug, Clone, PartialEq, Eq, Hash, Reflect, SystemSet)]
    pub struct StateSystemSet<S: States>(pub S);
    impl<S: States> From<S> for StateSystemSet<S>{
        fn from(value: S) -> Self {Self(value)}
    }

    pub fn state_set<S: States>(state: S) -> StateSystemSet<S>{
        StateSystemSet::from(state)
    }

    /// Allows for system sets that automatically only run during certain states, and are global across the app
    pub trait AppStateSets{
        fn configure_state_set<S: States, L: ScheduleLabel+Clone>(&mut self, schedule: L, state: S) -> &mut Self;
        fn configure_child_state_set<S: States, L: ScheduleLabel+Clone, P: SystemSet>(&mut self, schedule: L, state: S, parent: P) -> &mut Self;
    }
    impl AppStateSets for App{
        fn configure_state_set<S: States, L: ScheduleLabel+Clone>(&mut self, schedule: L, state: S) -> &mut Self{
            self.add_plugins(StateSetConfigurationPlugin::new(state, schedule))
        }
        fn configure_child_state_set<S: States, L: ScheduleLabel+Clone, P: SystemSet>(&mut self, schedule: L, state: S, parent: P) -> &mut Self{
            self.add_plugins(StateSetConfigurationPlugin::new(state.clone(), schedule.clone()))
                .configure_sets(schedule, StateSystemSet(state).in_set(parent))         
        }
    }

    pub struct StateSetConfigurationPlugin<S: States, L: ScheduleLabel+Clone>{
        state: S,
        label: L,
        name: String
    }
    impl<S: States, L: ScheduleLabel+Clone> StateSetConfigurationPlugin<S, L>{
        pub fn new(state: S, label: L) -> Self{Self{name: std::any::type_name::<Self>().to_string() + format!("{:?}{:?}", state, label).as_str(), state, label}}
    }
    impl<S: States, L: ScheduleLabel+Clone> Plugin for StateSetConfigurationPlugin<S, L>{
        fn build(&self, app: &mut App) {
            app.configure_sets(self.label.clone(), StateSystemSet(self.state.clone()).run_if(in_state(self.state.clone())));
        }
        fn name(&self) -> &str {&self.name}
    }    
}