//! CODEX (GPT-6) 2026-11-05: headless placement regressions, including authored log-gap and desk-edge geometry.
use super::*;
use bevy::{camera::{CameraProjection, ComputedCameraValues}, time::TimeUpdateStrategy};

// CODEX: exercise the lobby keyboard system, including modifier routing and per-note style retention.
#[test]
fn lobby_editor_controls_apply_to_one_note_and_survive_finishing() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).init_resource::<PaperSettings>().init_resource::<PostItSession>()
        .init_resource::<ButtonInput<KeyCode>>().add_message::<KeyboardInput>().add_systems(Update, keyboard);
    let note = app.world_mut().spawn((Paper::default(), PaperText {
        text: "one two three four five six seven eight nine ten".into(), editing: true, ..default()
    })).id();
    let other = app.world_mut().spawn((Paper::default(), PaperText::default())).id();
    app.update();
    let defaults = app.world().get::<PaperStyle>(note).unwrap().clone();
    tap_editor_key(&mut app, KeyCode::ArrowRight, false, false);
    assert_eq!(app.world().get::<PaperStyle>(note).unwrap().font_index, 1);
    tap_editor_key(&mut app, KeyCode::ArrowUp, false, true);
    assert!((app.world().get::<PaperStyle>(note).unwrap().margin - defaults.margin - 0.02).abs() < 1e-5);
    tap_editor_key(&mut app, KeyCode::ArrowRight, false, true);
    assert_eq!(app.world().get::<PaperStyle>(note).unwrap().alignment, 5);
    let text = app.world().get::<PaperText>(note).unwrap().text.clone();
    let style = app.world().get::<PaperStyle>(note).unwrap();
    let before = post_it_text::fitted_line_count(&text, 512, style.margin, style.font_size, style.font().bytes);
    tap_editor_key(&mut app, KeyCode::ArrowDown, true, false);
    let style = app.world().get::<PaperStyle>(note).unwrap();
    assert!(post_it_text::fitted_line_count(&text, 512, style.margin, style.font_size, style.font().bytes) < before);
    assert_eq!(*app.world().get::<PaperStyle>(other).unwrap(), defaults, "other notes retain their own style");
    app.world_mut().resource_mut::<Messages<KeyboardInput>>().write(KeyboardInput {
        key_code: KeyCode::Enter, logical_key: bevy::input::keyboard::Key::Enter,
        state: bevy::input::ButtonState::Pressed, text: None, repeat: false, window: Entity::PLACEHOLDER,
    });
    tap_editor_key(&mut app, KeyCode::Enter, true, false);
    assert!(app.world().get::<PaperText>(note).unwrap().text.ends_with('\n'));
    assert!(app.world().get::<PaperText>(note).unwrap().action.is_empty());
    let saved = app.world().get::<PaperStyle>(note).unwrap().clone();
    tap_editor_key(&mut app, KeyCode::Enter, false, false);
    assert_eq!(app.world().get::<PaperText>(note).unwrap().action, "finish");
    app.world_mut().get_mut::<PaperText>(note).unwrap().editing = false;
    tap_editor_key(&mut app, KeyCode::ArrowRight, false, false);
    assert_eq!(*app.world().get::<PaperStyle>(note).unwrap(), saved, "finished notes retain their style");
}

// CODEX: feed the real ButtonInput route used by both native input and existing smoke scripts.
fn tap_editor_key(app: &mut App, key: KeyCode, ctrl: bool, shift: bool) {
    let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
    keys.reset_all();
    if ctrl { keys.press(KeyCode::ControlLeft); }
    if shift { keys.press(KeyCode::ShiftLeft); }
    keys.press(key);
    app.update();
}

