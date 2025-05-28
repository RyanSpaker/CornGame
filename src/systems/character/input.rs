// abstracts keyboard input
use bevy::prelude::*;
use leafwing_input_manager::prelude::*;

#[derive(Actionlike, PartialEq, Eq, Clone, Copy, Hash, Debug, Reflect)]
pub enum CornCharacterInput {
    Crouch,
    Run,
    #[actionlike(DualAxis)]
    Move,
    #[actionlike(DualAxis)]
    Pan,

    Toggle,
}

impl CornCharacterInput {
    /// Define the default bindings to the input
    pub fn default_input_map() -> InputMap<Self> {
        let mut input_map = InputMap::default();

        // Default kbm input bindings
        input_map.insert_dual_axis(Self::Move, VirtualDPad::wasd());
        input_map.insert(Self::Crouch, KeyCode::ControlLeft);
        input_map.insert(Self::Run, KeyCode::ShiftLeft);
        input_map.insert_dual_axis(Self::Pan, MouseMove::default());

        input_map.insert(Self::Toggle, KeyCode::Backquote);

        input_map
    }
}
