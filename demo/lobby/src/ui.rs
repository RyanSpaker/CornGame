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
                window
                    .spawn((
                        Name::new("Start demo"),
                        CoreButton,
                        ImageNode::new(assets.load("menu/start.png")),
                        Node {
                            position_type: PositionType::Absolute,
                            right: Val::Px(12.0 * k),
                            bottom: Val::Px(18.0 * k),
                            width: Val::Px(155.0 * k),
                            ..default()
                        },
                    ))
                    .observe(start_demo);
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

// CODEX: the demo exercises monitor picking without depending on Game/SwapLevel/networking.
fn start_demo(_: On<PointerClick>, mut status: Query<&mut Text, With<StartStatus>>) {
    for mut text in &mut status {
        text.0 = "Demo started.\nThe cornfile loaded the cabin,\ncomputer and components.".into();
    }
    info!("Lobby demo: Start clicked");
}
