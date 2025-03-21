use std::net::{Ipv4Addr, SocketAddr};

use avian3d::prelude::*;
use bevy::prelude::*;
use clap::Parser;
use lightyear::prelude::server::{NetConfig, ServerCommands, ServerTransport};
use lightyear::prelude::*;

use lightyear::client::components::{ComponentSyncMode, LerpFn};
use lightyear::client::config::ClientConfig;
use lightyear::prelude::client::{Authentication, ClientCommands, ClientConnection, ClientTransport};
use lightyear::transport::config::SharedIoConfig;
use lightyear::utils::avian3d::*;
use lightyear::utils::bevy::TransformLinearInterpolation;
use server::ServerConfig;

pub struct CornNetworkingPlugin;
impl Plugin for CornNetworkingPlugin{
    fn build(&self, app: &mut App) {
        // TODO: currently we need ServerPlugins to run first, because it adds the
        // SharedPlugins. not ideal
        app.add_plugins(server::ServerPlugins {
            config: ServerConfig::default(),
        });
        app.add_plugins(client::ClientPlugins {
            config: ClientConfig::default(),
        });
        app.add_systems(Startup, network_on_start_system);

        app.register_component::<Name>(ChannelDirection::ServerToClient);
        
        // // Physics
        // app.register_component::<LinearVelocity>(ChannelDirection::Bidirectional)
        //     .add_prediction(ComponentSyncMode::Full);

        // app.register_component::<AngularVelocity>(ChannelDirection::Bidirectional)
        //     .add_prediction(ComponentSyncMode::Full);

        // app.register_component::<ExternalForce>(ChannelDirection::Bidirectional)
        //     .add_prediction(ComponentSyncMode::Full);

        // app.register_component::<ExternalImpulse>(ChannelDirection::Bidirectional)
        //     .add_prediction(ComponentSyncMode::Full);

        // // Do not replicate Transform when we are replicating Position/Rotation!
        // // See https://github.com/cBournhonesque/lightyear/discussions/941
        // // app.register_component::<Transform>(ChannelDirection::Bidirectional)
        // //     .add_prediction(ComponentSyncMode::Full);

        // app.register_component::<ComputedMass>(ChannelDirection::Bidirectional)
        //     .add_prediction(ComponentSyncMode::Full);

        // Position and Rotation have a `correction_fn` set, which is used to smear rollback errors
        // over a few frames, just for the rendering part in postudpate.
        //
        // They also set `interpolation_fn` which is used by the VisualInterpolationPlugin to smooth
        // out rendering between fixedupdate ticks.
        // app.register_component::<Position>(ChannelDirection::ServerToClient)
        //     .add_prediction(ComponentSyncMode::Full)
        //     .add_interpolation_fn(position::lerp)
        //     .add_interpolation(ComponentSyncMode::Full)
        //     .add_correction_fn(position::lerp);

        // app.register_component::<Rotation>(ChannelDirection::ServerToClient)
        //     .add_prediction(ComponentSyncMode::Full)
        //     .add_interpolation_fn(rotation::lerp)
        //     .add_interpolation(ComponentSyncMode::Full)
        //     .add_correction_fn(rotation::lerp);

        // do not replicate Transform but make sure to register an interpolation function
        // for it so that we can do visual interpolation
        // (another option would be to replicate transform and not use Position/Rotation at all)
        // app.add_interpolation::<Transform>(ComponentSyncMode::None);
        //app.add_interpolation_fn::<Transform>(TransformLinearInterpolation::lerp);

        app.register_component::<Transform>(ChannelDirection::ServerToClient);
    }
}


fn network_on_start_system(
    mut commands: Commands,
    res: Res<crate::Cli>
){
    // TODO replace with generic cli dev hooks
    if res.server {
        commands.run_system_cached(start_server);
    } else if res.client {
        commands.run_system_cached(start_client);
    }
}

const PORT : u16 = 4444;

pub fn start_server(
    mut commands: Commands,
    mut config: ResMut<ServerConfig>,
){
    info!("We are the host of the game!");

    // set the client connection to be local
    let server_addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), PORT);
    
    // You need to provide the private key and protocol id when building the `NetcodeConfig`
    let netcode_config = server::NetcodeConfig::default()
        .with_protocol_id(default())
        .with_key(default());
    
    
    let net_config = server::NetConfig::Netcode {
        config: netcode_config,
        io: server::IoConfig::from_transport(ServerTransport::UdpSocket(server_addr))
    };

    // Here we only provide a single net config, but you can provide multiple!
    config.net = vec![net_config];

    commands.start_server();
}

pub fn start_client(
    mut commands: Commands,
    mut config: ResMut<ClientConfig>,
){
    info!("The game is hosted by another client. Connecting to the host...");
    // update the client config to connect to the game host
    let client_addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), 0);
    let server_addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), PORT);
    let io_config = client::IoConfig::from_transport(client::ClientTransport::UdpSocket(client_addr));
    let auth = Authentication::Manual {
        // server's IP address
        server_addr,
        // ID to uniquely identify the client
        client_id: default(),
        // private key shared between the client and server
        private_key: default(),
        // PROTOCOL_ID identifies the version of the protocol
        protocol_id: default(),
    };
    let net_config = client::NetConfig::Netcode {
        auth,
        io: io_config,
        config: default()
    };
    config.net = net_config; 
    
    commands.connect_client();
}


// note: https://github.com/cBournhonesque/lightyear/blob/2037d468f513569deee79ca24e0eb06c2a4c35ea/examples/distributed_authority/src/server.rs#L58C1-L79C2