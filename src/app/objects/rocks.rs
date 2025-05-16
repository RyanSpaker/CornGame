/// A group of stones which can be stacked via an interaction.
/// 

#[derive(Debug, Clone, Component, Reflect)]
#[reflect(Component)]
pub struct Stones;


/// Stack operation
/// Pick random order of stones
/// Stack them up one by one using a curve for animation
/// 