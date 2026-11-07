//! CODEX (GPT-6): procedural desktop isolated for the second-stage corn/script port.
//! This adapts src/ecs/menu_lobby.rs's native Bevy UI, keeping the original artwork and composition.
use bevy::{prelude::*, text::FontSmoothing, ui_widgets::Button as CoreButton};

// CODEX: explicit UI roles avoid any dependency on the game's network state or relationships.
#[derive(Component)]
pub struct DesktopCursor;
// CODEX: local preview count label.
#[derive(Component)]
pub struct PlayerCount;
// CODEX: local Start response, deliberately independent of gameplay scene loading.
// CLAUDE: reflected for scripts (dev.rs)
#[derive(Component, Reflect, Default)]
#[reflect(Component)]
pub struct StartStatus;

// CLAUDE: raised by the desktop's confirmed "Reset" (Yes), on the monitor the local player is using. The
// network adapter (network.rs) answers it; this module knows nothing of servers.
#[derive(Event)]
pub struct ResetLobby;

// CLAUDE: the full-window "reset everyone?" dialog; Display::None until Reset is clicked.
#[derive(Component)]
struct ResetConfirm;

// CODEX: owns the entities that the monitor input adapter needs.
pub struct Desktop {
    pub cursor: Entity,
    pub root: Entity,
}

// CODEX: visual tuning grouped in one place until the desktop becomes corn-authored.
const BRIGHT: Color = Color::srgb_u8(0x67, 0x60, 0x05);
const DARK: Color = Color::srgb_u8(0x42, 0x3d, 0x00);

