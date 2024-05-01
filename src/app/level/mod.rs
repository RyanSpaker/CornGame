use bevy::{prelude::*, utils::{hashbrown::HashMap, HashSet, Uuid}};

/// Struct containing the state of the game level
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect, States)]
pub struct GlobalLevelState{
    /// The current level of the app
    pub level: Level,
    /// The update state of the level
    pub update_state: LevelUpdateState,
    /// The loading state of the level
    pub loading_state: LevelLoadingState
}

/// Enum containing all levels in the game
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect, States)]
pub enum Level{
    /// the main menu
    MainMenu,
    /// The main lobby one enters when entering the game
    Lobby,
    /// First level, a small simple maze
    SimpleMaze
}

/// Enum representing the update state of the level
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect, States)]
pub enum LevelUpdateState {
    /// The level should not be updated or rendered
    Disabled,
    /// The level should be rendered but not updated
    Static,
    /// The level should be updated and rendered
    Active
}

/// Enum representing the loading state of a level
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect, States)]
pub enum LevelLoadingState{
    Loading,
    Loaded
}

pub struct LevelDependencyConfig{
    id: String,
    loaded: bool,
    
}

pub struct LevelDependencieTracker{
    per_level: HashMap<Level, HashSet<String>>,
    global: HashSet<String>,
    loaded: HashSet<String>,
    dependencies: HashMap<String, LevelDependencyConfig>
}
/*
    LevelDependencies default will be where we define level dependencies
    When entering a level state, the resource will search its dependency list for unloaded dependencies that are needed
    
    Use a system set for running asset loading functions, including ones for setting up loading and for per frame updates of loading

    How to have dependencies inform the tracker if they are finished
    How to manage one-shot dependencies
    How to make dependencies that are simple or common, simple, and not overly complex as a result of the system

*/

