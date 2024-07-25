use bevy::prelude::*;

/// Struct containing the state of the game level
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect, States)]
pub struct GameplayState{
    /// State of the current level
    pub level_state: LevelState,
    /// State of the gameplay menu
    pub menu_state: GameMenuState
}

/// Total state of the gameplay level
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect, States)]
pub struct LevelState{
    /// The current level of the app
    pub level: Level,
    /// The update state of the level
    pub update_state: UpdateState,
    /// The loading state of the level
    pub loading_state: LoadingState
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

/// Enum representing the update state of the level or menu
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect, States)]
pub enum UpdateState {
    /// The level or menu should not be updated or rendered
    Disabled,
    /// The level or menu should be rendered but not updated
    Static,
    /// The level or menu should be updated and rendered
    Active
}

/// Enum representing the loading state of a level
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect, States)]
pub enum LoadingState{
    /// Level is currently loading
    Loading,
    /// Level is fully loaded
    Loaded
}

/// Struct representing the state of the gameplay menus
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect, States)]
pub enum GameMenuState{
    /// The game menus are not currently open
    Closed,
    /// A game menu is open. contains the currently opened menu and the update state of the menu
    Opened(GameMenu, UpdateState)
}

/// Enum containing all game menus
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect, States)]
pub enum GameMenu{
    /// Journal Menu (like in phasmophobia)
    Journal,
    /// Inventory menyu
    Inventory,
}
