pub mod vote;

use bevy::prelude::*;

#[derive(Default, Debug, Clone)]
pub struct ScanPrepassPlugin;
impl Plugin for ScanPrepassPlugin{
    fn build(&self, app: &mut App) {
        app.add_plugins(vote::VoteScanPlugin);
    }
}