// CODEX: render the original wallpaper, toolbar, window and controls onto the monitor's UI camera.
pub fn build(commands: &mut Commands, assets: &AssetServer, camera: Entity, size: f32) -> Desktop {
    let font: Handle<Font> = assets.load("fonts/roboto.ttf");
    let k = size / 512.0;
    let text_font = TextFont {
        font: font.into(),
        font_size: (16.0 * k).into(),
        font_smoothing: FontSmoothing::None,
        ..default()
    };
    let root = commands
        .spawn((
            Name::new("Computer desktop"),
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            UiTargetCamera(camera),
        ))
        .id();
    let mut cursor_entity = Entity::PLACEHOLDER;
    commands.entity(root).with_children(|desktop| {
        desktop.spawn((
            Name::new("wallpaper"),
            // CODEX: 0.20 Auto letterboxes images; Stretch preserves 0.16's fixed-size node rendering.
            ImageNode::new(assets.load("menu/wallpaper.jpg")).with_mode(NodeImageMode::Stretch),
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            ZIndex(-10),
        ));
        desktop
            .spawn((
                Name::new("toolbar"),
                Node {
                    position_type: PositionType::Absolute,
                    box_sizing: BoxSizing::ContentBox,
                    width: Val::Percent(100.0),
                    height: Val::Px(23.0 * k),
                    border: UiRect::all(Val::Px(4.0 * k)),
                    ..default()
                },
                BackgroundColor(BRIGHT),
                BorderColor::all(Color::BLACK),
            ))
            .with_children(|bar| {
                bar.spawn((
                    ImageNode::new(assets.load("menu/logo.png")),
                    Node {
                        height: Val::Percent(100.0),
                        ..default()
                    },
                ));
                bar.spawn((
                    Node {
                        flex_grow: 1.0,
                        border: UiRect::left(Val::Px(4.0 * k)),
                        ..default()
                    },
                    BorderColor::all(Color::BLACK),
                ));
                bar.spawn((
                    Text::new("12:57"),
                    text_font.clone(),
                    TextColor(Color::BLACK),
                    Node {
                        margin: UiRect {
                            top: Val::Px(2.0 * k),
                            right: Val::Px(10.0 * k),
                            ..default()
                        },
                        ..default()
                    },
                ));
            });
        desktop
            .spawn((
                Name::new("window"),
                Node {
                    position_type: PositionType::Relative,
                    width: Val::Percent(80.0),
                    height: Val::Percent(70.0),
                    left: Val::Percent(13.0),
                    top: Val::Percent(17.0),
                    border: UiRect::all(Val::Px(5.0 * k)),
                    flex_direction: FlexDirection::Column,
                    ..default()
                },
                BackgroundColor(DARK),
                BorderColor::all(Color::BLACK),
            ))
            .with_children(|window| {
                window
                    .spawn((
                        Node {
                            box_sizing: BoxSizing::ContentBox,
                            width: Val::Percent(100.0),
                            height: Val::Px(16.0 * k),
                            border: UiRect::bottom(Val::Px(4.0 * k)),
                            ..default()
                        },
                        BackgroundColor(Color::BLACK),
                        BorderColor::all(Color::BLACK),
                    ))
                    .with_children(|bar| {
                        bar.spawn((
                            Text::new("CORN GAME"),
                            text_font.clone(),
                            TextColor(Color::BLACK),
                            BackgroundColor(BRIGHT),
                        ));
                        bar.spawn(Node {
                            flex_grow: 1.0,
                            ..default()
                        });
                        for path in ["menu/minus.jpg", "menu/plus.jpg"] {
                            bar.spawn((ImageNode::new(assets.load(path)), Node::default()));
                        }
                    });
                // CODEX: preserve the original grid wrapper and its place in the window's flow.
                window
                    .spawn(Node {
                        display: Display::Grid,
                        ..default()
                    })
                    .with_child((
                        ImageNode::new(assets.load("menu/lock.png")),
                        Node {
                            width: Val::Px(100.0 * k),
                            height: Val::Px(100.0 * k),
                            ..default()
                        },
                    ));
                window.spawn((
                    Text::new(""),
                    text_font.clone(),
                    TextColor(Color::BLACK),
                    StartStatus,
                    Node {
                        position_type: PositionType::Absolute,
                        top: Val::Px(130.0 * k),
                        left: Val::Px(10.0 * k),
                        ..default()
                    },
                ));
                window.spawn((
                    Name::new("player count"),
                    Text::new("1"),
                    TextFont::from_font_size(20.0 * k),
                    PlayerCount,
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(12.0 * k),
                        bottom: Val::Px(18.0 * k),
                        width: Val::Px(155.0 * k),
                        ..default()
                    },
                ));
                // CLAUDE: Reset sits above Start, bottom right.
                window
                    .spawn(Node {
                        position_type: PositionType::Absolute,
                        right: Val::Px(12.0 * k),
                        bottom: Val::Px(18.0 * k),
                        width: Val::Px(155.0 * k),
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(8.0 * k),
                        ..default()
                    })
                    .with_children(|column| {
                        button(column, "Reset", &text_font, k, 155.0, 26.0).observe(open_reset);
                        column
                            .spawn((
                                Name::new("Start demo"),
                                CoreButton,
                                ImageNode::new(assets.load("menu/start.png")),
                                Node {
                                    width: Val::Px(155.0 * k),
                                    ..default()
                                },
                            ))
                            .observe(start_demo);
                    });
                // CLAUDE: over the whole window, so the buttons below can't be clicked while it is open.
                window
                    .spawn((
                        Name::new("Reset confirm"),
                        ResetConfirm,
                        Pickable::default(),
                        Node {
                            display: Display::None,
                            position_type: PositionType::Absolute,
                            left: Val::Px(0.0),
                            top: Val::Px(0.0),
                            width: Val::Percent(100.0),
                            height: Val::Percent(100.0),
                            align_items: AlignItems::Center,
                            justify_content: JustifyContent::Center,
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
                        ZIndex(5),
                    ))
                    .with_children(|overlay| {
                        overlay
                            .spawn((
                                Node {
                                    flex_direction: FlexDirection::Column,
                                    align_items: AlignItems::Center,
                                    row_gap: Val::Px(8.0 * k),
                                    padding: UiRect::all(Val::Px(10.0 * k)),
                                    border: UiRect::all(Val::Px(4.0 * k)),
                                    ..default()
                                },
                                BackgroundColor(BRIGHT),
                                BorderColor::all(Color::BLACK),
                            ))
                            .with_children(|panel| {
                                for line in ["Reset for everyone?", "Items, notes and players", "go back to the start."] {
                                    panel.spawn((Text::new(line), text_font.clone(), TextColor(Color::BLACK)));
                                }
                                panel
                                    .spawn(Node {
                                        column_gap: Val::Px(12.0 * k),
                                        margin: UiRect::top(Val::Px(4.0 * k)),
                                        ..default()
                                    })
                                    .with_children(|row| {
                                        button(row, "Yes", &text_font, k, 60.0, 26.0).observe(confirm_reset);
                                        button(row, "No", &text_font, k, 60.0, 26.0).observe(close_reset);
                                    });
                            });
                    });
            });
        cursor_entity = desktop
            .spawn((
                Name::new("Monitor cursor"),
                DesktopCursor,
                ImageNode::new(assets.load("menu/cursor.png")),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(size * 0.65),
                    top: Val::Px(size * 0.65),
                    width: Val::Px(47.0 * k),
                    ..default()
                },
                Pickable::IGNORE,
                ZIndex(10),
            ))
            .id();
    });
    Desktop {
        root,
        cursor: cursor_entity,
    }
}

