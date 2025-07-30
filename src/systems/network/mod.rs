use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::ops::Shl;
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
use std::fs::{create_dir_all, write};
use std::path::Path;
use std::fs::read_to_string;

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
            .with_replication_config(ComponentReplicationConfig {
                replicate_once: true,
                ..Default::default()
            });
        app.add_systems(Update, ReplicateAuto::dumb);

        app.add_plugins((CornPhysicsPluginNetworkPlugin, CharacterNetworkPlugin));
        // app.register_component::<ReplicateOtherClients>();
        // app.add_systems(FixedUpdate, replicate_other_clients);

        app.register_type::<ReconnectTimer>();
        app.add_systems(Update, reconnect_system);
    }
}

#[derive(Debug, Resource, Reflect)]
#[reflect(Resource)]
struct NetworkCrap {
    address: SocketAddr,
    conditioner: Option<LinkConditionerConfig>,
}

#[cfg(target_family = "wasm")]
pub fn get_digest_on_wasm() -> Option<String> {
    let window = web_sys::window().expect("expected window");

    if let Ok(obj) = window.location().hash() {
        info!("Using cert digest from window.location().hash()");
        let cd = obj.replace("#", "");
        if cd.len() > 10 {
            // lazy sanity check.
            return Some(cd);
        }
    }

    if let Some(obj) = window.get("CERT_DIGEST") {
        info!("Using cert digest from window.CERT_DIGEST");
        return Some(obj.as_string().expect("CERT_DIGEST should be a string"));
    }

    None
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

fn start_client(
    server: Query<Entity, With<Server>>,
    mut commands: Commands,
    crap: Res<NetworkCrap>,
) {
    let conditioner = crap
        .conditioner
        .as_ref()
        .map(|c| RecvLinkConditioner::new(c.clone()));

    #[cfg(target_family = "wasm")]
    let id: u64 = rand::random::<u64>().shl(32);

    #[cfg(not(target_family = "wasm"))]
    let id: u64 = std::process::id() as u64 + rand::random::<u64>().shl(32);

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
        ),
    ));

    let certificate_digest = {
        #[cfg(target_family = "wasm")]
        {
            // include_str!("../../certificates/digest.txt").to_string()
            get_digest_on_wasm().unwrap().replace(":", "")
        }
        #[cfg(not(target_family = "wasm"))]
        {
            "".to_string()
        }
    };

    if server.is_empty() {
        info!("starting client");
        client.insert(
            NetcodeClient::new(
                auth,
                client::NetcodeConfig {
                    client_timeout_secs: 15,
                    ..default()
                },
            )
            .unwrap(),
        );
        client.insert(WebTransportClientIo { certificate_digest });
    } else {
        // hostserver
        info!("starting host-client");
        client.insert(LinkOf {
            server: server.single().unwrap(),
        });
    }

    client.trigger(Connect);
}

