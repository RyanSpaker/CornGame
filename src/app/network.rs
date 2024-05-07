/// What I want is full generality. This is the correct solution. Anything else is degenerate.
/// All players are clients, all players are servers. There may be another server.
/// servers talk to each other, client only talks to local server
/// if root server disconnects, next server in line becomes root server
/// in case of a central server, central server serves as the perminant root server
/// servers are in a tree from the root, each talks to its parent only, parent is aware of all descendants.
/// servers are aware of siblings, to facilitate reparenting
/// entities are owned by a server, there are three considerations
/// 1) clients drive forward unreplicated state
/// 2) servers drive forward replicated state
/// 3) servers take input from their clients for states they own
/// 4) servers take input from their parent for states they do not own
/// 
/// extra: mesh side channels. It is possible that it would be helpfull in cases to allow siblings to communicate as well. The rules to keep this coherant are unclear.
/// 
/// ex) character controller
/// 1) animation interpolation (assume this is unknown to servers)
/// 2) animation state, position, velocity interpolated by local server
/// 3) character controller drives position/velocity of (owned) local character
/// 4) that position/velocity is passed to other servers, which interpolate it.
/// 
/// confounded b/c physics is behind the scenes. more specifically:
/// - does the client code or server code set the position (ie. does client emit only inputs.)
/// - I think clients emitting inputs is annoying (what about menu items, do we send mouse clicks?)
/// - however, we want to have the option of having a root server own everything for anticheat (not important to us, but in theory good)
/// - this implies a need for message passing, and if the message passed to the local server works, then it can be forwarded up the chain to the owning server automatically.
/// - and the local server can still process the message, and avoid waiting for the parent server.
/// - so, server and client should never touch the same state, likewise, npc should be in the server, and so-on.
/// 
/// So we have for npcs:
/// - main tick (owning server)
/// - interpolation tick (both)
/// - replicating handler (slave server)
///  
/// TODO how does client inputs feed into above.
/// 
/// problem if client ever touches physics is that it can't be moved up the chain.
/// 
/// replication: 
/// - creation and destruction of entities
/// - add remove components
/// 
/// Example for given object.
/// - we want to send over the name of the asset, but not the mesh.
/// - for scenes it is probably fine to send over all the subentities, this opens the door to easy procedural changes, and it isn't that much data.
/// - unclear how autoadded components work, unclear how to deal with components which should be synced for some entities and not others.
///   - https://docs.rs/bevy_replicon/latest/bevy_replicon/core/dont_replicate/trait.CommandDontReplicateExt.html#tymethod.dont_replicate
/// Issues: probably want to add in support for ggrs type procedural rollback, for things like physics, with automatic recovery, in order to reduce bandwidth.
/// - this would prevent need for sending all entities physics updates, for example.
/// - idea: hierarchical, fallible procedural rollback.
/// 
/// reason to think my design is good:
/// 1) data (components) and entities are either local or synced, they can be muddled or cleanly delineated, but it is factually the case that the information falls in 2 and only 2 categories
/// 2) you have alot of choices in design, and generally, using components as the unit of syncing makes sense. 
/// 3) synced data lives on a network of computers, if the server is where synced data lives, then every computer is part of the "server"
/// 4) keeping client server arch for single or multiplayer means one less thing to think about
/// 5) the symmetry makes more complex multiplayer an option
/// 6) and the symmetry equates to less thinking for the dev
/// 7) from the point of view of client code, messages to the server are messages to the syncing layer.
/// 8) the design of the syncing layer is not important to the client code (some server reads the message, and responds)
/// 
/// The client is all code that doesn't touch networking, the server is all code that does.
/// Only the server can create or modify replicated components.
/// Only the client can get user input.
/// 
/// conclusion: bevy replicon is a good *first order* aproximation of what we want, we may fork it or build on top of it to get server fallback and the correct^tm design I describe above (could be upstreamed maybe)
/// 
/// much like my novel software, I'm going to say we play fast and loose with this one.
/// 
/// Starting minimal example:
/// - get 2 instances of game talking to each other locally
/// - server side spawns 2 cubes, one for each player
/// - both control with arrow keys.

use std::{
    error::Error,
    net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket},
    time::SystemTime,
};