// CLAUDE: Start shows a snapshot of the lobby: server address (if any), posted notes, door and radio.
// The network resources are optional so the desktop still works with the network plugin removed. The
// owner's text is mirrored to observers (network.rs), so nothing here says "you".
fn start_demo(
    _: On<PointerClick>,
    mut status: Query<&mut Text, With<StartStatus>>,
    settings: Option<Res<crate::network::NetworkSettings>>,
    notes: Query<
        Option<&crate::network::NetworkPaper>,
        (
            With<crate::post_it::PaperOrigin>,
            Without<crate::hold::Held>,
            Without<crate::network::RemoteHeld>,
        ),
    >,
    doors: Query<&crate::door::Door>,
    radios: Query<&crate::radio::Radio>,
) {
    let mut lines = vec!["Demo started.".to_string()];
    // CLAUDE: just the server address, or N/A when offline.
    let server = settings
        .as_deref()
        .filter(|s| s.enabled)
        .map(|s| {
            let host = s.url.split("://").last().unwrap_or_default();
            host.strip_suffix("/lobby").unwrap_or(host).to_string()
        })
        .unwrap_or_else(|| "N/A".into());
    lines.push(format!("Server: {server}"));
    let (total, remote) = notes
        .iter()
        .fold((0, 0), |(t, r), net| (t + 1, r + net.is_some_and(|n| n.remote) as u32));
    lines.push(if remote > 0 {
        format!("Notes: {total} posted ({remote} remote)")
    } else {
        format!("Notes: {total} posted")
    });
    for door in &doors {
        lines.push(format!(
            "Door: {}, {}",
            if door.open { "open" } else { "closed" },
            if door.unlocked { "unlocked" } else { "locked" }
        ));
    }
    for radio in &radios {
        lines.push(format!("Radio: {}", if radio.on { "on" } else { "off" }));
    }
    let report = lines.join("\n");
    for mut text in &mut status {
        text.0 = report.clone();
    }
    info!("Lobby demo: Start clicked\n{report}");
}

// CLAUDE: a plain text button in the desktop's palette, `width` x `height` at 512 px.
fn button<'a>(
    parent: &'a mut ChildSpawnerCommands<'_>,
    label: &str,
    font: &TextFont,
    k: f32,
    width: f32,
    height: f32,
) -> EntityCommands<'a> {
    let mut button = parent.spawn((
        Name::new(format!("{label} button")),
        CoreButton,
        Node {
            width: Val::Px(width * k),
            height: Val::Px(height * k),
            border: UiRect::all(Val::Px(3.0 * k)),
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            ..default()
        },
        BackgroundColor(DARK),
        BorderColor::all(Color::BLACK),
    ));
    button.with_child((Text::new(label), font.clone(), TextColor(BRIGHT), Pickable::IGNORE));
    button
}

// CLAUDE: show or hide the reset dialog.
fn set_reset_dialog(dialog: &mut Query<&mut Node, With<ResetConfirm>>, open: bool) {
    for mut node in dialog {
        node.display = if open { Display::Flex } else { Display::None };
    }
}

// CLAUDE: a reset ends everyone's use of the computer, so no one is left looking at its dialog.
pub(crate) fn close_reset_dialog(world: &mut World) {
    let mut dialog = world.query_filtered::<&mut Node, With<ResetConfirm>>();
    for mut node in dialog.iter_mut(world) {
        node.display = Display::None;
    }
}

// CLAUDE: Reset asks first. Remote observers of the monitor see the dialog too (mirrored clicks).
fn open_reset(_: On<PointerClick>, mut dialog: Query<&mut Node, With<ResetConfirm>>) {
    set_reset_dialog(&mut dialog, true);
}

// CLAUDE: No.
fn close_reset(_: On<PointerClick>, mut dialog: Query<&mut Node, With<ResetConfirm>>) {
    set_reset_dialog(&mut dialog, false);
}

// CLAUDE: Yes. Only the player actually at the computer asks; observers' mirrored clicks must not.
fn confirm_reset(
    _: On<PointerClick>,
    mut dialog: Query<&mut Node, With<ResetConfirm>>,
    session: Res<crate::computer::ComputerSession>,
    mut commands: Commands,
) {
    set_reset_dialog(&mut dialog, false);
    if session.active.is_some() {
        commands.trigger(ResetLobby);
    }
}
