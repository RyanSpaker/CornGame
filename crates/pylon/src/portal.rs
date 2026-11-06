//! CODEX (GPT-6): resolve current world poses before propagation, including portal crossing and clipping.
//! CLAUDE (claude-opus-5-5): realms and portals: places that are far apart, or overlap, joined by doorways
//! (the pylon is bigger inside).
//!
//! - A [`Realm`] is an entity, and what's below it is in it: each gets [`InRealm`] and render layer `id`
//!   (unless it has layers of its own). A camera in a realm sees only that realm, and lights light only
//!   their own realm's things (bevy matches lights to views by layers): the sun in the field doesn't
//!   reach the pylon's interior. Realm 0 is bevy's default layer, so what's in no realm is in realm 0. A
//!   realm also says how it looks to cameras in it: fog color, and with [`RealmAmbient`] beside it, the
//!   ambient light's brightness.
//! - A [`Portal`] in a realm leads to realm `to`. Its surface (a `size` rectangle, local XY facing +Z)
//!   shows the other realm, from where the player would see it: a camera of its own renders that into an
//!   image the surface samples in screen space. The camera's near plane is the other realm's portal back
//!   (oblique clipping), so what stands behind that doorway isn't seen. Walking through the front moves
//!   the player by `offset`. While not `open` (scripts close it as its doors shut) it neither renders
//!   nor can be crossed.
//! - The [`Player`] ([`InRealm`] too) sees its realm, and its realm's portals.
//! - A [`PortalFrame`] (a doorway's doors) is seen from its realm, never through a
//!   portal: it's hidden while the player is in another realm (the only time a camera sees its realm
//!   through a portal or a window). Unlike a layer of its own, it still casts its realm's shadows.
//! - A [`WindowView`] camera renders a realm from a fixed point, for windows ([`ViewTextures`]).
//!
//! Cameras change realm (layers, look, clip plane) in the frame the player crosses.

use bevy::{
    asset::uuid_handle,
    camera::{
        visibility::{RenderLayers, VisibilitySystems},
        CameraUpdateSystems, Exposure, Hdr, RenderTarget,
    },
    light::cluster::{ClusterConfig, ClusterFarZMode, ClusterZConfig},
    core_pipeline::{
        prepass::DepthPrepass,
        tonemapping::{DebandDither, Tonemapping},
    },
    prelude::*,
    render::render_resource::{Extent3d, TextureFormat},
    window::{PrimaryWindow, WindowResized},
};

use crate::{material::ShaderMaterial, player::Player, precompile::WarmUpView};

pub fn plugin(app: &mut App) {
    app.register_type::<Realm>()
        .register_type::<RealmAmbient>()
        .register_type::<InRealm>()
        .register_type::<Portal>()
        .register_type::<WindowView>()
        .register_type::<PortalFrame>()
        .register_type::<ViewTextures>()
        .add_systems(Startup, make_textures)
        .add_systems(Update, (resize_textures, target_texture))
        // after the player walks (Update), before transforms propagate: what a camera sees, and its
        // shadow cascades, are worked out after, for active cameras. and before bevy makes cameras'
        // projection matrices, or a portal's clip plane is a frame late
        .add_systems(
            PostUpdate,
            (join_realms, make_portals, cross, show_frames, follow, wake_views, shut_portals)
                .chain()
                // CODEX (GPT-6): helpers must see script writes before resolving cameras and portal planes.
                .after(corn_scripting::ScriptSystems)
                .before(TransformSystems::Propagate)
                .before(VisibilitySystems::VisibilityPropagate)
                .before(CameraUpdateSystems),
        );
}

/// portal surfaces are on layer `PORTAL_LAYERS + realm`: seen by the player's camera in that realm, never
/// by a camera looking through a portal (it would see its own image)
pub const PORTAL_LAYERS: usize = 16;

/// a place. What's below this entity is in it
#[derive(Component, Reflect, Clone, Debug, Default)]
#[reflect(Component, Default)]
pub struct Realm {
    pub id: u32,
    /// their fog's color (alpha 0: none)
    pub fog: Color,
}

// CLAUDE
/// the ambient light's brightness for cameras that see a realm, on its [`Realm`] entity (bevy's
/// `AmbientLight` is a camera's own). Without one, cameras keep their own
#[derive(Component, Reflect, Clone, Copy, Debug, Default, PartialEq)]
#[reflect(Component, Default)]
pub struct RealmAmbient(pub f32);

/// the realm something is in: the player's (it moves), everything below a [`Realm`] (given)
#[derive(Component, Reflect, Clone, Copy, Debug, Default, PartialEq)]
#[reflect(Component, Default)]
pub struct InRealm(pub u32);