use bevy::{ecs::{entity, event}, prelude::*, transform::commands};
use bevy_replicon::{client::{client_mapper::ServerEntityMap, ServerEntityTicks}, prelude::*};
use bevy_replicon_renet::{
    renet::{
        transport::{
            ClientAuthentication, NetcodeClientTransport, NetcodeServerTransport,
            ServerAuthentication, ServerConfig,
        },
        ConnectionConfig, RenetClient, RenetServer,
    },
    RenetChannelsExt, RepliconRenetPlugins,
};
use bevy_tnua::controller::TnuaController;
use clap::Parser;
use serde::{Deserialize, Serialize};

use crate::ecs::main_camera::MainCamera;

use super::loading::TestBox;

// #[derive(Component, Serialize, Deserialize)]
// pub struct Blueprint<M>(M);

// we need the ability to replicate objects in a scene file
// the only option is to either replicated every part of the whole scene
// or use regular scene loading on the client and then somehow merge/sync/cleanup the entities replicated from the server and cleaned up from the client.
// 
// idea, use name compenent, merge server components into the loaded ones, send server mapping for client's entity id, somehow delete / cleanup the old entity.

pub struct CornNetworkingPlugin;
impl Plugin for CornNetworkingPlugin{
    fn build(&self, app: &mut App) {
        app.init_resource::<Cli>();
        app.add_plugins((RepliconPlugins, RepliconRenetPlugins));

        // TODO move this to top level app
        app.add_systems(Startup, read_cli.map(Result::unwrap)); 

        //TODO make this generic blueprint system
        app.add_systems(Update, (super::loading::TestBox::spawn).after(ClientSet::Receive));

        // TODO make this more robust (upstreamable)
        app.add_systems(PreUpdate, name_sync_test.after(ClientSet::Receive).run_if(has_authority.map(|b|!b)));

        // Replication of core stuff
        app.replicate::<Transform>();
        app.replicate::<Name>();

        // TODO client interpolation, plus maybe move these to physics system setup
        use bevy_xpbd_3d::prelude::*;
        app.replicate::<LinearVelocity>();
        app.replicate::<AngularVelocity>();

        // blueprints
        app.register_type::<TestBox>();
        app.replicate::<TestBox>();

        // This is the classic case of I wish I could put the line of code in 2 places
        // TODO replicate character controller

        //app.add_systems(Update, scene_add_repl_test.after(ClientSet::Receive).run_if(has_authority));
        
        // app.add_client_event::<UpdateMapping>(ChannelKind::Ordered)
        //     .add_systems(Update, handle_update_mapping.run_if(has_authority));

        // app.add_server_event::<DestroyOld>(ChannelKind::Ordered)
        //     .add_systems(Update, handle_delete_old);  
    }
}

// #[derive(Clone, Copy, Debug, Deserialize, Event, Serialize)]
// struct UpdateMapping(Entity, Entity); /*new, old*/

// #[derive(Clone, Copy, Debug, Deserialize, Event, Serialize)]
// struct DestroyOld(Entity);

// fn handle_update_mapping(
//     mut rx: EventReader<FromClient<UpdateMapping>>,
//     mut entity_map: ResMut<ClientEntityMap>,
//     mut tx: EventWriter<ToClients<DestroyOld>>
// ) {
//     for FromClient { client_id, event } in rx.read() {
//         let server_entity = event.1; // You can insert more components, they will be sent to the client's entity correctly.

//         entity_map.insert(
//             *client_id,
//             ClientMapping {
//                 server_entity,
//                 client_entity: event.0,
//             },
//         );

//         //tx.send(DestroyOld(event.1));
//     }
// }

// /// delete the replicated entity after it gets remapped
// fn handle_delete_old(
//     mut commands: Commands,
//     mut bullet_events: EventReader<DestroyOld>,
// ) {
//     for event in bullet_events.read() {
//         commands.entity(event.0).despawn();
//     }
// }

