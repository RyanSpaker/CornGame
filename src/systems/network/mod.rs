use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use bevy::ecs::component::HookContext;
use bevy::ecs::world::DeferredWorld;
use bevy::prelude::*;
use lightyear::netcode::{Key, NetcodeClient, NetcodeServer};
use lightyear::prelude::client::WebTransportClientIo;
#[cfg(not(target_family = "wasm"))]
use lightyear::prelude::server::{Start, Started, WebTransportServerIo};
use lightyear::prelude::*;

use bevy::ecs::system::{Query, Res};
use serde::{Deserialize, Serialize};

use crate::systems::character::CharacterNetworkPlugin;
use crate::systems::physics::CornPhysicsPluginNetworkPlugin;

pub struct CornNetworkingPlugin;
impl Plugin for CornNetworkingPlugin {
    fn build(&self, app: &mut App) {
        // This should be determined by lightyear
        let tick_duration = Duration::from_secs_f64(1.0 / 64.0);

        // add both plugins, runtime config.
        // app.add_plugins(SharedPlugins {
        //     tick_duration, 
        // }); 
        #[cfg(not(target_family = "wasm"))]
        app.add_plugins(server::ServerPlugins { tick_duration });
        app.add_plugins(client::ClientPlugins { tick_duration });

        app.add_systems(Update, network_on_start_system.run_if(run_once));
        app.insert_resource(NetworkCrap {
            address: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 42217),
            conditioner: LinkConditionerConfig::average_condition().into(),
        });

        app.register_component::<Name>();
        app.register_component::<ReplicateAuto>()
            .with_replication_config(
                ComponentReplicationConfig { replicate_once: true, ..Default::default() }
            );
        app.add_systems(Update, ReplicateAuto::dumb);

        app.add_plugins((
            CornPhysicsPluginNetworkPlugin,
            CharacterNetworkPlugin
        ));
        // app.register_component::<ReplicateOtherClients>();
        // app.add_systems(FixedUpdate, replicate_other_clients);
    }
}

#[derive(Debug, Resource, Reflect)]
#[reflect(Resource)]
struct NetworkCrap {
    address: SocketAddr,
    conditioner: Option<LinkConditionerConfig>,
}

fn network_on_start_system(mut commands: Commands, res: Res<crate::Cli>) {
    // TODO replace with generic cli dev hooks
    if res.server {
        #[cfg(not(target_arch = "wasm32"))]
        commands.run_system_cached(start_server);
        #[cfg(target_arch = "wasm32")]
        unimplemented!()
    }
    
    if res.client {
        commands.run_system_cached(start_client);
    }
}

fn start_client(server: Query<Entity, With<Server>>, mut commands: Commands, crap: Res<NetworkCrap>) {
    let conditioner = crap
        .conditioner
        .as_ref()
        .map(|c| RecvLinkConditioner::new(c.clone()));

    let id = std::process::id();

    let auth = Authentication::Manual {
        server_addr: crap.address.clone(),
        client_id: id.into(),
        private_key: Key::default(),
        protocol_id: 0,
    };
    
    let mut client = commands.spawn((
        Client::default(),
        
        Link::new(conditioner),
        ReplicationReceiver::default(),
        PredictionManager::default(),
        InterpolationManager::default(),
        Name::from("Client"),
        ReplicationSender::new(
            Duration::from_millis(100),
            SendUpdatesMode::SinceLastAck,
            false,
        )
    ));

    let certificate_digest = {
        #[cfg(target_family = "wasm")]
        {
            //include_str!("../../certificates/digest.txt").to_string()
            "".to_string()
        }
        #[cfg(not(target_family = "wasm"))]
        {
            "".to_string()
        }
    };

    if server.is_empty(){
        info!("starting client");
        client.insert(
            NetcodeClient::new(auth, client::NetcodeConfig::default()).unwrap());
        client.insert(WebTransportClientIo {
            certificate_digest,
        });
    }else{
        // hostserver
        info!("starting host-client");
        client.insert(LinkOf { server: server.single().unwrap() });
    }

    client.trigger(Connect);
}

#[cfg(not(target_family = "wasm"))]
fn start_server(mut commands: Commands, crap: Res<NetworkCrap>) {
    let server_addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), crap.address.port());

    // TODO env var
    let certificate = Identity::self_signed(vec![
        "127.0.0.1".to_string(),
        "::1".to_string(),
        "localhost".to_string(),
    ])
    .unwrap();
    let digest = certificate.certificate_chain().as_slice()[0].hash();
    println!("🔐 Certificate digest: {digest}");

    let mut server = commands.spawn((
        Name::from("server"),
        Started, // webtransport code doesn't add this.
        LocalAddr(server_addr),
        WebTransportServerIo {
            certificate,
        },
        NetcodeServer::new(server::NetcodeConfig::default()),
    ));
    server.with_child((
        Name::from("on_connect"),
        Observer::new(handle_new_client),
    ));
    server.trigger(Start);
}