// CODEX: verify the lobby's real texture cache and HUD react to typography and retain finished styling.
#[test]
fn lobby_renderer_and_hud_follow_per_note_style() {
    use bevy::ecs::system::RunSystemOnce;
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), TransformPlugin, bevy::mesh::MeshPlugin))
        .init_asset::<Image>().init_asset::<StandardMaterial>()
        .init_resource::<PaperSettings>().init_resource::<PostItSession>().add_systems(Update, render_text);
    // CODEX: use the real glTF material values, including a lit navy base and matte roughness.
    let scene: serde_json::Value = serde_json::from_str(include_str!("../assets/lobby.gltf")).unwrap();
    let source = scene["materials"].as_array().unwrap().iter()
        .find(|m| m["name"] == "postit_handwritten_ink").unwrap();
    let pbr = &source["pbrMetallicRoughness"];
    let color = pbr["baseColorFactor"].as_array().unwrap();
    let authored = app.world_mut().resource_mut::<Assets<StandardMaterial>>().add(StandardMaterial {
        base_color: Color::linear_rgba(color[0].as_f64().unwrap() as f32, color[1].as_f64().unwrap() as f32,
            color[2].as_f64().unwrap() as f32, color[3].as_f64().unwrap() as f32),
        metallic: pbr["metallicFactor"].as_f64().unwrap() as f32,
        perceptual_roughness: pbr["roughnessFactor"].as_f64().unwrap() as f32,
        cull_mode: None, ..default()
    });
    app.world_mut().spawn((bevy::gltf::GltfMaterialName("postit_handwritten_ink".into()), MeshMaterial3d(authored.clone())));
    let mesh = app.world_mut().resource_mut::<Assets<Mesh>>().add(Plane3d::default().mesh().size(0.076, 0.076));
    let note = app.world_mut().spawn((Paper { min: Vec3::splat(-0.038), max: Vec3::splat(0.038) },
        PaperText { text: "Corn".into(), editing: true, ..default() }, Transform::IDENTITY))
        .with_child((Mesh3d(mesh), Transform::IDENTITY)).id();
    app.update();
    let image = app.world().get::<PaperInk>(note).unwrap().image.clone();
    let first = app.world().resource::<Assets<Image>>().get(&image).unwrap().data.clone();
    // CODEX: cloned ink keeps the authored lighting/color/roughness and leaves the source untouched.
    let mut query = app.world_mut().query::<(&ChildOf, &MeshMaterial3d<StandardMaterial>)>();
    let handle = query.iter(app.world()).find(|(parent, _)| parent.parent() == note).unwrap().1.0.clone();
    let materials = app.world().resource::<Assets<StandardMaterial>>();
    let copied = materials.get(&handle).unwrap();
    let original = materials.get(&authored).unwrap();
    assert_ne!(handle, authored);
    assert!(!copied.unlit);
    assert_eq!(copied.base_color, original.base_color);
    assert_eq!(copied.perceptual_roughness, original.perceptual_roughness);
    assert_eq!(copied.metallic, original.metallic);
    assert_eq!(copied.alpha_mode, AlphaMode::Blend);
    assert!(original.base_color_texture.is_none());
    assert!(first.as_ref().unwrap().chunks_exact(4).all(|pixel| pixel[..3] == [255, 255, 255]));
    app.world_mut().get_mut::<PaperStyle>(note).unwrap().font_index = 1;
    app.world_mut().get_mut::<PaperStyle>(note).unwrap().alignment = 0;
    app.update();
    let ink = app.world().get::<PaperInk>(note).unwrap();
    assert_eq!(ink.style.font_index, 1);
    assert_eq!(ink.style.alignment, 0);
    assert!(ink.description.contains("Roboto"));
    assert_ne!(first, app.world().resource::<Assets<Image>>().get(&image).unwrap().data);
    let controls = app.world_mut().spawn((PaperEditorHint, Text::new(""), Visibility::Hidden)).id();
    app.world_mut().resource_mut::<PostItSession>().editing = Some(note);
    app.world_mut().run_system_once(editor_prompt).unwrap();
    assert_eq!(*app.world().get::<Visibility>(controls).unwrap(), Visibility::Inherited);
    assert!(app.world().get::<Text>(controls).unwrap().0.contains("Roboto"));
    app.world_mut().get_mut::<PaperText>(note).unwrap().editing = false;
    app.world_mut().resource_mut::<PostItSession>().editing = None;
    app.update();
    app.world_mut().run_system_once(editor_prompt).unwrap();
    assert_eq!(*app.world().get::<Visibility>(controls).unwrap(), Visibility::Hidden);
    let style = app.world().get::<PaperStyle>(note).unwrap();
    let expected = ink_mask(style.image("Corn", false, false, 512));
    assert_eq!(app.world().resource::<Assets<Image>>().get(&image).unwrap().data, expected.data);
}

