use avian3d::prelude::*;
use bevy::prelude::*;

use lightyear::client::components::{ComponentSyncMode, LerpFn};
use lightyear::prelude::*;
use lightyear::utils::avian3d::*;
use server::ServerConfig;

pub struct CornNetworkingPlugin;
impl Plugin for CornNetworkingPlugin{
    fn build(&self, app: &mut App) {
        // TODO: currently we need ServerPlugins to run first, because it adds the
        // SharedPlugins. not ideal
        app.add_plugins(server::ServerPlugins {
            config: ServerConfig::default(),
        });
        
        app.register_component::<Position>(ChannelDirection::Bidirectional)
            .add_prediction(ComponentSyncMode::Full)
            .add_interpolation(ComponentSyncMode::Full)
            .add_interpolation_fn(position::lerp)
            .add_correction_fn(position::lerp);

        app.register_component::<Rotation>(ChannelDirection::Bidirectional)
            .add_prediction(ComponentSyncMode::Full)
            .add_interpolation(ComponentSyncMode::Full)
            .add_interpolation_fn(rotation::lerp)
            .add_correction_fn(rotation::lerp);

        // NOTE: interpolation/correction is only needed for components that are visually displayed!
        // we still need prediction to be able to correctly predict the physics on the client
        app.register_component::<LinearVelocity>(ChannelDirection::Bidirectional)
            .add_prediction(ComponentSyncMode::Full);

        app.register_component::<AngularVelocity>(ChannelDirection::Bidirectional)
            .add_prediction(ComponentSyncMode::Full);

        // channels
        // app.add_channel::<Channel>(ChannelSettings {
        //     mode: ChannelMode::OrderedReliable(ReliableSettings::default()),
        //     ..default()
        // });
    }
}