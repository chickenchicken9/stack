// Bevy code commonly triggers these lints and they may be important signals
// about code quality. They are sometimes hard to avoid though, and the CI
// workflow treats them as errors, so this allows them throughout the project.
// Feel free to delete this line.
#![allow(clippy::too_many_arguments, clippy::type_complexity)]

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_ggrs::*;
use bevy_matchbox::matchbox_socket::{PeerId, SingleChannel};
use bevy_matchbox::MatchboxSocket;
use bytemuck::{Pod, Zeroable};

fn main() {
    let mut app = App::new();

    GGRSPlugin::<GgrsConfig>::new()
        .with_input_system(input)
        .register_rollback_component::<Transform>()
        .build(&mut app);

    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            // fill the entire browser window
            fit_canvas_to_parent: true,
            // don't hijack keyboard shortcuts like F5, F6, F12, Ctrl+R etc.
            prevent_default_event_handling: false,
            ..default()
        }),
        ..default()
    }))
    .insert_resource(ClearColor(Color::rgb(0.53, 0.53, 0.53)))
    .add_startup_systems((setup, spawn_players, start_matchbox_socket))
    .add_systems((move_players.in_schedule(GGRSSchedule), wait_for_players))
    .run();
}

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn(Camera2dBundle::default());

    commands.spawn((
        // Create a TextBundle that has a Text with a single section.
        TextBundle::from_section(
            // Accepts a `String` or any type that converts into a `String`, such as `&str`
            "hello\nbevy!",
            TextStyle {
                font: asset_server.load("OpenSans-Medium.ttf"),
                font_size: 100.0,
                color: Color::BLACK,
            },
        ) // Set the alignment of the Text
        .with_text_alignment(TextAlignment::Center)
        // Set the style of the TextBundle itself.
        .with_style(Style {
            position_type: PositionType::Absolute,
            position: UiRect {
                top: Val::Px(5.0),
                left: Val::Px(15.0),
                ..default()
            },
            ..default()
        }),
    ));
}

fn start_matchbox_socket(mut commands: Commands) {
    let room_url = "wss://chickenchicken-matchbox.fly.dev/extreme_bevy?next=2";
    info!("connecting to matchbox server: {:?}", room_url);
    commands.insert_resource(MatchboxSocket::new_ggrs(room_url));
}

fn wait_for_players(mut commands: Commands, mut socket: ResMut<MatchboxSocket<SingleChannel>>) {
    // Check for new connections
    socket.update_peers();
    let players = socket.players();

    let num_players = 2;
    if players.len() < num_players {
        return; // wait for more players
    }

    // create a GGRS P2P session
    let mut session_builder = ggrs::SessionBuilder::<GgrsConfig>::new()
        .with_num_players(num_players)
        .with_input_delay(2);

    for (i, player) in players.into_iter().enumerate() {
        session_builder = session_builder
            .add_player(player, i)
            .expect("failed to add player");
    }

    // move the channel out of the socket (required because GGRS takes ownership of it)
    if let Ok(channel) = socket.take_channel(0) {
        info!("All peers have joined, going in-game");

        // start the GGRS session
        let ggrs_session = session_builder
            .start_p2p_session(channel)
            .expect("failed to start session");

        commands.insert_resource(bevy_ggrs::Session::P2PSession(ggrs_session));
    } else {
        // channel was already taken, hopefully; how do i stop this from running?
    }
}

struct GgrsConfig;

impl ggrs::Config for GgrsConfig {
    // 4-directions + fire fits easily in a single byte
    type Input = GameInput;
    type State = u8;
    // Matchbox' WebRtcSocket addresses are called `PeerId`s
    type Address = PeerId;
}

#[repr(C)]
#[derive(
    Copy,
    Clone,
    PartialEq,
    Pod,
    Zeroable,
    Component,
    Debug,
    Default,
    // Serialize,
    // Deserialize,
    Reflect,
)]
pub struct GameInput {
    pub mouse: Vec2,
    pub has_mouse: u8,
    pub keys: u8,