// CODEX: placement rejects distant/dynamic surfaces; accepted notes follow their moving parents.
#[test]
fn projection_obeys_reach_and_parent_motion() {
    for (distance, body, accepted) in [(0.8, RigidBody::Static, true), (2.0, RigidBody::Static, false), (0.8, RigidBody::Dynamic, false), (0.8, RigidBody::Kinematic, true)] {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::diagnostic::DiagnosticsPlugin, TransformPlugin,
            AssetPlugin::default(), bevy::mesh::MeshPlugin, PhysicsPlugins::default()))
            .init_resource::<PaperSettings>().init_resource::<PostItSession>()
            .insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(1.0 / 60.0)))
            .add_observer(place);
        app.finish();
        app.cleanup();
        app.world_mut().spawn((Player::default(), Transform::IDENTITY, Camera {
            computed: ComputedCameraValues { clip_from_view: PerspectiveProjection::default().get_clip_from_view(), ..default() }, ..default()
        }));
        let parent = app.world_mut().spawn((body, GravityScale(0.0), Collider::cuboid(2.0, 2.0, 0.02),
            Transform::from_xyz(0.0, 0.0, -distance))).id();
        let note = app.world_mut().spawn((Paper { min: Vec3::splat(-0.038), max: Vec3::splat(0.038) },
            Holdable::default(), Held { scale: Vec3::ONE, ..default() }, Transform::IDENTITY)).id();
        for _ in 0..3 { app.update(); }
        app.world_mut().trigger(Interact { entity: note });
        app.update();
        assert_eq!(app.world().get::<PaperText>(note).is_some(), accepted, "body {body:?}, distance {distance}");
        if accepted {
            assert_eq!(app.world().get::<ChildOf>(note).unwrap().parent(), parent);
            assert!(app.world().get::<Held>(note).is_none());
            assert!(app.world().get::<PaperText>(note).unwrap().editing);
            let before = app.world().get::<GlobalTransform>(note).unwrap().translation();
            app.world_mut().get_mut::<Transform>(parent).unwrap().translation.x += 0.25;
            app.update();
            let after = app.world().get::<GlobalTransform>(note).unwrap().translation();
            assert!((after.x - before.x - 0.25).abs() < 1e-4, "paper follows its moving surface");
        } else {
            assert!(app.world().get::<Held>(note).is_some());
        }
    }
}