/// a doorway to realm `to`, in the realm it's in
#[derive(Component, Reflect, Clone, Debug)]
#[reflect(Component, Default)]
pub struct Portal {
    pub to: u32,
    /// crossing moves the player by this world-space displacement
    pub offset: Vec3,
    pub size: Vec2,
    /// renders and can be crossed
    pub open: bool,
    /// the surface's color while shut (what its closed doors look like): black by default. read once, as
    /// the portal is made
    pub shut: Color,
}

impl Default for Portal {
    fn default() -> Self {
        Self { to: 0, offset: Vec3::ZERO, size: Vec2::ONE, open: false, shut: Color::BLACK }
    }
}

/// a camera that renders realm `shows` from `at` into [`ViewTextures`]`.window`, for windows in realm
/// `windows_in` (one observer for all of them). It turns and zooms with the player's camera, and renders
/// while the windows can be seen: the player is in their realm, or an open portal leads there
#[derive(Component, Reflect, Clone, Debug, Default)]
#[reflect(Component, Default)]
// it sleeps until a window is seen: its pipelines are built beforehand
#[require(Camera3d, WarmUpView)]
pub struct WindowView {
    pub at: Vec3,
    pub shows: u32,
    pub windows_in: u32,
}

/// the images window views render into, screen sized. Scripts give them to materials:
/// `world.get_resource("ViewTextures").window`
#[derive(Resource, Reflect, Clone, Debug)]
#[reflect(Resource)]
pub struct ViewTextures {
    pub window: Handle<Image>,
}

/// the image a portal's camera renders into, and the surface's materials: showing it, or black while
/// shut (the image is stale then: it was rendered from wherever the player last stood)
#[derive(Component)]
struct PortalSurface {
    image: Handle<Image>,
    open: Handle<ShaderMaterial>,
    shut: Handle<ShaderMaterial>,
}

/// seen only from its own realm, never through a portal or a window: hidden while the player is elsewhere.
/// a door, so the other realm's isn't seen through the doorway as a second one behind it
#[derive(Component, Reflect, Clone, Copy, Debug, Default)]
#[reflect(Component, Default)]
pub struct PortalFrame;

/// the camera of this portal
#[derive(Component)]
struct PortalView {
    portal: Entity,
}

/// the player's world position after last frame's crossing
#[derive(Component)]
struct LastEye(Vec3);

/// how far a portal's surface reaches behind its plane: a box, open at the front, so as the eye passes
/// through the plane (and the camera's near plane cuts the front) what's behind it still shows the other
/// realm. more than the near plane's corners reach
const DEPTH: f32 = 0.15;

// CLAUDE
/// a portal's surface: its `size` rectangle, facing +Z, and a box behind it facing in
fn surface_mesh(size: Vec2) -> Mesh {
    let mut surface = Mesh::from(Rectangle::new(size.x, size.y));
    let mut inside = Mesh::from(Cuboid::new(size.x, size.y, DEPTH)).translated_by(Vec3::Z * -DEPTH / 2.0);
    inside.invert_winding().expect("a cuboid's indices are triangles");
    surface.merge(&inside).expect("both have positions, normals and uvs");
    surface
}

// CLAUDE
/// light clusters to a fixed depth: by default it's the last frame's farthest light, and a camera that
/// starts seeing a realm's lights (woken, or crossing into it) drops them for frames
fn fixed_clusters(far: f32) -> ClusterConfig {
    ClusterConfig::FixedZ {
        total: 4096,
        z_slices: 24,
        z_config: ClusterZConfig { far_z_mode: ClusterFarZMode::Constant(far), ..default() },
        dynamic_resizing: true,
    }
}

fn screen_image(size: UVec2) -> Image {
    Image::new_target_texture(size.x.max(1), size.y.max(1), TextureFormat::Rgba16Float, None)
}

fn screen_size(windows: &Query<&Window, With<PrimaryWindow>>) -> UVec2 {
    windows.iter().next().map_or(UVec2::new(1280, 720), Window::physical_size)
}

// a fixed id, so scripts can hand it to materials (`asset:<uuid>`)
const WINDOW_IMAGE: Handle<Image> = uuid_handle!("6f3c0a52-7d1e-4b8a-9b2c-51e0d8a4c102");

fn make_textures(mut commands: Commands, windows: Query<&Window, With<PrimaryWindow>>, mut images: ResMut<Assets<Image>>) {
    let _ = images.insert(WINDOW_IMAGE.id(), screen_image(screen_size(&windows)));
    commands.insert_resource(ViewTextures { window: WINDOW_IMAGE });
}

