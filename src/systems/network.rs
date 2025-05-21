use std::hash::{DefaultHasher, Hasher};
use std::net::{Ipv4Addr, SocketAddr};
use std::time::Duration;

use avian3d::prelude::*;
use bevy::ecs::query::QueryData;
use bevy::prelude::*;
use clap::Parser;
use lightyear::prelude::server::{AuthorityPeer, ControlledBy, NetConfig, ReplicationTarget, ServerCommandsExt, ServerTransport};
use lightyear::prelude::*;

use lightyear::client::components::{ComponentSyncMode, LerpFn};
use lightyear::client::config::ClientConfig;
use lightyear::prelude::client::{Authentication, ClientCommandsExt, ClientConnection, ClientTransport, ReplicateToServer};
use lightyear::shared::replication::components::InitialReplicated;
use lightyear::transport::config::SharedIoConfig;
use lightyear::utils::avian3d::*;
use lightyear::utils::bevy::TransformLinearInterpolation;
use server::ServerConfig;
use bevy::ecs::system::{SystemParam, Query, Res};

pub struct CornNetworkingPlugin;
impl Plugin for CornNetworkingPlugin{
    fn build(&self, app: &mut App) {
        // TODO: currently we need ServerPlugins to run first, because it adds the
        // SharedPlugins. not ideal
        let shared = SharedConfig { 
            server_replication_send_interval: Duration::from_millis(100), 
            client_replication_send_interval: Duration::from_millis(100), 
            ..default() 
        };

        app.add_plugins(client::ClientPlugins {
            config: ClientConfig{
                shared,
                ..default()
            },
        });
        app.add_plugins(server::ServerPlugins {
            config: ServerConfig{
                shared,
                ..default()
            }
        });

        app.add_systems(Startup, network_on_start_system);
        app.add_systems(FixedUpdate, replicate_other_clients);

        app.register_component::<Name>(ChannelDirection::Bidirectional);
        app.register_component::<ReplicateOtherClients>(ChannelDirection::Bidirectional);
        
        // // Physics
        app.register_component::<LinearVelocity>(ChannelDirection::Bidirectional)
            .add_prediction(ComponentSyncMode::Full);

        app.register_component::<AngularVelocity>(ChannelDirection::Bidirectional)
            .add_prediction(ComponentSyncMode::Full);

        app.register_component::<ExternalForce>(ChannelDirection::Bidirectional)
            .add_prediction(ComponentSyncMode::Full);

        app.register_component::<ExternalImpulse>(ChannelDirection::Bidirectional)
            .add_prediction(ComponentSyncMode::Full);

        // // Do not replicate Transform when we are replicating Position/Rotation!
        // // See https://github.com/cBournhonesque/lightyear/discussions/941
        // // app.register_component::<Transform>(ChannelDirection::Bidirectional)
        // //     .add_prediction(ComponentSyncMode::Full);

        app.register_component::<ComputedMass>(ChannelDirection::Bidirectional)
            .add_prediction(ComponentSyncMode::Full);

        // Position and Rotation have a `correction_fn` set, which is used to smear rollback errors
        // over a few frames, just for the rendering part in postudpate.
        //
        // They also set `interpolation_fn` which is used by the VisualInterpolationPlugin to smooth
        // out rendering between fixedupdate ticks.
        // app.register_component::<Position>(ChannelDirection::Bidirectional)
        //     .add_prediction(ComponentSyncMode::Full)
        //     .add_interpolation_fn(position::lerp)
        //     .add_interpolation(ComponentSyncMode::Full)
        //     .add_correction_fn(position::lerp);

        // app.register_component::<Rotation>(ChannelDirection::Bidirectional)
        //     .add_prediction(ComponentSyncMode::Full)
        //     .add_interpolation_fn(rotation::lerp)
        //     .add_interpolation(ComponentSyncMode::Full)
        //     .add_correction_fn(rotation::lerp);

        // do not replicate Transform but make sure to register an interpolation function
        // for it so that we can do visual interpolation
        // (another option would be to replicate transform and not use Position/Rotation at all)

        app.register_component::<Transform>(ChannelDirection::Bidirectional)
            .add_interpolation(ComponentSyncMode::Full)
            .add_interpolation_fn(TransformLinearInterpolation::lerp);

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
    mut client_config: ResMut<ClientConfig>,
    fixed_time: Res<Time<Fixed>>,
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
        // .with_conditioner(LinkConditionerConfig { incoming_latency: Duration::from_millis(200), incoming_jitter: default(), incoming_loss: 0.0  })
    };

