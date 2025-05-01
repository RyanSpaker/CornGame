use bevy::prelude::*;
use std::{hash::Hash, fmt::Debug};

/// Struct representing the state of the app menus
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect, States)]
pub enum AppMenuState{
    /// The system menus are not currently open
    Closed,
    /// The system menu is open. contains the currently opened menu
    Opened(AppMenu)
}

/// Enum containing all app menus
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect, States)]
pub enum AppMenu{
    /// The first menu the game show upon starting, after entering the lobby it no longer shows up
    MainMenu,
    /// The menu used to alter game settings
    Options,
    /// A menu for showing the game credits
    Credits,
    /// A menu that shows up when the player pauses the game
    PauseMenu
}