// CODEX: an off-center log lip must stop a full-size sheet before its center reaches the groove.
// A smaller sheet fits past the lip; this also catches accidental use of screen/held visual size.
#[test]
fn square_sweep_catches_log_lip_and_uses_authored_scale() {
    for (scale, catches_lip) in [(0.5, false), (1.0, true), (2.0, true)] {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::diagnostic::DiagnosticsPlugin, TransformPlugin,
            AssetPlugin::default(), bevy::mesh::MeshPlugin, PhysicsPlugins::default()))
            .init_resource::<PaperSettings>().init_resource::<PostItSession>()
            .insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(1.0 / 60.0)))
            .add_observer(place);
        app.finish();
        app.cleanup();
        app.world_mut().spawn((Player::default(), Transform::IDENTITY, Camera {
            computed: ComputedCameraValues { clip_from_view: PerspectiveProjection::default().get_clip_from_view(), ..default() }, ..default()
        }));
        // CODEX: full-size cases have no center-ray target at all; only the square can find the lip.
        let groove = (!catches_lip).then(|| app.world_mut().spawn((RigidBody::Static, Collider::cuboid(2.0, 2.0, 0.02),
            Transform::from_xyz(0.0, 0.0, -0.8))).id());
        // CODEX: the center ray misses this lip entirely, but the sheet's right edge hits it.
        let lip = app.world_mut().spawn((RigidBody::Static, Collider::cuboid(0.03, 2.0, 0.02),
            Transform::from_xyz(0.045, 0.0, -0.6))).id();
        let note = app.world_mut().spawn((Paper { min: Vec3::new(-0.038, 0.0, -0.038),
            max: Vec3::new(0.038, 0.014, 0.038) }, Holdable::default(),
            Held { scale: Vec3::splat(scale), ..default() }, Transform::IDENTITY)).id();
        for _ in 0..3 { app.update(); }
        app.world_mut().trigger(Interact { entity: note });
        app.update();
        assert_eq!(app.world().get::<ChildOf>(note).unwrap().parent(), if catches_lip { lip } else { groove.unwrap() });
        let global = app.world().get::<GlobalTransform>(note).unwrap();
        assert_eq!(app.world().get::<PaperPlacement>(note).unwrap().used_square_fallback, catches_lip);
        // CODEX: the closest curled vertex stays on the camera side of the contacted face.
        let back = global.transform_point(Vec3::new(0.038, 0.0, 0.038)).z;
        let face = if catches_lip { -0.59 } else { -0.79 };
        assert!(back >= face + 0.001, "paper penetrated the surface: {back} < {face}");
        assert!((global.compute_transform().scale.x - scale).abs() < 1e-4);
    }
}

// CODEX: aiming just above/below a table's front edge must select its top/front face, never a
// diagonal between them. Rotate the whole fixture as well so this cannot pass via axis snapping.
#[test]
fn table_edge_keeps_aimed_face_angle_and_square_clearance() {
    for tilt in [0.0, 0.3] {
        for (target, face_normal) in [(Vec3::new(0.0, 0.0, -0.515), Vec3::Y),
            (Vec3::new(0.0, -0.015, -0.5), Vec3::Z)] {
            let mut app = App::new();
            app.add_plugins((MinimalPlugins, bevy::diagnostic::DiagnosticsPlugin, TransformPlugin,
                AssetPlugin::default(), bevy::mesh::MeshPlugin, PhysicsPlugins::default()))
                .init_resource::<PaperSettings>().init_resource::<PostItSession>()
                .insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(1.0 / 60.0)))
                .add_observer(place);
            app.finish();
            app.cleanup();
            let rotation = Quat::from_rotation_z(tilt);
            let camera = Transform::from_translation(rotation * Vec3::new(0.0, 0.2, 0.0))
                .looking_at(rotation * target, rotation * Vec3::Y);
            app.world_mut().spawn((Player::default(), camera, Camera {
                computed: ComputedCameraValues { clip_from_view: PerspectiveProjection::default().get_clip_from_view(), ..default() }, ..default()
            }));
            let table = app.world_mut().spawn((RigidBody::Static, Collider::cuboid(1.0, 0.1, 0.5),
                Transform { translation: rotation * Vec3::new(0.0, -0.05, -0.75), rotation, ..default() })).id();
            let note = app.world_mut().spawn((Paper { min: Vec3::new(-0.038, 0.0, -0.038),
                max: Vec3::new(0.038, 0.014, 0.038) }, Holdable { below: 0.0, ..default() },
                Held { scale: Vec3::ONE, ..default() }, Transform::IDENTITY)).id();
            for _ in 0..3 { app.update(); }
            app.world_mut().trigger(Interact { entity: note });
            app.update();
            assert_eq!(app.world().get::<ChildOf>(note).unwrap().parent(), table);
            assert!(!app.world().get::<PaperPlacement>(note).unwrap().used_square_fallback,
                "a clear desk edge must retain the original point placement");
            let global = app.world().get::<GlobalTransform>(note).unwrap();
            let actual_normal = global.compute_transform().rotation * Vec3::Y;
            assert!(actual_normal.dot(rotation * face_normal) > 0.99999,
                "edge tilted the note away from the aimed face: {actual_normal:?}");
            // CODEX: check the entire curled sheet, not just its center, against the selected face.
            for x in [-0.038, 0.038] {
                for y in [0.0, 0.014] {
                    for z in [-0.038, 0.038] {
                        let p = rotation.inverse() * global.transform_point(Vec3::new(x, y, z));
                        let clearance = if face_normal == Vec3::Y { p.y } else { p.z + 0.5 };
                        assert!(clearance >= 0.001, "edge buried a paper corner: {clearance}");
                    }
                }
            }
        }
    }
}

