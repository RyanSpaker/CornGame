
// abstracts keyboard input
use bevy::prelude::*;
use leafwing_input_manager::prelude::*;

#[derive(Actionlike, PartialEq, Eq, Clone, Copy, Hash, Debug, Reflect)]
pub enum Action {
    Crouch,
    Run,
    Move,
    Pan
}

impl Action {
    /// Define the default bindings to the input
    pub fn default_input_map() -> InputMap<Self> {
        let mut input_map = InputMap::default();

        // Default kbm input bindings
        input_map.insert(Self::Move, VirtualDPad::wasd());
        input_map.insert(Self::Crouch, KeyCode::ControlLeft);
        input_map.insert(Self::Run, KeyCode::ShiftLeft);
        input_map.insert(Self::Pan, DualAxis::mouse_motion());

        input_map
    }
}