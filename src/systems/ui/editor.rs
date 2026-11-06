
use std::env;
use std::sync::Arc;

use bevy::prelude::*;
use bevy_editor_pls::controls::{self, EditorControls};
use bevy_editor_pls::editor_window::{EditorWindow, EditorWindowContext};
use bevy_editor_pls::{egui, spawn_default_windows, AddEditorWindow, EditorPlugin, EguiPlugin};
use parking_lot::RwLock;

use crate::ecs::corn::stored::image::ImageCarvedHexagonalShader;
use crate::ecs::corn::sensor::CornSensor;
use crate::scenes::LoadScene;
use crate::systems::character::Player;
use crate::systems::network::NetworkWindow;
// mod compat; 

fn eguibad<T: Send + Sync + Default + 'static>(ui: &mut egui::Ui, id: egui::Id) -> Arc<RwLock<T>>{
    ui.ctx().data_mut(|d| d.get_temp_mut_or_insert_with::<Arc<RwLock<T>>>(id, Default::default).clone())
}

#[derive(Debug, Clone, Default, Component)]
struct SceneLoadWindow;

impl EditorWindow for SceneLoadWindow {
    fn ui(&self, world: &mut World, _cx: EditorWindowContext, ui: &mut egui::Ui) {
        // TODO tab complete
        let buffer = eguibad::<String>(ui, ui.auto_id_with("path"));
        ui.horizontal(|ui| {
            ui.label("Scene to load:");
            ui.text_edit_singleline(&mut *buffer.write());
        });

        // Add a button to trigger scene loading
        if ui.button("Load Scene").clicked() {
            world.spawn(LoadScene::new(&**buffer.read()));
        }
    }
}

/// Draws the loaded maze as a top-down diagnostic map. It deliberately reads the
/// runtime AABBs/transforms, making broken GLTF transforms obvious without another export.
#[derive(Debug, Clone, Default, Component)]
struct MapDevWindow;

#[derive(Default)]
struct MapDevSettings {
    show_labels: bool,
    corn_textures: Vec<CornMapTexture>,
}

/// Egui owns its textures, so retain one per Bevy image instead of uploading the carving
/// image every editor frame. The texture is rebuilt after a process restart/hot reload.
struct CornMapTexture {
    image: Handle<Image>,
    texture: egui::TextureHandle,
}

#[derive(Clone)]
struct MapBox {
    min: Vec2,
    max: Vec2,
    name: String,
}

struct MapPlayer {
    position: Vec2,
    is_in_corn: bool,
    value: f32,
    samples: Vec<MapCornSample>,
}

struct MapCornSample {
    field: Entity,
    uv: Vec2,
    coverage: f32,
}

struct MapCornField {
    bounds: MapBox,
    texture: Option<egui::TextureHandle>,
}

impl EditorWindow for MapDevWindow {
    fn name(&self, _world: &mut World, _cx: EditorWindowContext<'_>) -> String {
        "Map dev".to_string()
    }

    fn ui(&self, world: &mut World, _cx: EditorWindowContext, ui: &mut egui::Ui) {
        let settings = eguibad::<MapDevSettings>(ui, ui.auto_id_with("map dev settings"));
        ui.checkbox(&mut settings.write().show_labels, "object labels");

        let corn_sources = world
            .query::<(&ImageCarvedHexagonalShader, &GlobalTransform)>()
            .iter(world)
            .map(|(field, transform)| {
                // Mirror the sensor conversion: the shader's center is field-local and the
                // entity GlobalTransform puts that local point into the rendered world.
                let center = transform.transform_point(field.center);
                (
                    MapBox {
                        min: center.xz() - field.half_extents,
                        max: center.xz() + field.half_extents,
                        name: "corn".to_string(),
                    },
                    field.image.clone(),
                )
            })
            .collect::<Vec<_>>();
        let corn = {
            let images = world.resource::<Assets<Image>>();
            let mut settings = settings.write();
            corn_sources
                .into_iter()
                .map(|(bounds, handle)| MapCornField {
                    texture: images.get(handle.id()).and_then(|image| {
                        corn_map_texture(&mut settings, ui.ctx(), &handle, image)
                    }),
                    bounds,
                })
                .collect::<Vec<_>>()
        };
        let players = world
            .query_filtered::<(&GlobalTransform, Option<&CornSensor>), With<Player>>()
            .iter(world)
            .map(|(transform, sensor)| MapPlayer {
                position: transform.translation().xz(),
                is_in_corn: sensor.is_some_and(|sensor| sensor.is_in_corn),
                value: sensor.map_or(0.0, |sensor| sensor.value),
                samples: sensor.map_or_else(Vec::new, |sensor| {
                    sensor
                        .cornfields
                        .iter()
                        .map(|(field, location)| MapCornSample {
                            field: *field,
                            uv: location.pos,
                            // CornSensor uses this same linear red value as its coverage.
                            coverage: location.val.to_linear().red,
                        })
                        .collect()
                }),
            })
            .collect::<Vec<_>>();

        ui.label(format!("{} active corn fields · {} players", corn.len(), players.len()));
        if corn.is_empty() {
            ui.colored_label(egui::Color32::RED, "No active ImageCarvedHexagonalShader fields");
        }
        for (index, player) in players.iter().enumerate() {
            ui.label(format!(
                "player {}: x={:.2} z={:.2} · in corn={} · value={:.3} · sampled fields={}",
                index + 1,
                player.position.x,
                player.position.y,
                player.is_in_corn,
                player.value,
                player.samples.len(),
            ));
            for sample in &player.samples {
                ui.small(format!(
                    "  field {}: uv=({:.3}, {:.3}) red={:.3}",
                    sample.field, sample.uv.x, sample.uv.y, sample.coverage,
                ));
            }
        }
        draw_map(ui, &corn, &players, settings.read().show_labels);
    }
}