// CODEX: read the same authored vertices and node transforms used by lobby.corn's colliders. Keep
// this small glTF fixture reader here so production placement has no asset-format dependencies.
fn authored_mesh(node_name: &str) -> (Vec<Vec3>, Vec<[u32; 3]>) {
    use std::path::Path;
    let assets = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
    let doc: serde_json::Value = serde_json::from_slice(&std::fs::read(assets.join("lobby.gltf")).unwrap()).unwrap();
    let binary = std::fs::read(assets.join(doc["buffers"][0]["uri"].as_str().unwrap())).unwrap();
    let nodes = doc["nodes"].as_array().unwrap();
    let node_index = nodes.iter().position(|node| node["name"] == node_name).unwrap();
    let node = &nodes[node_index];
    let primitive = &doc["meshes"][node["mesh"].as_u64().unwrap() as usize]["primitives"][0];
    // CODEX: exported fixtures use f32 VEC3 positions and unsigned scalar triangle indices.
    let data = |index: usize, packed_stride: usize| {
        let accessor = &doc["accessors"][index];
        let view = &doc["bufferViews"][accessor["bufferView"].as_u64().unwrap() as usize];
        let offset = view["byteOffset"].as_u64().unwrap_or(0) + accessor["byteOffset"].as_u64().unwrap_or(0);
        (&binary[offset as usize..], view["byteStride"].as_u64().unwrap_or(packed_stride as u64) as usize,
            accessor["count"].as_u64().unwrap() as usize, accessor["componentType"].as_u64().unwrap())
    };
    let (positions, stride, count, kind) = data(primitive["attributes"]["POSITION"].as_u64().unwrap() as usize, 12);
    assert_eq!(kind, 5126);
    let mut vertices: Vec<Vec3> = (0..count).map(|i| Vec3::from_array(std::array::from_fn(|axis| {
        let start = i * stride + axis * 4;
        f32::from_le_bytes(positions[start..start + 4].try_into().unwrap())
    }))).collect();
    let index_accessor = primitive["indices"].as_u64().unwrap() as usize;
    let index_width = match doc["accessors"][index_accessor]["componentType"].as_u64().unwrap() {
        5123 => 2, 5125 => 4, other => panic!("unsupported fixture index type: {other}"),
    };
    let (indices, stride, count, _) = data(index_accessor, index_width);
    let indices: Vec<u32> = (0..count).map(|i| {
        let start = i * stride;
        if index_width == 2 { u16::from_le_bytes(indices[start..start + 2].try_into().unwrap()) as u32 }
        else { u32::from_le_bytes(indices[start..start + 4].try_into().unwrap()) }
    }).collect();
    let triangles = indices.chunks_exact(3).map(|triangle| triangle.try_into().unwrap()).collect();
    // CODEX: walk ancestors so the regression stays correct if the authored mesh is regrouped.
    let mut index = node_index;
    loop {
        let node = &nodes[index];
        let vector = |key: &str, fallback: Vec3| node[key].as_array().map_or(fallback,
            |v| Vec3::new(v[0].as_f64().unwrap() as f32, v[1].as_f64().unwrap() as f32, v[2].as_f64().unwrap() as f32));
        let rotation = node["rotation"].as_array().map_or(Quat::IDENTITY,
            |v| Quat::from_array(std::array::from_fn(|i| v[i].as_f64().unwrap() as f32)));
        let local = Transform { translation: vector("translation", Vec3::ZERO), rotation, scale: vector("scale", Vec3::ONE) };
        for vertex in &mut vertices { *vertex = local.transform_point(*vertex); }
        let parent = nodes.iter().position(|n| n["children"].as_array().is_some_and(|children|
            children.iter().any(|child| child.as_u64() == Some(index as u64))));
        let Some(parent) = parent else { break };
        index = parent;
    }
    (vertices, triangles)
}