fn resize_textures(
    mut resized: MessageReader<WindowResized>,
    windows: Query<&Window, With<PrimaryWindow>>,
    textures: Option<Res<ViewTextures>>,
    surfaces: Query<&PortalSurface>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<ShaderMaterial>>,
) {
    if resized.read().last().is_none() {
        return;
    }
    let size = screen_size(&windows).max(UVec2::ONE);
    let mut changed = Vec::new();
    for handle in textures.iter().map(|t| &t.window).chain(surfaces.iter().map(|s| &s.image)) {
        if let Some(mut image) = images.get_mut(handle) {
            if image.size() != size {
                image.resize(Extent3d { width: size.x, height: size.y, depth_or_array_layers: 1 });
                changed.push(handle.id());
            }
        }
    }
    // CLAUDE: a material's bind group is only remade when the material changes, not its image: touch the
    // ones that sample a resized image, or they keep sampling the old texture
    let stale: Vec<_> = materials.iter().filter(|(_, m)| m.texture.as_ref().is_some_and(|t| changed.contains(&t.id()))).map(|(id, _)| id).collect();
    for id in stale {
        let _ = materials.get_mut(id);
    }
}

/// window views render into their image. checked every frame: a scene spawning the camera may insert
/// its own (default, window) target after the view appears
fn target_texture(mut commands: Commands, views: Query<(Entity, Option<&RenderTarget>, Has<Hdr>), With<WindowView>>, textures: Option<Res<ViewTextures>>) {
    let Some(textures) = textures else { return };
    for (entity, target, hdr) in &views {
        if !matches!(target, Some(RenderTarget::Image(t)) if t.handle == textures.window) {
            commands.entity(entity).insert(RenderTarget::Image(textures.window.clone().into()));
        }
        // HDR in, so the player's camera tonemaps what it sees through windows once
        if !hdr {
            commands.entity(entity).insert((Hdr, Tonemapping::None, DebandDither::Disabled));
        }
    }
}

// CLAUDE
/// what's new below a [`Realm`] joins it: [`InRealm`], and its layer unless it has its own (portals get
/// theirs as their surface is made)
fn join_realms(
    mut commands: Commands,
    new: Query<(Entity, Has<RenderLayers>, Has<Portal>), (Without<InRealm>, Or<(Added<ChildOf>, Added<Realm>)>)>,
    parents: Query<&ChildOf>,
    realms: Query<&Realm>,
) {
    for (entity, layered, portal) in &new {
        // the nearest realm: this, or an ancestor
        let Some(realm) = std::iter::once(entity).chain(parents.iter_ancestors(entity)).find_map(|a| realms.get(a).ok()) else { continue };
        let mut entity = commands.entity(entity);
        entity.insert(InRealm(realm.id));
        if !layered && !portal {
            entity.insert(RenderLayers::layer(realm.id as usize));
        }
    }
}

// CLAUDE
/// a new portal gets its surface and its camera, which takes the player camera's settings (the player
/// must be there)
fn make_portals(
    mut commands: Commands,
    new: Query<(Entity, &Portal, &InRealm), Without<PortalSurface>>,
    players: Query<(Entity, &Camera, Option<&Msaa>, Has<DepthPrepass>, Option<&DistanceFog>, Option<&AmbientLight>, Option<&Exposure>, Option<&Projection>), With<Player>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    assets: Res<AssetServer>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ShaderMaterial>>,
) {
    let Some((player, camera, msaa, prepass, fog, ambient, exposure, projection)) = players.iter().next() else { return };
    let far = match projection {
        Some(Projection::Perspective(p)) => p.far,
        _ => 1000.0,
    };
    if !new.is_empty() {
        // XXX fix reclustering: crossing, the player's camera sees the other realm's lights first, and its
        // clusters (fit to last frame's farthest light) drop them for frames. Fixed depth clusters hide it;
        // what we actually need (part of portals) is to swap the portal camera's clusters in for the main
        // camera's as it crosses. or drop the frame (could solve a number of flicker issues, if possible)
        commands.entity(player).insert(fixed_clusters(far));
    }
    for (portal_entity, portal, realm) in &new {
        let image = images.add(screen_image(screen_size(&windows)));
        let mut view = commands.spawn((
            Name::new("portal view"),
            PortalView { portal: portal_entity },
            Camera3d::default(),
            Camera { order: -1, clear_color: camera.clear_color.clone(), is_active: false, ..default() },
            // it sleeps until its portal opens: its pipelines are built beforehand
            WarmUpView,
            RenderTarget::Image(image.clone().into()),
            // HDR in, so the player's camera tonemaps what it sees through portals once
            (Hdr, Tonemapping::None, DebandDither::Disabled),
            RenderLayers::layer(portal.to as usize),
            // woken where it last stood (inside, the outer portal's camera isn't moved)
            fixed_clusters(far),
            msaa.copied().unwrap_or_default(),
        ));
        if prepass {
            view.insert(DepthPrepass);
        }
        if let Some(fog) = fog {
            view.insert(fog.clone());
        }
        if let Some(ambient) = ambient {
            view.insert(ambient.clone());
        }
        if let Some(exposure) = exposure {
            view.insert(*exposure);
        }
        // portal.wesl: open.x 1 shows the image, 0 the shut color
        let shut_color = Vec4::from_array(portal.shut.to_linear().to_f32_array());
        let mut material = |show: f32| {
            let params = [("open".to_string(), Vec4::X * show), ("shut".to_string(), shut_color)].into_iter().collect();
            materials.add(ShaderMaterial { fragment: assets.load("shaders/portal.wesl"), texture: Some(image.clone()), params, ..default() })
        };
        let (open, shut) = (material(1.0), material(0.0));
        commands.entity(portal_entity).insert((
            MeshMaterial3d(if portal.open { open.clone() } else { shut.clone() }),
            PortalSurface { image, open, shut },
            Mesh3d(meshes.add(surface_mesh(portal.size))),
            RenderLayers::layer(PORTAL_LAYERS + realm.0 as usize),
        ));
    }
}

