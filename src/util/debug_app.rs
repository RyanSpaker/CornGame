use bevy::{app::Plugins, ecs::schedule::ScheduleLabel, prelude::*};

/// Trait that adds conditional functiosn for app that add functionality only for debug mode
pub trait DebugApp{
    fn add_debug_systems<M>(
        &mut self,
        schedule: impl ScheduleLabel,
        systems: impl IntoSystemConfigs<M>,
    ) -> &mut Self;
    #[track_caller]
    fn add_plugins<M>(&mut self, plugins: impl Plugins<M>) -> &mut Self;
}
impl DebugApp for App{
    fn add_debug_systems<M>(
        &mut self,
        schedule: impl ScheduleLabel,
        systems: impl IntoSystemConfigs<M>,
    ) -> &mut Self {
        #[cfg(debug_assertions)]
        self.add_systems(schedule, systems);
        self
    }
    fn add_plugins<M>(&mut self, plugins: impl Plugins<M>) -> &mut Self {
        #[cfg(debug_assertions)]
        self.add_plugins(plugins);
        self
    }
}