// CODEX: create a minimal live physics world; callers check the point hit before invoking placement.
fn authored_fixture(collider: Collider, camera: Transform, curl_height: f32) -> (App, Entity, Entity) {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, bevy::diagnostic::DiagnosticsPlugin, TransformPlugin,
        AssetPlugin::default(), bevy::mesh::MeshPlugin, PhysicsPlugins::default()))
        .init_resource::<PaperSettings>().init_resource::<PostItSession>()
        .insert_resource(TimeUpdateStrategy::ManualDuration(std::time::Duration::from_secs_f32(1.0 / 60.0)))
        .add_observer(place);
    app.finish();
    app.cleanup();
    app.world_mut().spawn((Player::default(), camera, Camera {
        computed: ComputedCameraValues { clip_from_view: PerspectiveProjection::default().get_clip_from_view(), ..default() }, ..default()
    }));
    let surface = app.world_mut().spawn((RigidBody::Static, collider, Transform::IDENTITY)).id();
    let note = app.world_mut().spawn((Paper { min: Vec3::new(-0.038, 0.0, -0.038),
        max: Vec3::new(0.038, curl_height, 0.038) }, Holdable { below: 0.0, ..default() },
        Held { scale: Vec3::ONE, ..default() }, Transform::IDENTITY)).id();
    for _ in 0..3 { app.update(); }
    (app, surface, note)
}

// CODEX: reproduce the groove from the user's live placements with the actual cabin bark trimesh.
// Confirm the point ray hits the groove, so fallback cannot pass just because the ray missed.
#[test]
fn authored_log_gap_uses_square_fallback() {
    use bevy::ecs::system::RunSystemOnce;
    let (vertices, triangles) = authored_mesh("Cylinder.001");
    let collider = Collider::trimesh(vertices, triangles);
    for z in [-1.75, -1.56, -1.27] {
        for curl in [0.0144, 0.0211] {
            let camera = Transform::from_xyz(1.9, 1.68, z).looking_at(Vec3::new(2.450187, 1.615167, z), Vec3::Y);
            let (mut app, surface, note) = authored_fixture(collider.clone(), camera, curl);
            let hit = app.world_mut().run_system_once(move |spatial: SpatialQuery| spatial.cast_ray(
                camera.translation, camera.forward(), 1.0, true, &SpatialQueryFilter::default())).unwrap().unwrap();
            assert_eq!(hit.entity, surface);
            assert!(hit.normal.y < -0.5, "fixture must point into the sloped log groove: {:?}", hit.normal);
            app.world_mut().trigger(Interact { entity: note });
            app.update();
            assert!(app.world().get::<PaperPlacement>(note).unwrap().used_square_fallback,
                "log-gap point placement is occluded, z={z}, curl={curl}");
            assert_eq!(app.world().get::<ChildOf>(note).unwrap().parent(), surface);
        }
    }
}

// CODEX: exact point-placement anchors recovered from the user's failed live blank notes. These sit
// higher in the groove than the original fixture, where a square tilted with the paper fits the gap.
#[test]
fn live_upper_log_gap_uses_square_fallback() {
    use bevy::ecs::system::RunSystemOnce;
    let (vertices, triangles) = authored_mesh("Cylinder.001");
    let collider = Collider::trimesh(vertices, triangles);
    let normal = Vec3::new(-0.6322385, -0.7747738, 0.0);
    for origin in [Vec3::new(2.4126754, 1.6438417, -1.9079471),
        Vec3::new(2.3984036, 1.6554879, -1.9799341)] {
        let target = origin - normal * PaperSettings::default().surface_gap;
        for x in [1.9, 2.1] {
            let camera = Transform::from_xyz(x, 1.692, target.z).looking_at(target, Vec3::Y);
            let (mut app, surface, note) = authored_fixture(collider.clone(), camera, 0.021095239);
            let hit = app.world_mut().run_system_once(move |spatial: SpatialQuery| spatial.cast_ray(
                camera.translation, camera.forward(), 1.0, true, &SpatialQueryFilter::default())).unwrap().unwrap();
            assert_eq!(hit.entity, surface);
            assert!(hit.normal.dot(normal) > 0.9999, "live fixture must hit the same sloped face");
            app.world_mut().trigger(Interact { entity: note });
            app.update();
            assert!(app.world().get::<PaperPlacement>(note).unwrap().used_square_fallback,
                "missed the live upper groove: target={target:?}, camera_x={x}");
        }
    }
}

