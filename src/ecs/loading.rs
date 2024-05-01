use bevy::{asset::UntypedAssetId, prelude::*};




/*
    Loading Dependency Format:

    Upon entering a loading state functions are called which spawn Level Dependency Components

    level dependency spawning functions:
        can be scheduled manually by specifying either OnEnter(state) or during Update since state transition happens in PostUpdate
        Common resources like assets can have a special system for managing just them

    Needs:
        Easily figure out when all dependencies are finished
            store initial hashset of unfinished dependencies. Query changes dependencies, remove from set. when set empty, dependencies are done
        Easily query a list of all dependencies in a constant order (iter_many?)

        Store optional progress and metadata

*/

/// Component representing a loading task. contains the untyped asset id, as well as whether the task is finished 
#[derive(Debug, Clone, PartialEq, Eq, Hash, TypePath, Component)]
pub struct LoadingTask{
    /// Asset id of the asset being loaded
    asset_id: UntypedAssetId,
    /// bool recording whether the asset is loaded or not
    finished: bool
}
impl LoadingTask{
    /// A system which updates all loading task finish bools by querying their assets load state
    pub fn update_load_state(assets: Res<AssetServer>, mut tasks: Query<&mut LoadingTask>) {
        for mut task in &mut tasks{
            if task.finished || !assets.is_loaded_with_dependencies(task.asset_id) {continue;}
            task.finished = true;
        }
    }
}