// CODEX: read fresh hierarchy transforms before applying a world-space crossing displacement locally.
fn cross(
    mut commands: Commands,
    mut players: Query<(Entity, &mut InRealm, Option<&LastEye>, Option<&ChildOf>), With<Player>>,
    portals: Query<(Entity, &Portal, &InRealm), Without<Player>>,
    mut spatial: ParamSet<(TransformHelper, Query<&mut Transform>)>,
) {
    for (entity, mut realm, last, parent) in &mut players {
        // CODEX: both endpoints are world positions; cached globals can predate movement or parenting.
        let Ok(eye) = spatial.p0().compute_global_transform(entity) else { continue };
        let mut now = eye.translation();
        let before = last.map_or(now, |l| l.0);
        for (portal_entity, portal, portal_realm) in &portals {
            if *portal_realm != *realm || !portal.open {
                continue;
            }
            let Ok(global) = spatial.p0().compute_global_transform(portal_entity) else { continue };
            let to_local = global.affine().inverse();
            let (a, b) = (to_local.transform_point3(before), to_local.transform_point3(now));
            let half = portal.size * 0.5;
            // at the plane, to where it leads: what's seen through it doesn't move
            if a.z >= 0.0 && b.z < 0.0 && b.x.abs() < half.x && b.y.abs() < half.y {
                // CODEX: offsets are world-space displacements; preserve a parented player's local pose.
                let local_offset = if let Some(parent) = parent {
                    let Ok(parent) = spatial.p0().compute_global_transform(parent.parent()) else { continue };
                    parent.affine().inverse().transform_vector3(portal.offset)
                } else {
                    portal.offset
                };
                let mut transforms = spatial.p1();
                let Ok(mut transform) = transforms.get_mut(entity) else { continue };
                transform.translation += local_offset;
                now += portal.offset;
                realm.0 = portal.to;
                break;
            }
        }
        commands.entity(entity).insert(LastEye(now));
    }
}

// CLAUDE
/// cameras see their realm and take its look; portal and window views follow the player's camera. A
/// portal's view is clipped at the far portal: what stands between the view and it isn't drawn
// CODEX: ParamSet separates on-demand world reads from camera Transform writes, avoiding ECS aliasing.
fn follow(
    mut commands: Commands,
    players: Query<(Entity, &InRealm, Option<&RenderLayers>, Option<&Projection>), With<Player>>,
    realms: Query<(&Realm, Option<&RealmAmbient>)>,
    portals: Query<(Entity, &Portal, &InRealm)>,
    mut portal_views: Query<(Entity, &PortalView, &Camera, Option<&RenderLayers>, Option<&mut Projection>, Option<&ChildOf>), Without<Player>>,
    mut window_views: Query<(Entity, &WindowView, Option<&RenderLayers>, Option<&mut Projection>, Option<&ChildOf>), (Without<Player>, Without<PortalView>)>,
    mut looks: Query<(&mut AmbientLight, &mut DistanceFog)>,
    mut spatial: ParamSet<(TransformHelper, Query<&mut Transform>)>,
) {
    let Some((player, realm, layers, projection)) = players.iter().next() else { return };
    // CODEX: cross has already run; include its teleport and every current ancestor transform.
    let Ok(eye) = spatial.p0().compute_global_transform(player) else { return };
    let eye = eye.compute_transform();
    let realm_look = |id: u32| realms.iter().find(|(r, _)| r.id == id);
    let seen = RenderLayers::from_layers(&[realm.0 as usize, PORTAL_LAYERS + realm.0 as usize]);
    if layers != Some(&seen) {
        commands.entity(player).insert(seen);
    }
    look(&mut looks, player, realm_look(realm.0));
    let Some(Projection::Perspective(eye_projection)) = projection else { return };

    for (view, of, camera, layers, view_projection, parent) in &mut portal_views {
        let Ok((_, portal, portal_realm)) = portals.get(of.portal) else { continue };
        // only portals in the player's realm render (wake_views), and only theirs need following
        if !camera.is_active && *portal_realm != *realm {
            continue;
        }
        set_layers(&mut commands, view, layers, portal.to);
        look(&mut looks, view, realm_look(portal.to));
        // the other realm's portal back: a point on its plane, and its front (the side to show)
        let far = portals
            .iter()
            .find(|(_, p, r)| r.0 == portal.to && p.to == portal_realm.0)
            .and_then(|(entity, _, _)| spatial.p0().compute_global_transform(entity).ok())
            // CODEX: inverse-transpose keeps the plane normal correct under nonuniform parent scale.
            .map(|at| (at.translation(), at.affine().matrix3.inverse().transpose().mul_vec3(Vec3::Z).normalize()));
        let followed = Transform { translation: eye.translation + portal.offset, rotation: eye.rotation, scale: Vec3::ONE };
        let parent = if let Some(parent) = parent {
            let Ok(at) = spatial.p0().compute_global_transform(parent.parent()) else { continue };
            Some(at)
        } else { None };
        let mut transforms = spatial.p1();
        let Ok(mut transform) = transforms.get_mut(view) else { continue };
        place(&mut transform, view_projection, followed, eye_projection, far, parent);
    }

    for (view, window, layers, view_projection, parent) in &mut window_views {
        set_layers(&mut commands, view, layers, window.shows);
        look(&mut looks, view, realm_look(window.shows));
        let followed = Transform { translation: window.at, rotation: eye.rotation, scale: Vec3::ONE };
        let parent = if let Some(parent) = parent {
            let Ok(at) = spatial.p0().compute_global_transform(parent.parent()) else { continue };
            Some(at)
        } else { None };
        let mut transforms = spatial.p1();
        let Ok(mut transform) = transforms.get_mut(view) else { continue };
        place(&mut transform, view_projection, followed, eye_projection, None, parent);
    }
}