    // Here we only provide a single net config, but you can provide multiple!
    config.net = [net_config].to_vec();
    //config.shared.mode = Mode::HostServer;

    // NOTE: lightyear does not autodetect fixed timestep 
    // https://discord.com/channels/691052431525675048/1189344685546811564/1268573185276776501
    config.shared.tick = TickConfig{ tick_duration: fixed_time.timestep() };

    client_config.net = client::NetConfig::Local { id: std::process::id() as u64 };
    client_config.shared = config.shared.clone();
    //client_config.shared.mode = Mode::HostServer;

    commands.start_server();
}

pub fn start_client(
    mut commands: Commands,
    mut config: ResMut<ClientConfig>,
    fixed_time: Res<Time<Fixed>>,
){
    info!("The game is hosted by another client. Connecting to the host...");
    // update the client config to connect to the game host
    let client_addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), 0);
    let server_addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), PORT);
    let io_config = client::IoConfig::from_transport(client::ClientTransport::UdpSocket(client_addr));
        // .with_conditioner(LinkConditionerConfig { incoming_latency: Duration::from_millis(200), incoming_jitter: default(), incoming_loss: 0.0  });
    let auth = Authentication::Manual {
        // server's IP address
        server_addr,
        // ID to uniquely identify the client
        client_id: std::process::id() as u64, // XXX wont work for non-local multiplayer
        // private key shared between the client and server
        private_key: default(),
        // PROTOCOL_ID identifies the version of the protocol
        protocol_id: default()
    };
    let net_config = client::NetConfig::Netcode {
        auth,
        io: io_config,
        config: default()
    };
    config.net = net_config; 
    config.shared.tick = TickConfig{ tick_duration: fixed_time.timestep() };
    //config.shared.mode = Mode::HostServer;
    
    commands.connect_client();
}
#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq, Reflect)]
#[reflect(Component)]
#[component(storage = "SparseSet")]
pub struct ReplicateOtherClients(
    /// parent_sync
    pub bool
);
 
pub fn replicate_other_clients(
    identity: NetworkIdentity,
    mut commands: Commands,
    replicated_cursor: Query<
        (
            Entity, 
            Option<&AuthorityPeer>,
            Has<HasAuthority>,
            Has<Replicated>,
            &ReplicateOtherClients
        ),
        Added<ReplicateOtherClients>
    >,
) {
    for (entity, peer, has_auth, replicated, value) in replicated_cursor.iter() {

        if identity.is_server() || identity.is_host_server() {
            if let Some(AuthorityPeer::Client(client_id)) = peer {
                commands.entity(entity)
                .insert((
                    ControlledBy {
                        target: NetworkTarget::Single(*client_id),
                        lifetime: server::Lifetime::SessionBased,
                    },
                    ReplicationTarget{
                        target: NetworkTarget::AllExceptSingle(*client_id)
                    },
                    ReplicateHierarchy { // heirarchy replication causes panic when using hostserver
                        enabled: false,
                        recursive: false,
                    }
                )); 
            }
            if !replicated {
                let mut e = commands.entity(entity);
                e.insert((
                    ReplicationTarget::default(),
                    ReplicateHierarchy { // heirarchy replication causes panic when using hostserver
                        enabled: false,
                        recursive: false,
                    }
                ));
                if value.0 {
                    e.insert(ParentSync::default());
                }
            }
        } else if identity.is_client() && !replicated {
            let mut e = commands.entity(entity);
            e.insert((
                ReplicateToServer,
                ReplicateHierarchy { // heirarchy replication causes panic when using hostserver
                    enabled: false,
                    recursive: false,
                })
            );
            if value.0 {
                e.insert(ParentSync::default());
            }
        }

        // for all cursors we have received, add a Replicate component so that we can start replicating it
        // to other clients

    }
}