/// Whenever a new client connects to the server, a new entity will get spawned with
/// the `Connected` component, which represents the connection between the server and that specific client.
///
/// You can add more components to customize how this connection, for example by adding a
/// `ReplicationSender` (so that the server can send replication updates to that client)
/// or a `MessageSender`.
fn handle_new_client(
    trigger: Trigger<OnAdd, Connected>,
    crap: Res<NetworkCrap>,
    mut commands: Commands,
) {
    info!(client = %trigger.target(), "new client connected");
    let conditioner = crap
        .conditioner
        .as_ref()
        .map(|c| RecvLinkConditioner::new(c.clone())); // TODO how to attach

    commands.entity(trigger.target()).insert((
        ReplicationSender::new(
            Duration::from_millis(100),
            SendUpdatesMode::SinceLastSend,
            false,
        ),
        ReplicationReceiver::default(),
    ));
}

#[derive(Debug, Component, Serialize, Deserialize, PartialEq)]
#[component(on_add = ReplicateAuto::on_insert)]
#[require(DisableReplicateHierarchy)]
pub struct ReplicateAuto;
impl ReplicateAuto {
    /// Hook to automatically add Replicate with the correct mode depending on presence of Server or Client.
    pub fn on_insert(mut world: DeferredWorld, context: HookContext) {
        world.commands().queue(move |world: &mut World| {
            // Note: Replicating doesn't seem to work
            if let Some(r) = world.get::<Replicated>(context.entity) {
                let peer = r.from;
                
                if world
                    .query_filtered::<Entity, With<Server>>()
                    .single(&world)
                    .is_ok() 
                {
                    world
                        .entity_mut(context.entity)
                        .insert(Replicate::to_clients(NetworkTarget::AllExceptSingle(peer)));
                }
                // came from network, but we might add this in on-add logic, so just ignore
                world.entity_mut(context.entity).remove::<Self>();
                return;
            }
            
            if world
                .query_filtered::<Entity, With<Server>>()
                .single(&world)
                .is_ok()
            {
                world
                    .entity_mut(context.entity)
                    .insert(Replicate::to_clients(NetworkTarget::All));
            }
            else if world
                .query_filtered::<Entity, With<Client>>()
                .single(&world)
                .is_ok()
            {
                world
                    .entity_mut(context.entity)
                    .insert(Replicate::to_server());
            }
        });
    }

    pub fn dumb(
        new_client: Query<Entity, Added<Client>>,
        // new_server: Query<Entity, (With<Server>, Added<Started>)>, Started is never added
        new_server: Query<Entity, Added<Server>>,
        query: Query<Entity, With<ReplicateAuto>>, 
        mut commands: Commands
    ){
        if new_client.is_empty() && new_server.is_empty() {
            return
        }
        for e in query.iter() {
            trace!("rerun ReplicateAuto logic");
            if ! new_client.is_empty(){
                commands.entity(e).insert(Replicate::to_server());
            }else{
                commands.entity(e).insert(Replicate::to_clients(NetworkTarget::All));
            }
        }
    }
}

// #[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq, Reflect)]
// #[reflect(Component)]
// #[component(storage = "SparseSet")]
// pub struct ReplicateOtherClients(
//     /// parent_sync
//     pub bool,
// );

// pub fn replicate_other_clients(
//     identity: NetworkIdentity,
//     mut commands: Commands,
//     replicated_cursor: Query<
//         (
//             Entity,
//             Option<&AuthorityPeer>,
//             Has<HasAuthority>,
//             Has<Replicated>,
//             &ReplicateOtherClients,
//         ),
//         Added<ReplicateOtherClients>,
//     >,
// ) {
//     for (entity, peer, _, replicated, value) in replicated_cursor.iter() {
//         if identity.is_server() || identity.is_host_server() {
//             if let Some(AuthorityPeer::Client(client_id)) = peer {
//                 commands.entity(entity).insert((
//                     ControlledBy {
//                         target: NetworkTarget::Single(*client_id),
//                         lifetime: server::Lifetime::SessionBased,
//                     },
//                     ReplicateToClient {
//                         target: NetworkTarget::AllExceptSingle(*client_id),
//                     },
//                 ));
//             }
//             if !replicated {
//                 let mut e = commands.entity(entity);
//                 e.insert((ReplicateToClient::default(),));
//                 if value.0 {
//                     e.insert(ChildOfSync::default());
//                 }
//             }
//         } else if identity.is_client() && !replicated {
//             let mut e = commands.entity(entity);
//             e.insert((ReplicateToServer,));
//             if value.0 {
//                 e.insert(ChildOfSync::default());
//             }
//         }

//         // for all cursors we have received, add a Replicate component so that we can start replicating it
//         // to other clients
//     }
// }