/// Converts the exact red channel sampled by `CornSensor` into a translucent green map.
/// A black/carved texel becomes transparent; a bright texel remains visible corn.
fn corn_map_texture(
    settings: &mut MapDevSettings,
    context: &egui::Context,
    handle: &Handle<Image>,
    image: &Image,
) -> Option<egui::TextureHandle> {
    if let Some(cached) = settings.corn_textures.iter().find(|cached| cached.image == *handle) {
        return Some(cached.texture.clone());
    }
    let rgba = image.clone().try_into_dynamic().ok()?.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    let pixels = rgba
        .pixels()
        .map(|pixel| {
            // Keep this conversion identical to CornSensor's sampled `value` path.
            let coverage = Color::Srgba(Srgba::from_u8_array(pixel.0)).to_linear().red;
            egui::Color32::from_rgba_unmultiplied(35, 190, 65, (coverage * 190.0) as u8)
        })
        .collect();
    let texture = context.load_texture(
        format!("corn carving {:?}", handle.id()),
        egui::ColorImage { size, pixels },
        egui::TextureOptions::NEAREST,
    );
    settings.corn_textures.push(CornMapTexture {
        image: handle.clone(),
        texture: texture.clone(),
    });
    Some(texture)
}

fn draw_map(
    ui: &mut egui::Ui,
    corn: &[MapCornField],
    players: &[MapPlayer],
    show_labels: bool,
) {
    let (response, painter) = ui.allocate_painter(egui::vec2(440.0, 440.0), egui::Sense::hover());
    let mut min = Vec2::splat(f32::INFINITY);
    let mut max = Vec2::splat(f32::NEG_INFINITY);
    for bounds in corn.iter().map(|field| &field.bounds) {
        min = min.min(bounds.min);
        max = max.max(bounds.max);
    }
    for player in players {
        min = min.min(player.position);
        max = max.max(player.position);
    }
    if !min.is_finite() {
        painter.text(response.rect.center(), egui::Align2::CENTER_CENTER, "No map data loaded", egui::FontId::proportional(14.0), egui::Color32::GRAY);
        return;
    }

    // Ten percent padding and a minimum map span keep one small object readable.
    let span = (max - min).max(Vec2::splat(1.0));
    min -= span * 0.1;
    max += span * 0.1;
    let map_span = max - min;
    let scale = (response.rect.width() / map_span.x).min(response.rect.height() / map_span.y);
    let map_point = |point: Vec2| {
        egui::pos2(
            response.rect.center().x + (point.x - (min.x + max.x) * 0.5) * scale,
            // Deliberately keep +Z downward in the editor image. This matches the texture's
            // top-to-bottom V axis exactly, so a sensor UV and a visible carving coincide.
            response.rect.center().y + (point.y - (min.y + max.y) * 0.5) * scale,
        )
    };
    painter.rect_filled(response.rect, 0.0, egui::Color32::from_gray(18));

    // The carving texture is below diagnostic geometry, matching the field/image lookup used
    // by CornSensor while leaving carved (black) texels transparent.
    for field in corn {
        let rect = egui::Rect::from_two_pos(map_point(field.bounds.min), map_point(field.bounds.max));
        if let Some(texture) = &field.texture {
            painter.image(texture.id(), rect, egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
        } else {
            painter.rect_filled(rect, 0.0, egui::Color32::from_rgba_unmultiplied(35, 190, 65, 80));
        }
        painter.rect_stroke(rect, 0.0, egui::Stroke::new(1.0, egui::Color32::GREEN), egui::StrokeKind::Middle);
    }
    if show_labels {
        for field in corn {
            let rect = egui::Rect::from_two_pos(map_point(field.bounds.min), map_point(field.bounds.max));
            painter.text(rect.left_top(), egui::Align2::LEFT_TOP, &field.bounds.name, egui::FontId::proportional(10.0), egui::Color32::GREEN);
        }
    }
    for player in players {
        painter.circle_filled(map_point(player.position), 4.0, egui::Color32::RED);
    }
}

#[derive(Debug)]
pub struct MyEditorPlugin;
impl Plugin for MyEditorPlugin{
    fn build(&self, app: &mut App) {
        if env::var("CORN_EDITOR") != Ok("0".to_string()) {
            app.add_plugins(EguiPlugin{
                enable_multipass_for_primary_context: false,
            });
            app.add_plugins(EditorPlugin::default());
            app.insert_resource(editor_controls());
            app.add_editor_window::<SceneLoadWindow>();
            app.add_editor_window::<MapDevWindow>();
            app.add_systems(Startup, spawn_default_windows);
            app.add_plugins(NetworkWindow);
        }

        app.add_systems(Startup, |mut window: Query<&mut Window>, cli: Res<crate::Cli>|{
            for mut w in window.iter_mut() {
                dbg!(w.resolution.scale_factor());
                dbg!(w.resolution.base_scale_factor());
                dbg!(w.resolution.scale_factor_override());

                w.resolution.set_scale_factor(1.5);
                if cli.client {
                    w.title = "client".into();
                }
                if cli.server {
                    w.title = "server".into();
                    if cli.client {
                        w.title = "client server".into()
                    }
                }
            }      
        });
    }
}

fn editor_controls() -> EditorControls {
    let mut editor_controls = EditorControls::default_bindings();
    editor_controls.unbind(controls::Action::PlayPauseEditor);

    editor_controls.insert(
        controls::Action::PlayPauseEditor,
        controls::Binding {
            input: controls::UserInput::Single(controls::Button::Keyboard(KeyCode::F3)),
            conditions: vec![controls::BindingCondition::ListeningForText(false)],
        },
    );

    editor_controls
}