/// predicatable Id which is the same on client and server, and unique
/// is a hierarchical hash, so we can, for example, salt the root of a blueprint to disambiguate multiple instances
/// should be immutable
#[derive(Debug,Copy,Clone, Component, Reflect)]
#[reflect(Component)]
struct Uid(u64);
 
impl Uid {
    fn map_entities(){
        
    }

    fn generate(
        trigger: Trigger<OnAdd, UidGen>,
        parents: Query<&Parent>,
        uids: Query<&Uid>,
        seeds: Query<&UidSeed>,
        names: Query<&Name>,
        use_path: Query<&UidUsePath>,
        uid_gen: Query<&UidGen>,
        mut commands: Commands,
    ){
        let e = trigger.entity();

        // XXX currently Uid not allowed to change
        let mut root = parents.iter_ancestors(e).find(|e| uids.contains(*e));
        
        let tree : Vec<_> = parents.iter_ancestors(e).take_while(|e| Some(*e) != root).collect();

        let mut uid = 0;
        if let Some(e) = root{
            uid = uids.get(e).unwrap().0;
        }

        let mut path : Vec<Option<&str>> = Vec::new();
        let mut debug_str = String::new();

        // generate needed id's starting with furthest ancestor, since each id uses ancestors for id
        // TODO: generate in such a way that intermediate paths can be ignored
        // TODO: a way to create asset refs which are convertable to Uids 
        for entity in tree {

            let name = names.get(entity).map(|n|n.as_str()).ok();
            path.push(name);

            let do_gen = uid_gen.contains(entity);
            if do_gen {
                let mut hasher = DefaultHasher::new();
                hasher.write_u64(uid);
                debug_str += &format!("{}\n", uid);

                if let Ok(use_path) = use_path.get(entity) {
                    match use_path {
                        UidUsePath::Path => for p in path.iter() {
                            if let Some(p) = p {
                                hasher.write((*p).as_bytes());
                                debug_str += &format!("{:?}\n", p);
                            }
                        },
                        UidUsePath::Name => {
                            if let Some(n) = name {
                                hasher.write((*n).as_bytes());
                                debug_str += &format!("{:?}\n", n);
                            }
                        }
                    }
                }

                if let Ok(seed) = seeds.get(entity) {
                    hasher.write(seed.0.as_bytes());
                    debug_str += &format!("{:?}\n", seed);
                }

                uid = hasher.finish();
                commands.entity(entity).insert(Uid(uid));

                path.clear();
            }
        }
    }
}

#[derive(Debug, Clone, Component, Reflect)]
#[reflect(Component)]
#[require(UidGen)]
struct UidSeed(String);

#[derive(Debug,Copy,Clone, Component, Reflect)]
#[reflect(Component)]
#[require(UidGen)]
enum UidUsePath{
    Name,
    Path,   
}

#[derive(Debug,Copy,Clone, Component, Reflect, Default)]
#[reflect(Component)]
#[component(storage = "SparseSet")]
struct UidGen;

#[derive(Debug,Clone, Component, Reflect, Default)]
#[reflect(Component)]
struct UidDebug(String);

// note: https://github.com/cBournhonesque/lightyear/blob/2037d468f513569deee79ca24e0eb06c2a4c35ea/examples/distributed_authority/src/server.rs#L58C1-L79C2