    // To avoid transmute size errors like this:
    // > error[E0512]: cannot transmute between types of different sizes, or dependently-sized types
    // Set the appropriate amount of padding bytes here, according to the error.
    _padding: [u8; 6],
}

const INPUT_UP: u8 = 1 << 0;
const INPUT_DOWN: u8 = 1 << 1;
const INPUT_LEFT: u8 = 1 << 2;
const INPUT_RIGHT: u8 = 1 << 3;
const INPUT_FIRE: u8 = 1 << 4;

#[derive(Component)]
struct Player {
    handle: usize,
}

fn spawn_players(mut commands: Commands, mut rip: ResMut<RollbackIdProvider>) {
    // Player 1
    commands.spawn((
        Player { handle: 0 },
        rip.next(),
        SpriteBundle {
            transform: Transform::from_translation(Vec3::new(-100., 0., 0.)),
            sprite: Sprite {
                color: Color::BISQUE,
                custom_size: Some(Vec2::new(100., 100.)),
                ..default()
            },
            ..default()
        },
    ));

    // Player 2
    commands.spawn((
        Player { handle: 1 },
        rip.next(),
        SpriteBundle {
            transform: Transform::from_translation(Vec3::new(100., 0., 0.)),
            sprite: Sprite {
                color: Color::BLUE,
                custom_size: Some(Vec2::new(100., 100.)),
                ..default()
            },
            ..default()
        },
    ));
}

fn input(
    _: In<ggrs::PlayerHandle>,
    keys: Res<Input<KeyCode>>,
    window: Query<&Window, With<PrimaryWindow>>,
    camera: Query<(&Camera, &GlobalTransform)>,
) -> GameInput {
    let mut input = GameInput { ..default() };

    let (camera, camera_transform) = camera.single();
    if let Some(pos) = window
        .single()
        .cursor_position()
        .and_then(|cursor| camera.viewport_to_world(camera_transform, cursor))
        .map(|ray| ray.origin.truncate())
    {
        input.mouse = pos;
        input.has_mouse = 1;
        // info!("Mouse @ {}", pos);
    }

    if keys.any_pressed([KeyCode::Up, KeyCode::W]) {
        input.keys |= INPUT_UP;
    }
    if keys.any_pressed([KeyCode::Down, KeyCode::S]) {
        input.keys |= INPUT_DOWN;
    }
    if keys.any_pressed([KeyCode::Left, KeyCode::A]) {
        input.keys |= INPUT_LEFT
    }
    if keys.any_pressed([KeyCode::Right, KeyCode::D]) {
        input.keys |= INPUT_RIGHT;
    }
    if keys.any_pressed([KeyCode::Space, KeyCode::Return]) {
        input.keys |= INPUT_FIRE;
    }

    input
}

fn move_players(
    inputs: Res<PlayerInputs<GgrsConfig>>,
    mut player_query: Query<(&mut Transform, &Player)>,
) {
    for (mut transform, player) in player_query.iter_mut() {
        let (input, _) = inputs[player.handle];

        let mut direction = Vec2::ZERO;

        if input.keys & INPUT_UP != 0 {
            direction.y += 1.;
        }
        if input.keys & INPUT_DOWN != 0 {
            direction.y -= 1.;
        }
        if input.keys & INPUT_RIGHT != 0 {
            direction.x += 1.;
        }
        if input.keys & INPUT_LEFT != 0 {
            direction.x -= 1.;
        }
        // if direction == Vec2::ZERO {
        //     continue;
        // }

        let move_speed = 0.13;
        let _move_delta = (direction * move_speed).extend(0.);

        // transform.translation += move_delta;
        if input.has_mouse == 1 {
            transform.translation.x = input.mouse.x;
            transform.translation.y = input.mouse.y;
        }
    }
}