fn set_layers(commands: &mut Commands, view: Entity, layers: Option<&RenderLayers>, realm: u32) {
    let shows = RenderLayers::layer(realm as usize);
    if layers != Some(&shows) {
        commands.entity(view).insert(shows);
    }
}

/// put a view where it follows to, with the player's projection, clipped at `clip` (a point and the
/// front of a plane) if given
fn place(transform: &mut Mut<Transform>, view_projection: Option<Mut<Projection>>, followed: Transform, eye: &PerspectiveProjection, clip: Option<(Vec3, Vec3)>, parent: Option<GlobalTransform>) {
    // CODEX: clipping uses the desired world pose, while a parented view needs a local Transform.
    let local = parent.map_or(followed, |parent| GlobalTransform::from(followed).reparented_to(&parent));
    if **transform != local {
        **transform = local;
    }
    let Some(mut view_projection) = view_projection else { return };
    let mut want = eye.clone();
    want.near_clip_plane = clip.and_then(|(point, front)| clip_plane(&followed, point, front)).unwrap_or(vec4(0.0, 0.0, -1.0, -want.near));
    if !matches!(&*view_projection, Projection::Perspective(p) if p.fov == want.fov && p.near == want.near && p.far == want.far && p.near_clip_plane == want.near_clip_plane) {
        *view_projection = Projection::Perspective(want);
    }
}

/// give `camera` the realm's look, if both have one
fn look(looks: &mut Query<(&mut AmbientLight, &mut DistanceFog)>, camera: Entity, realm: Option<(&Realm, Option<&RealmAmbient>)>) {
    let (Some((realm, realm_ambient)), Ok((mut ambient, mut fog))) = (realm, looks.get_mut(camera)) else { return };
    if let Some(&RealmAmbient(brightness)) = realm_ambient {
        if ambient.brightness != brightness {
            ambient.brightness = brightness;
        }
    }
    if fog.color != realm.fog {
        fog.color = realm.fog;
    }
}

// CLAUDE
/// the plane through `point` facing `front`, in the view space of a camera at `camera`, as bevy's
/// `near_clip_plane` takes it (only what's in front is drawn). None when the camera isn't behind it
fn clip_plane(camera: &Transform, point: Vec3, front: Vec3) -> Option<Vec4> {
    // a little toward the camera, so the plane doesn't cut what stands on it
    const BIAS: f32 = 0.005;
    let to_view = camera.compute_affine().inverse();
    let normal = to_view.transform_vector3(front).normalize();
    let distance = normal.dot(to_view.transform_point3(point)) - BIAS;
    (distance > 0.0).then(|| normal.extend(-distance))
}

