use bevy::prelude::*;
use std::{fmt::Debug, hash::Hash};

use super::level::GlobalLevelState;

/// Enum for representing the state of the application
#[derive(Default, Debug, Clone, PartialEq, Eq, Hash, Reflect, States)]
pub enum AppState{
    /// Initial State of the app, responsible for loading bare essential assets, and preparing for a main menu scene
    #[default] Startup,
    /// State after loading, represents an app that is ready to be interacted with. Contains a Menu state and a Level state
    Open(GlobalMenuState, GlobalLevelState)
}