#[cfg(not(target_family = "wasm"))]
fn start_server(mut commands: Commands, crap: Res<NetworkCrap>) {
    let server_addr = SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), crap.address.port());

    // TODO env var
    let cert_dir = "assets/certs";
    let cert_pem_path = format!("{}/cert.pem", cert_dir);
    let key_pem_path = format!("{}/key.pem", cert_dir);
    create_dir_all(cert_dir).ok();

    let certificate = if Path::new(&cert_pem_path).exists() && Path::new(&key_pem_path).exists() {
        // TODO bevy async tasks
        // Load certificate asynchronously, but block until ready since Bevy systems are not async.
        // Use block_in_place to avoid blocking the async runtime if present.

        use tokio::runtime::Builder;
        let rt = Builder::new_current_thread()
            .enable_all() // Needed for timers, fs, etc.
            .build()
            .unwrap();

        let cert_result = rt.block_on(Identity::load_pemfiles(
            cert_pem_path.clone(),
            key_pem_path.clone(),
        ));
        match cert_result {
            Ok(cert) => {
                // Check expiry
                // wtf why does wtransport not expose this directly

                use x509_parser::prelude::{FromDer, X509Certificate};
                let der = cert.certificate_chain().as_slice()[0].der();
                let xcert = X509Certificate::from_der(der).expect("valid");
                if !xcert.1.validity().is_valid() {
                    info!("Certificate expired, regenerating...");
                    None
                } else {
                    Some(cert)
                }
            }
            Err(e) => {
                info!("Failed to parse certificate: {e}, regenerating...");
                None
            }
        }
    } else {
        None
    };

    let certificate = certificate.unwrap_or_else(|| {
        info!("generating new self signed cert");
        // Read certificate idents from a file, fallback to defaults if not found
        let idents_path = format!("{}/idents.txt", cert_dir);
        let idents: Vec<String> = if let Ok(contents) = read_to_string(&idents_path) {
            contents
                .lines()
                .map(|line| line.trim().to_string())
                .filter(|line| !line.is_empty())
                .collect()
        } else {
            error!("could not load address file {}", idents_path);
            vec![
                "127.0.0.1".to_string(),
                "::1".to_string(),
                "localhost".to_string(),
            ]
        };

        let cert = Identity::self_signed(idents).unwrap();

        let digest = cert.certificate_chain().as_slice()[0].hash();

        // Write cert.pem
        write(
            &cert_pem_path,
            cert.certificate_chain().as_slice()[0].to_pem(),
        )
        .ok();

        // Write key.pem
        write(&key_pem_path, cert.private_key().to_secret_pem()).ok();

        cert
    });

    let digest = certificate.certificate_chain().as_slice()[0].hash();

    // Write digest
    let digest_path = format!("{}/digest.txt", cert_dir);
    write(&digest_path, format!("{digest}")).ok();
    println!("🔐 Certificate digest: {digest}");

    let mut server = commands.spawn((
        Name::from("server"),
        Started, // webtransport code doesn't add this.
        LocalAddr(server_addr),
        WebTransportServerIo { certificate },
        NetcodeServer::new(server::NetcodeConfig::default().with_client_timeout_secs(4)),
    ));
    server.with_child((Name::from("on_connect"), Observer::new(handle_new_client)));
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
            } else if world
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
        mut commands: Commands,
    ) {
        if new_client.is_empty() && new_server.is_empty() {
            return;
        }
        for e in query.iter() {
            trace!("rerun ReplicateAuto logic");
            if !new_client.is_empty() {
                commands.entity(e).insert(Replicate::to_server());
            } else {
                commands
                    .entity(e)
                    .insert(Replicate::to_clients(NetworkTarget::All));
            }
        }
    }
}

#[derive(Debug, Default, Clone, Copy, Component)]
pub struct NetworkWindow;
impl bevy_editor_pls::editor_window::EditorWindow for NetworkWindow {
    fn name(
        &self,
        _world: &mut bevy::prelude::World,
        _cx: bevy_editor_pls::editor_window::EditorWindowContext<'_>,
    ) -> String {
        "Network".to_string()
    }

    fn ui(
        &self,
        world: &mut bevy::prelude::World,
        _cx: bevy_editor_pls::editor_window::EditorWindowContext,
        ui: &mut bevy_editor_pls::egui::Ui,
    ) {
        if let Ok(client) = world.query_filtered::<Entity, With<Client>>().single(world) {
            if ui.button("connect").clicked() {
                world.trigger_targets(Connect, client);
            }
            if ui.button("disconnect").clicked() {
                world.trigger_targets(Disconnect, client);
            }
        }
    }
}

impl Plugin for NetworkWindow {
    fn build(&self, app: &mut App) {
        use bevy_editor_pls::AddEditorWindow;
        // app.init_resource::<PreviouslyActiveCameras>();
        app.add_editor_window::<NetworkWindow>();
    }
}

#[derive(Component, Default, Reflect)]
#[reflect(Component)]
struct ReconnectTimer(Timer);

fn reconnect_system(
    mut disconnect_query: Query<Entity, (With<Client>, Added<Disconnected>)>,
    mut timer_query: Query<(Entity, &mut ReconnectTimer), (With<Client>, With<Disconnected>)>,
    time: Res<Time>,
    mut commands: Commands,
) {
    // Start timer on disconnect for clients that just got disconnected
    for entity in disconnect_query.iter_mut() {
        let timer = Timer::from_seconds(3.0, TimerMode::Once);
        commands.entity(entity).insert(ReconnectTimer(timer));
    }

    // Tick timers and reconnect if finished
    for (entity, mut timer) in timer_query.iter_mut() {
        timer.0.tick(time.delta());
        if timer.0.finished() {
            commands.entity(entity).trigger(Connect);
        }
    }
}