// CLAUDE
/// views render only while what they show can be seen: open portals in the player's realm, and windows
/// in the player's realm or through an open portal. A portal's camera goes with it
fn wake_views(
    mut commands: Commands,
    players: Query<&InRealm, With<Player>>,
    portals: Query<(&Portal, &InRealm)>,
    mut portal_views: Query<(Entity, &PortalView, &mut Camera)>,
    mut window_views: Query<(&WindowView, &mut Camera), Without<PortalView>>,
) {
    let Some(&realm) = players.iter().next() else { return };
    let wake = |camera: &mut Mut<Camera>, active: bool| {
        if camera.is_active != active {
            camera.is_active = active;
        }
    };
    for (view, of, mut camera) in &mut portal_views {
        let Ok((portal, at)) = portals.get(of.portal) else {
            commands.entity(view).despawn();
            continue;
        };
        wake(&mut camera, portal.open && *at == realm);
    }
    for (window, mut camera) in &mut window_views {
        let through = portals.iter().any(|(portal, at)| portal.open && *at == realm && portal.to == window.windows_in);
        wake(&mut camera, realm.0 == window.windows_in || through);
    }
}

// CLAUDE
/// frames show only while the player is in their realm
fn show_frames(players: Query<&InRealm, With<Player>>, mut frames: Query<(&InRealm, &mut Visibility), With<PortalFrame>>) {
    let Some(&realm) = players.iter().next() else { return };
    for (at, mut visibility) in &mut frames {
        let want = if *at == realm { Visibility::Inherited } else { Visibility::Hidden };
        if *visibility != want {
            *visibility = want;
        }
    }
}

