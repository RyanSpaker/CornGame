use bevy::{ecs::schedule::ScheduleLabel, prelude::*};
use crate::systems::scenes::{scene_loaded, CornScene};

#[derive(Debug, Clone, PartialEq, Eq, Hash, SystemSet)]
pub struct SceneSet<S: CornScene>(pub S);

/// Adds a app method to configure a state set in a schedule. The system set will run when in state during schedule.
/// This trait makes it so that we can configure a state set anywhere in the app and it will only configure once, and the same system set will be used everywhere.
pub trait AppSceneSet{
    /// Configures a set to run in a schedule only during the state
    fn configure_scene_set<P: SystemSet+Clone, S: CornScene, L:ScheduleLabel+Clone>(&mut self, schedule: L, scene: S, set: P) -> &mut Self;
}
impl AppSceneSet for App{
    fn configure_scene_set<P: SystemSet+Clone, S: CornScene, L:ScheduleLabel+Clone>(&mut self, schedule: L, scene: S, set: P) -> &mut Self {
        let plugin = SceneSetPlugin::new(schedule, scene, set);
        if !self.get_added_plugins::<SceneSetPlugin<P, S, L>>().contains(&&plugin){self.add_plugins(plugin);}
        self
    }
}

/// Plugin to configure a set to run during a state in a schedule. Used to ensure configuration is done only once when used in multiple places
pub struct SceneSetPlugin<P, S, L> where P: SystemSet+Clone, S: CornScene, L: ScheduleLabel+Clone{
    set: P,
    scene: S,
    schedule: L,
    name: String
}
impl<P: SystemSet+Clone, S: CornScene, L: ScheduleLabel+Clone> PartialEq for SceneSetPlugin<P, S, L>{
    fn eq(&self, other: &Self) -> bool {self.name==other.name}
    fn ne(&self, other: &Self) -> bool {self.name!=other.name}
}
impl<P: SystemSet+Clone, S: CornScene, L: ScheduleLabel+Clone> SceneSetPlugin<P, S, L>{pub fn new(schedule: L, scene: S, set: P)->Self{Self { 
    name: format!("StateSetPlugin: {:?} {:?}, {:?}", set, scene, schedule),
    set,
    scene,
    schedule
}}}
impl<P: SystemSet+Clone, S: CornScene, L: ScheduleLabel+Clone> Plugin for SceneSetPlugin<P, S, L>{
    fn build(&self, app: &mut App) {
        app
        .configure_sets(
            self.schedule.clone(),
            self.set.clone().run_if(scene_loaded(self.scene.clone()))
        );
    }
    fn name(&self) -> &str {&self.name}
}