// CODEX: test the actual desk hull with the note overhanging its top/front edge. A clear point
// placement must retain its exact face angle and must not take the log-gap fallback path.
#[test]
fn authored_desk_edge_does_not_use_square_fallback() {
    let (vertices, _) = authored_mesh("tabletop");
    let collider = Collider::convex_hull(&vertices).unwrap();
    let top = vertices.iter().map(|v| v.y).fold(f32::NEG_INFINITY, f32::max);
    let front = vertices.iter().map(|v| v.z).fold(f32::INFINITY, f32::min);
    for inset in [0.005, 0.015, 0.03] {
        for (target, normal) in [(Vec3::new(1.5, top, front + inset), Vec3::Y),
            (Vec3::new(1.5, top - inset, front), Vec3::NEG_Z)] {
            let camera = Transform::from_xyz(1.5, 1.65, front - 0.5).looking_at(target, Vec3::Y);
            let (mut app, surface, note) = authored_fixture(collider.clone(), camera, 0.0211);
            app.world_mut().trigger(Interact { entity: note });
            app.update();
            assert!(!app.world().get::<PaperPlacement>(note).unwrap().used_square_fallback,
                "desk-edge point placement is visible, target={target:?}");
            assert_eq!(app.world().get::<ChildOf>(note).unwrap().parent(), surface);
            let placed = app.world().get::<GlobalTransform>(note).unwrap().compute_transform();
            assert!((placed.rotation * Vec3::Y).dot(normal) > 0.9999,
                "point placement must retain the desk's true face angle");
        }
    }
}

// CODEX (GPT-6): assistance follows editing/tuning without modifying the authored PBR material.
#[test]
fn editing_glow_preserves_authored_material_and_disables_on_finish() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).init_resource::<PaperSettings>()
        .init_resource::<Assets<PaperMaterial>>().add_systems(Update, editor_glow);
    let authored = LinearRgba::rgb(0.01, 0.02, 0.03);
    let material = app.world_mut().resource_mut::<Assets<PaperMaterial>>()
        .add(PaperMaterial { base: StandardMaterial { emissive: authored, ..default() }, extension: PaperGlow::default() });
    let note = app.world_mut().spawn((Paper::default(), PaperText { editing: true, ..default() }))
        .with_child((MeshMaterial3d(material.clone()), PaperSurface)).id();
    app.update();
    let paper = app.world().resource::<Assets<PaperMaterial>>().get(&material).unwrap();
    assert_eq!(paper.base.emissive, authored);
    assert_eq!(paper.extension.glow.w, 1.0);
    assert_eq!(paper.extension.darkness.x, 0.001);
    assert_eq!(paper.extension.darkness.y, 0.002);
    assert_eq!(paper.extension.darkness.z, 0.0046);
    app.world_mut().resource_mut::<PaperSettings>().editing_darkness_threshold = 0.004;
    app.update();
    let paper = app.world().resource::<Assets<PaperMaterial>>().get(&material).unwrap();
    assert_eq!(paper.extension.darkness.x, 0.004);
    app.world_mut().get_mut::<PaperText>(note).unwrap().editing = false;
    app.update();
    let paper = app.world().resource::<Assets<PaperMaterial>>().get(&material).unwrap();
    assert_eq!(paper.extension.glow.w, 0.0);
    assert_eq!(paper.base.emissive, authored);
}