// CLAUDE
/// a shut portal is black: its camera has stopped, and its last image only lined up with the view it was
/// rendered for. Swapped in the frame the camera wakes, which renders that frame (before the player's)
fn shut_portals(mut surfaces: Query<(&Portal, &PortalSurface, &mut MeshMaterial3d<ShaderMaterial>)>) {
    for (portal, surface, mut material) in &mut surfaces {
        let want = if portal.open { &surface.open } else { &surface.shut };
        if material.0 != *want {
            material.0 = want.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // CODEX: a moving portal ancestor and rotated/scaled player parent must be resolved before crossing.
    #[test]
    fn crossing_uses_fresh_portal_and_player_ancestors() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin));
        app.add_systems(PostUpdate, cross.before(TransformSystems::Propagate));
        let offset = Vec3::new(120.0, 0.0, 13.85);
        let portal_parent = app.world_mut().spawn(Transform::IDENTITY).id();
        app.world_mut().spawn((
            Portal { offset, size: Vec2::new(0.8, 2.2), to: 1, open: true, ..default() },
            InRealm(0), Transform::from_xyz(0.0, 1.1, 0.6), ChildOf(portal_parent),
        ));
        let parent_pose = Transform::from_xyz(10.0, 0.0, 0.0)
            .with_rotation(Quat::from_rotation_y(0.7)).with_scale(Vec3::splat(2.0));
        let player_parent = app.world_mut().spawn(parent_pose).id();
        let local = |at| GlobalTransform::from(Transform::from_translation(at))
            .reparented_to(&GlobalTransform::from(parent_pose));
        let player = app.world_mut().spawn((
            Player::default(), InRealm(0), local(Vec3::new(10.0, 1.68, 1.0)), ChildOf(player_parent),
        )).id();
        app.update();
        // CODEX: neither newly written local transform has propagated when cross runs next.
        app.world_mut().get_mut::<Transform>(portal_parent).unwrap().translation.x = 10.0;
        *app.world_mut().get_mut::<Transform>(player).unwrap() = local(Vec3::new(10.0, 1.68, 0.55));
        app.update();
        let expected = Vec3::new(10.0, 1.68, 0.55) + offset;
        assert_eq!(app.world().get::<InRealm>(player), Some(&InRealm(1)));
        assert!(app.world().get::<GlobalTransform>(player).unwrap().translation().abs_diff_eq(expected, 1e-4));
        assert!(app.world().get::<LastEye>(player).unwrap().0.abs_diff_eq(expected, 1e-4));
    }

    // CODEX: script-phase ancestor movement must affect the camera pose and clip plane in the same frame.
    #[test]
    fn following_uses_fresh_world_pose_and_clip_plane() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin));
        app.add_systems(PostUpdate, follow.after(corn_scripting::ScriptSystems).before(TransformSystems::Propagate));
        let player_parent = app.world_mut().spawn(Transform::IDENTITY).id();
        let player = app.world_mut().spawn((
            Player::default(), InRealm(0), Transform::IDENTITY, ChildOf(player_parent),
            Projection::Perspective(PerspectiveProjection::default()),
        )).id();
        let offset = Vec3::X * 120.0;
        let portal = app.world_mut().spawn((Portal { to: 1, offset, open: true, ..default() }, InRealm(0), Transform::IDENTITY)).id();
        let far_parent = app.world_mut().spawn(Transform::from_xyz(120.0, 0.0, -2.0)).id();
        app.world_mut().spawn((
            Portal { to: 0, ..default() }, InRealm(1),
            Transform::from_rotation(Quat::from_rotation_y(std::f32::consts::PI)), ChildOf(far_parent),
        ));
        let view_parent = app.world_mut().spawn(
            Transform::from_xyz(-3.0, 0.0, 2.0).with_rotation(Quat::from_rotation_y(0.6)).with_scale(Vec3::splat(2.0)),
        ).id();
        let view = app.world_mut().spawn((
            PortalView { portal }, Camera::default(), Transform::IDENTITY, ChildOf(view_parent),
            Projection::Perspective(PerspectiveProjection::default()),
        )).id();
        let window = app.world_mut().spawn((
            WindowView { at: Vec3::new(0.0, 3.59, 0.0), ..default() },
            Transform::IDENTITY, ChildOf(view_parent),
        )).id();
        app.update();
        // CODEX: exercise the public ordering label with a script-like writer after Update.
        app.add_systems(PostUpdate, (move |mut transforms: Query<&mut Transform>| {
            let mut parent = transforms.get_mut(player_parent).unwrap();
            parent.translation.x = 1.0;
            parent.rotation = Quat::from_rotation_y(0.2);
            transforms.get_mut(far_parent).unwrap().translation.z = -4.0;
        }).in_set(corn_scripting::ScriptSystems));
        app.update();
        let eye = app.world().get::<GlobalTransform>(player).unwrap().compute_transform();
        let expected = Transform { translation: eye.translation + offset, rotation: eye.rotation, scale: Vec3::ONE };
        let actual = app.world().get::<GlobalTransform>(view).unwrap().compute_transform();
        assert!(actual.translation.abs_diff_eq(expected.translation, 1e-4));
        assert!(actual.rotation.abs_diff_eq(expected.rotation, 1e-5));
        let expected_plane = clip_plane(&expected, Vec3::new(120.0, 0.0, -4.0), Vec3::NEG_Z).unwrap();
        let Projection::Perspective(projection) = app.world().get::<Projection>(view).unwrap() else { panic!("perspective portal view") };
        assert!(projection.near_clip_plane.abs_diff_eq(expected_plane, 1e-4));
        let window = app.world().get::<GlobalTransform>(window).unwrap().compute_transform();
        assert!(window.translation.abs_diff_eq(Vec3::new(0.0, 3.59, 0.0), 1e-4));
        assert!(window.rotation.abs_diff_eq(eye.rotation, 1e-5));
    }

    // CODEX: follow must resolve the post-crossing pose, not the pre-crossing cached global.
    #[test]
    fn following_includes_a_same_frame_crossing() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin));
        app.add_systems(PostUpdate, (cross, follow).chain().before(TransformSystems::Propagate));
        let offset = Vec3::X * 120.0;
        app.world_mut().spawn((Portal { to: 1, offset, open: true, size: Vec2::splat(3.0), ..default() }, InRealm(0), Transform::IDENTITY));
        let back = app.world_mut().spawn((Portal { to: 0, offset: -offset, open: true, ..default() }, InRealm(1), Transform::from_xyz(120.0, 0.0, 0.0))).id();
        let player = app.world_mut().spawn((
            Player::default(), InRealm(0), Transform::from_xyz(0.0, 1.0, 0.1),
            Projection::Perspective(PerspectiveProjection::default()),
        )).id();
        let view = app.world_mut().spawn((PortalView { portal: back }, Camera::default(), Transform::IDENTITY)).id();
        app.update();
        app.world_mut().get_mut::<Transform>(player).unwrap().translation.z = -0.1;
        app.update();
        assert_eq!(app.world().get::<InRealm>(player), Some(&InRealm(1)));
        assert!(app.world().get::<GlobalTransform>(view).unwrap().translation().abs_diff_eq(Vec3::new(0.0, 1.0, -0.1), 1e-5));
    }

    #[test]
    fn crossing_moves_between_realms() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, TransformPlugin));
        app.add_systems(PostUpdate, cross.before(TransformSystems::Propagate));
        // outside: a doorway at z = 0.6 facing +z; inside, 120 m away, one facing back into its room
        let offset = Vec3::new(120.0, 0.0, 13.85);
        let outer = app.world_mut().spawn((Portal { offset, size: Vec2::new(0.8, 2.2), to: 1, open: true, ..default() }, InRealm(0), Transform::from_xyz(0.0, 1.1, 0.6))).id();
        let inside = Transform::from_xyz(120.0, 1.1, 14.45).with_rotation(Quat::from_rotation_y(std::f32::consts::PI));
        app.world_mut().spawn((Portal { offset: -offset, size: Vec2::new(0.8, 2.2), to: 0, open: true, ..default() }, InRealm(1), inside));
        let player = app.world_mut().spawn((Player::default(), InRealm(0), Transform::from_xyz(0.0, 1.68, 1.0))).id();
        let at = |app: &App| app.world().get::<Transform>(player).unwrap().translation;
        let realm = |app: &App| app.world().get::<InRealm>(player).unwrap().0;
        let walk = |app: &mut App, to: Vec3| {
            app.world_mut().get_mut::<Transform>(player).unwrap().translation = to;
            app.update();
        };
        app.update();

        // beside the doorway: nothing happens
        walk(&mut app, Vec3::new(1.0, 1.68, 0.5));
        walk(&mut app, Vec3::new(0.0, 1.68, 1.0));
        assert_eq!(realm(&app), 0);

        // closed, it's only a plane
        app.world_mut().get_mut::<Portal>(outer).unwrap().open = false;
        walk(&mut app, Vec3::new(0.1, 1.68, 0.55));
        assert_eq!(realm(&app), 0);
        walk(&mut app, Vec3::new(0.0, 1.68, 1.0));
        app.world_mut().get_mut::<Portal>(outer).unwrap().open = true;

        // just in front of it, still outside
        walk(&mut app, Vec3::new(0.1, 1.68, 0.61));
        assert_eq!(realm(&app), 0);
        // through it: inside, where that is past the inner doorway, in the room
        walk(&mut app, Vec3::new(0.1, 1.68, 0.55));
        assert_eq!(realm(&app), 1);
        assert!(at(&app).abs_diff_eq(Vec3::new(120.1, 1.68, 14.4), 1e-4), "{}", at(&app));

        // and back out, as far past the outer doorway
        walk(&mut app, Vec3::new(120.1, 1.68, 14.5));
        assert_eq!(realm(&app), 0);
        assert!(at(&app).abs_diff_eq(Vec3::new(0.1, 1.68, 0.65), 1e-4), "{}", at(&app));
    }

    // CLAUDE
    /// what's below a realm joins it (its layer, unless it has its own); what isn't, doesn't
    #[test]
    fn realms_take_in_what_is_below_them() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins).add_systems(PostUpdate, join_realms);
        let world = app.world_mut();
        let room = world.spawn(Realm { id: 1, ..default() }).id();
        let lamp = world.spawn(ChildOf(room)).id();
        let bulb = world.spawn(ChildOf(lamp)).id();
        let own = world.spawn((ChildOf(room), RenderLayers::layer(5))).id();
        let outside = world.spawn_empty().id();
        app.update();
        let world = app.world();
        for e in [room, lamp, bulb] {
            assert_eq!(world.get::<InRealm>(e), Some(&InRealm(1)));
            assert_eq!(world.get::<RenderLayers>(e), Some(&RenderLayers::layer(1)));
        }
        assert_eq!(world.get::<RenderLayers>(own), Some(&RenderLayers::layer(5)));
        assert_eq!(world.get::<InRealm>(own), Some(&InRealm(1)));
        assert!(world.get::<InRealm>(outside).is_none());
    }

    // CLAUDE
    /// a frame shows only while the player is in its realm
    #[test]
    fn frames_show_from_their_realm() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins).add_systems(PostUpdate, show_frames);
        let player = app.world_mut().spawn((Player::default(), InRealm(0))).id();
        let outside = app.world_mut().spawn((PortalFrame, InRealm(0), Visibility::default())).id();
        let inside = app.world_mut().spawn((PortalFrame, InRealm(1), Visibility::default())).id();
        let shown = |app: &App, e: Entity| app.world().get::<Visibility>(e) == Some(&Visibility::Inherited);
        app.update();
        assert!(shown(&app, outside) && !shown(&app, inside));
        app.world_mut().get_mut::<InRealm>(player).unwrap().0 = 1;
        app.update();
        assert!(!shown(&app, outside) && shown(&app, inside));
    }

    // CLAUDE
    /// the clip plane is the portal's, in the view's space, facing away from the view
    #[test]
    fn clip_plane_is_the_far_portal() {
        // a view at the origin turned to look along +x; the portal 2 m ahead, showing what's past it
        let camera = Transform::default().looking_to(Vec3::X, Vec3::Y);
        let plane = clip_plane(&camera, Vec3::new(2.0, 0.0, 0.0), Vec3::X).unwrap();
        // in view space ahead is -z
        assert!(plane.truncate().abs_diff_eq(Vec3::NEG_Z, 1e-5) && (plane.w + 1.995).abs() < 1e-5, "{plane}");
        // a point past it is drawn (in front of the plane), one between view and portal isn't
        let side = |p: Vec3| plane.dot(camera.compute_affine().inverse().transform_point3(p).extend(1.0));
        assert!(side(Vec3::new(5.0, 1.0, 0.0)) > 0.0 && side(Vec3::new(1.0, 0.0, 0.0)) < 0.0);
        // the view past the portal (in front of it): no plane
        assert!(clip_plane(&Transform::from_xyz(3.0, 0.0, 0.0), Vec3::new(2.0, 0.0, 0.0), Vec3::X).is_none());
    }
}
