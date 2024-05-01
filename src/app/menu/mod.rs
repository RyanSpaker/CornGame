use bevy::prelude::*;
use std::{hash::Hash, fmt::Debug};

/// Struct representing the state of each menu system in the app.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect, States)]
pub struct GlobalMenuState{
    /// State of the app menus like the main menu, the options menu, and so on
    pub app_menu: MenuSetState<AppMenu>,
    /// The state of gameplay menus like the journal or inventory
    pub gameplay_menu: MenuSetState<GameMenu>
}

/// Enum representing the state of a menu set
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect, States)]
pub enum MenuSetState<T>
where T: Debug + Clone + PartialEq + Eq + Hash + Reflect + States
{
    /// The menu set is closed
    Closed,
    /// The menu set is opened, contains which specific menu is active, and the update state of the menu
    Opened(T, MenuUpdateState)
}

/// Enum representing the update state of the menu
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect, States)]
pub enum MenuUpdateState {
    /// The menu should not be updated
    Static,
    /// The menu should be updated
    Active
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

/// Enum containing all game menus
#[derive(Debug, Clone, PartialEq, Eq, Hash, Reflect, States)]
pub enum GameMenu{
    /// The inventory menu
    Inventory,
    /// The journal
    Journal
}