/// name sync
/// TODO maybe don't require replication on query
fn name_sync_test(
    query: Query<(Entity, &Name), Added<Name>>,
    all: Query<(Entity, &Name)>,
    mut mapper: ResMut<ServerEntityMap>,
    mut dumb: ResMut<ServerEntityTicks>,
    mut commands: Commands
){
    for new in query.iter() {
        if let Some(old) = all.iter().find(|v|v.1 == new.1 && v.0 != new.0){
            let old_is_repl = mapper.to_server().get(&old.0).cloned();
            let new_is_repl = mapper.to_server().get(&new.0).cloned();

            dbg!(old, new, old_is_repl, new_is_repl);

            #[allow(clippy::unnecessary_unwrap)] 
            if old_is_repl.is_some() && new_is_repl.is_none() {
                // gameobject replicated *before* client scene-loader spawns it
                mapper.remove_by_client(old.0);
                mapper.insert(old_is_repl.unwrap(), new.0);
                let a = dumb.remove(&old.0).unwrap();
                dumb.insert(new.0, a);

                commands.entity(new.0).insert(Replication);
                commands.add(crate::util::clone_entity::CloneEntity {
                    source: old.0,
                    destination: new.0,
                });
                commands.entity(old.0).despawn();
            }

            #[allow(clippy::unnecessary_unwrap)] 
            if new_is_repl.is_some() && old_is_repl.is_none(){
                // gameobject replicated *after* client scene-loader spawns it
                mapper.remove_by_client(new.0);
                mapper.insert(new_is_repl.unwrap(), old.0);
                let a = dumb.remove(&new.0).unwrap();
                dumb.insert(old.0, a);

                commands.entity(old.0).insert(Replication);
                commands.add(crate::util::clone_entity::CloneEntity {
                    source: new.0,
                    destination: old.0,
                });
                commands.entity(new.0).despawn();
            }
        }
    }
}

/// hacky test to add replication to all objects in scene (except meshes, in order to avoid a weird name overlap)
/// TODO need to use something other than name for this.
pub fn scene_add_repl_test(
    query: Query<(Entity, &Name), (With<Transform>, Without<Handle<Mesh>>, Without<Replication>, Added<Name>, Without<Camera> )>,
    mut commands: Commands
){
    for (id,name) in query.iter() {
        if name.as_ref() == "Plane"{
            dbg!(name);
            commands.entity(id).insert(Replication);
        }
    }
}

const PORT: u16 = 5000;
const PROTOCOL_ID: u64 = 0;

#[derive(Parser, PartialEq, Resource)]
enum Cli {
    Server {
        #[arg(short, long, default_value_t = PORT)]
        port: u16,
    },
    Client {
        #[arg(short, long, default_value_t = Ipv4Addr::LOCALHOST.into())]
        ip: IpAddr,

        #[arg(short, long, default_value_t = PORT)]
        port: u16,
    },
}

impl Default for Cli {
    fn default() -> Self {
        Self::parse()
    }
}

fn read_cli(
    mut commands: Commands,
    cli: Res<Cli>,
    channels: Res<RepliconChannels>,
) -> Result<(), Box<dyn Error>> {
    match *cli {
        Cli::Server { port } => {
            let server_channels_config = channels.get_server_configs();
            let client_channels_config = channels.get_client_configs();

            let server = RenetServer::new(ConnectionConfig {
                server_channels_config,
                client_channels_config,
                ..Default::default()
            });

            let current_time = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?;
            let public_addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), port);
            let socket = UdpSocket::bind(public_addr)?;
            let server_config = ServerConfig {
                current_time,
                max_clients: 10,
                protocol_id: PROTOCOL_ID,
                authentication: ServerAuthentication::Unsecure,
                public_addresses: vec![public_addr],
            };
            let transport = NetcodeServerTransport::new(server_config, socket)?;

            commands.insert_resource(server);
            commands.insert_resource(transport);
            // commands.spawn(PlayerBundle::new(
            //     ClientId::SERVER,
            //     Vec2::ZERO,
            //     Color::GREEN,
            // ));
        }
        Cli::Client { port, ip } => {
            let server_channels_config = channels.get_server_configs();
            let client_channels_config = channels.get_client_configs();

            let client = RenetClient::new(ConnectionConfig {
                server_channels_config,
                client_channels_config,
                ..Default::default()
            });

            let current_time = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?;
            let client_id = current_time.as_millis() as u64;
            let server_addr = SocketAddr::new(ip, port);
            let socket = UdpSocket::bind((ip, 0))?;
            let authentication = ClientAuthentication::Unsecure {
                client_id,
                protocol_id: PROTOCOL_ID,
                server_addr,
                user_data: None,
            };
            let transport = NetcodeClientTransport::new(current_time, authentication, socket)?;

            commands.insert_resource(client);
            commands.insert_resource(transport);
        }
    }

    Ok(())
}