use bevy::{
    prelude::*,
    input::mouse::MouseMotion,
    window::{CursorGrabMode, WindowMode, PresentMode, WindowPosition, MonitorSelection},
    diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin},
};
use noise::{NoiseFn, Perlin};
use strum_macros::EnumString;
use rand::random;

#[derive(States, Debug, Clone, Copy, Eq, PartialEq, Hash, Default)]
enum GameState {
    #[default]
    MainMenu,
    Loading,
    Playing,
    Paused,
}

#[derive(Component)]
struct Player {
    yaw: f32,
    pitch: f32,
    last_jump_time: Option<f32>,
}

#[derive(Component)]
struct Velocity(Vec3);

#[derive(Component)]
struct Gravity(f32);

#[derive(Component)]
struct Block;

#[derive(Component)]
struct PauseMenu;

#[derive(Resource)]
struct WorldGenProgress {
    blocks_completed: usize,
    total_blocks: usize,
}

#[derive(Component)]
struct MainMenuUI;

#[derive(Component)]
struct LoadingScreenUI;

#[derive(Component)]
struct Crosshair;

#[derive(Resource)]
struct GameSettings {
    fov: f32,
    show_keystrokes: bool,
    keybinds: KeyBinds,
    currently_binding: Option<KeyBind>,
}

#[derive(Resource)]
struct KeyBinds {
    forward: KeyCode,
    backward: KeyCode,
    left: KeyCode,
    right: KeyCode,
    jump: KeyCode,
    sprint: KeyCode,
    sneak: KeyCode,
}

impl Default for KeyBinds {
    fn default() -> Self {
        Self {
            forward: KeyCode::W,
            backward: KeyCode::S,
            left: KeyCode::A,
            right: KeyCode::D,
            jump: KeyCode::Space,
            sprint: KeyCode::Space,
            sneak: KeyCode::ShiftLeft,
        }
    }
}

#[derive(Component)]
struct FpsText;

#[derive(Component)]
struct KeystrokesDisplay;

#[derive(Debug, Clone, Copy, EnumString)]
enum KeyBind {
    Forward,
    Backward,
    Left,
    Right,
    Jump,
    Sprint,
}

#[derive(Component)]
struct Flight;

const WORLD_SIZE: i32 = 128;
const SPRINT_MULTIPLIER: f32 = 5.0;
const PLAYER_HEIGHT: f32 = 2.0;
const PLAYER_WIDTH: f32 = 0.5;
const PLAYER_JUMP_FORCE: f32 = 10.0;
const PLAYER_BASE_SPEED: f32 = 200.0;
const GRAVITY: f32 = 20.0;
const MOUSE_SENSITIVITY: f32 = 0.002;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                present_mode: PresentMode::AutoVsync,
                mode: WindowMode::Windowed,
                resizable: true,
                resolution: (1920., 1080.).into(),
                title: "Minceraft".to_string(),
                position: WindowPosition::Centered(MonitorSelection::Primary),
                ..default()
            }),
            ..default()
        }))
        .add_plugins(FrameTimeDiagnosticsPlugin::default())
        .add_plugins(LogDiagnosticsPlugin::default())
        .add_state::<GameState>()
        .insert_resource(WorldGenProgress {
            blocks_completed: 0,
            total_blocks: (WORLD_SIZE * WORLD_SIZE) as usize,
        })
        .insert_resource(GameSettings {
            fov: 100.0,
            show_keystrokes: true,
            keybinds: load_settings().unwrap_or_default(),
            currently_binding: None,
        })
        .add_systems(Startup, (setup, spawn_fps_counter))
        .add_systems(Update, (
            main_menu.run_if(in_state(GameState::MainMenu)),
            loading_screen.run_if(in_state(GameState::Loading)),
            player_control.run_if(in_state(GameState::Playing)),
            physics_system.run_if(in_state(GameState::Playing)),
            toggle_pause,
        ))
        .add_systems(Update, (
            pause_menu,
            adjust_fov,
        ).run_if(in_state(GameState::Paused)))
        .add_systems(Update, update_fps_text)
        .add_systems(Update, update_window_title)
        .add_systems(OnEnter(GameState::Loading), cleanup_main_menu)
        .add_systems(OnEnter(GameState::Playing), (
            cleanup_loading_screen,
            cleanup_pause_menu,
            spawn_crosshair
        ))
        .add_systems(Update, keystrokes_display.run_if(in_state(GameState::Playing)))
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut windows: Query<&mut Window>,
    settings: Res<GameSettings>,
) {
    let mut window = windows.single_mut();
    window.cursor.visible = false;
    window.cursor.grab_mode = CursorGrabMode::Locked;

    let cube_mesh = meshes.add(Mesh::from(shape::Cube { size: 1.0 }));
    let dirt_material = materials.add(Color::rgb(0.5, 0.3, 0.2).into());
    let grass_material = materials.add(Color::rgb(0.3, 0.5, 0.3).into());
    
    let seed = random::<u32>();
    let perlin = Perlin::new(seed as u32);
    
    for x in -10..10 {
        for z in -10..10 {
            let px = x as f64 * 0.1;
            let pz = z as f64 * 0.1;
            let height = (perlin.get([px, pz]) * 5.0).max(0.0) as i32;
            
            for y in -5..=height {
                commands.spawn((
                    PbrBundle {
                        mesh: cube_mesh.clone(),
                        material: if y == height { grass_material.clone() } else { dirt_material.clone() },
                        transform: Transform::from_xyz(x as f32, y as f32, z as f32),
                        ..default()
                    },
                    Block,
                ));
            }
        }
    }

    commands.spawn((
        Camera3dBundle {
            transform: Transform::from_xyz(0.0, 15.0, 0.0),
            projection: Projection::Perspective(PerspectiveProjection {
                fov: settings.fov.to_radians(),
                ..default()
            }),
            ..default()
        },
        Player {
            yaw: 0.0,
            pitch: 0.0,
            last_jump_time: None,
        },
        Velocity(Vec3::ZERO),
        Gravity(GRAVITY),
    ));

    commands.spawn(DirectionalLightBundle {
        directional_light: DirectionalLight {
            shadows_enabled: true,
            illuminance: 50000.0,
            ..default()
        },
        transform: Transform::from_xyz(4.0, 8.0, 4.0)
            .looking_at(Vec3::ZERO, Vec3::Y),
        ..default()
    });

    commands.insert_resource(AmbientLight {
        color: Color::WHITE,
        brightness: 0.3,
    });
}

fn player_control(
    mut commands: Commands,
    mut query: Query<(Entity, &mut Transform, &mut Player, &mut Velocity)>,
    keyboard_input: Res<Input<KeyCode>>,
    mut motion_evr: EventReader<MouseMotion>,
    time: Res<Time>,
    settings: Res<GameSettings>,
    flight_query: Query<(), With<Flight>>,
) {
    let (entity, mut transform, mut player, mut velocity) = query.single_mut();
    
    if keyboard_input.just_pressed(settings.keybinds.jump) {
        let current_time = time.elapsed_seconds();
        if let Some(last_time) = player.last_jump_time {
            if current_time - last_time < 0.3 {
                if flight_query.is_empty() {
                    commands.entity(entity).insert(Flight);
                    velocity.0.y = 0.0;
                } else {
                    commands.entity(entity).remove::<Flight>();
                }
            }
        }
        player.last_jump_time = Some(current_time);
    }

    for ev in motion_evr.read() {
        player.pitch -= ev.delta.y * MOUSE_SENSITIVITY;
        player.yaw -= ev.delta.x * MOUSE_SENSITIVITY;
    }

    player.pitch = player.pitch.clamp(-1.5, 1.5);
    transform.rotation = Quat::from_euler(EulerRot::YXZ, player.yaw, player.pitch, 0.0);

    let sprint_multiplier = if keyboard_input.pressed(settings.keybinds.sprint) { SPRINT_MULTIPLIER } else { 1.0 };
    let speed = PLAYER_BASE_SPEED * sprint_multiplier;
    
    let forward = -transform.forward();
    let right = transform.right();

    let forward = Vec3::new(forward.x, 0.0, forward.z).normalize();
    let right = Vec3::new(right.x, 0.0, right.z).normalize();

    let mut movement = match (
        keyboard_input.pressed(settings.keybinds.forward),
        keyboard_input.pressed(settings.keybinds.backward),
        keyboard_input.pressed(settings.keybinds.right), 
        keyboard_input.pressed(settings.keybinds.left)
    ) {
        (true, false, false, false) => -forward,
        (false, true, false, false) => forward,
        (false, false, true, false) => right,
        (false, false, false, true) => -right,
        (true, false, true, false) => (-forward + right).normalize(),
        (true, false, false, true) => (-forward - right).normalize(),
        (false, true, true, false) => (forward + right).normalize(),
        (false, true, false, true) => (forward - right).normalize(),
        _ => Vec3::ZERO,
    };

    if flight_query.contains(entity) {
        if keyboard_input.pressed(settings.keybinds.jump) {
            velocity.0.y = PLAYER_BASE_SPEED * time.delta_seconds();
        } else if keyboard_input.pressed(KeyCode::ShiftLeft) {
            velocity.0.y = -PLAYER_BASE_SPEED * time.delta_seconds();
        } else {
            velocity.0.y = 0.0;
        }
    } else if keyboard_input.just_pressed(settings.keybinds.jump) && velocity.0.y.abs() < 0.1 {
        velocity.0.y = PLAYER_JUMP_FORCE;
    }

    if movement != Vec3::ZERO {
        movement = movement.normalize() * speed * time.delta_seconds();
        velocity.0.x = movement.x;
        velocity.0.z = movement.z;
    } else {
        velocity.0.x = 0.0;
        velocity.0.z = 0.0;
    }

    if keyboard_input.pressed(settings.keybinds.sprint) {
        velocity.0.y = 10.0;
    }
    
    if keyboard_input.pressed(settings.keybinds.sneak) {
        velocity.0.y = -10.0;
    }
}

fn physics_system(
    time: Res<Time>,
    mut player_query: Query<(&mut Transform, &mut Velocity, &Gravity), With<Player>>,
    blocks: Query<&Transform, (With<Block>, Without<Player>)>,
    flight_query: Query<(), With<Flight>>,
) {
    let (mut player_transform, mut velocity, gravity) = player_query.single_mut();
    let dt = time.delta_seconds();

    if flight_query.is_empty() {
        velocity.0.y -= gravity.0 * dt;
    }

    let mut new_pos = player_transform.translation + velocity.0 * dt;

    for block_transform in &blocks {
        let block_pos = block_transform.translation;
        let diff = new_pos - block_pos;

        let player_size = Vec3::new(PLAYER_WIDTH, PLAYER_HEIGHT, PLAYER_WIDTH);
        let block_size = Vec3::new(1.0, 1.0, 1.0);
        
        let min_dist = (player_size + block_size) * 0.5;
        
        if diff.x.abs() < min_dist.x && diff.y.abs() < min_dist.y && diff.z.abs() < min_dist.z {
            if diff.y.abs() > diff.x.abs() && diff.y.abs() > diff.z.abs() {
                if diff.y > 0.0 {
                    new_pos.y = block_pos.y + min_dist.y;
                    velocity.0.y = 0.0;
                } else {
                    new_pos.y = block_pos.y - min_dist.y;
                    velocity.0.y = 0.0;
                }
            }
            else if diff.x.abs() > diff.z.abs() {
                new_pos.x = block_pos.x + min_dist.x * diff.x.signum();
                velocity.0.x = 0.0;
            } else {
                new_pos.z = block_pos.z + min_dist.z * diff.z.signum();
                velocity.0.z = 0.0;
            }
        }
    }

    player_transform.translation = new_pos;
}

fn toggle_pause(
    mut next_state: ResMut<NextState<GameState>>,
    current_state: Res<State<GameState>>,
    keyboard: Res<Input<KeyCode>>,
    mut windows: Query<&mut Window>,
    mut commands: Commands,
    blocks: Query<Entity, With<Block>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    if keyboard.pressed(KeyCode::ControlLeft) && keyboard.just_pressed(KeyCode::R) {
        for entity in blocks.iter() {
            commands.entity(entity).despawn();
        }
        
        let cube_mesh = meshes.add(Mesh::from(shape::Cube { size: 1.0 }));
        let dirt_material = materials.add(Color::rgb(0.5, 0.3, 0.2).into());
        let grass_material = materials.add(Color::rgb(0.3, 0.5, 0.3).into());
        
        let seed = random::<u32>();
        let perlin = Perlin::new(seed);
        
        let size = WORLD_SIZE / 2;
        for x in -size..size {
            for z in -size..size {
                let px = x as f64 * 0.1;
                let pz = z as f64 * 0.1;
                let height = (perlin.get([px, pz]) * 5.0).max(0.0) as i32;
                
                for y in -5..=height {
                    commands.spawn((
                        PbrBundle {
                            mesh: cube_mesh.clone(),
                            material: if y == height { grass_material.clone() } else { dirt_material.clone() },
                            transform: Transform::from_xyz(x as f32, y as f32, z as f32),
                            ..default()
                        },
                        Block,
                    ));
                }
            }
        }
        return;
    }

    if keyboard.just_pressed(KeyCode::Escape) {
        let mut window = windows.single_mut();
        match current_state.get() {
            GameState::Playing => {
                window.cursor.visible = true;
                window.cursor.grab_mode = CursorGrabMode::None;
                next_state.set(GameState::Paused);
            }
            GameState::Paused => {
                window.cursor.visible = false;
                window.cursor.grab_mode = CursorGrabMode::Locked;
                next_state.set(GameState::Playing);
            }
            GameState::MainMenu | GameState::Loading => {}
        }
    }
}

fn pause_menu(
    mut commands: Commands,
    existing_menu: Query<Entity, With<PauseMenu>>,
    mut settings: ResMut<GameSettings>,
    keyboard: Res<Input<KeyCode>>,
) {
    for entity in existing_menu.iter() {
        commands.entity(entity).despawn_recursive();
    }

    if keyboard.just_pressed(KeyCode::K) {
        settings.show_keystrokes = !settings.show_keystrokes;
    }

    if let Some(binding) = settings.currently_binding {
        for key in keyboard.get_just_pressed() {
            match binding {
                KeyBind::Forward => settings.keybinds.forward = *key,
                KeyBind::Backward => settings.keybinds.backward = *key,
                KeyBind::Left => settings.keybinds.left = *key,
                KeyBind::Right => settings.keybinds.right = *key,
                KeyBind::Jump => settings.keybinds.jump = *key,
                KeyBind::Sprint => settings.keybinds.sprint = *key,
            }
            settings.currently_binding = None;
            save_settings(&settings).unwrap_or_else(|e| eprintln!("Failed to save settings: {}", e));
            return;
        }
    }

    if keyboard.just_pressed(KeyCode::Key1) {
        settings.currently_binding = Some(KeyBind::Forward);
    } else if keyboard.just_pressed(KeyCode::Key2) {
        settings.currently_binding = Some(KeyBind::Backward);
    } else if keyboard.just_pressed(KeyCode::Key3) {
        settings.currently_binding = Some(KeyBind::Left);
    } else if keyboard.just_pressed(KeyCode::Key4) {
        settings.currently_binding = Some(KeyBind::Right);
    } else if keyboard.just_pressed(KeyCode::Key5) {
        settings.currently_binding = Some(KeyBind::Jump);
    } else if keyboard.just_pressed(KeyCode::Key6) {
        settings.currently_binding = Some(KeyBind::Sprint);
    }

    commands
        .spawn((
            NodeBundle {
                style: Style {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(20.0),
                    ..default()
                },
                background_color: Color::rgba(0.0, 0.0, 0.0, 0.7).into(),
                ..default()
            },
            PauseMenu,
        ))
        .with_children(|parent| {
            parent.spawn(TextBundle::from_section(
                "PAUSED",
                TextStyle {
                    font_size: 40.0,
                    color: Color::WHITE,
                    ..default()
                },
            ));

            parent.spawn(TextBundle::from_section(
                "Press ESC to resume",
                TextStyle {
                    font_size: 20.0,
                    color: Color::WHITE,
                    ..default()
                },
            ));

            parent.spawn(TextBundle::from_section(
                "Press UP/DOWN to adjust FOV",
                TextStyle {
                    font_size: 20.0,
                    color: Color::WHITE,
                    ..default()
                },
            ));

            parent.spawn(TextBundle::from_section(
                format!("Press K to {} keystrokes ({})", 
                    if settings.show_keystrokes { "hide" } else { "show" },
                    if settings.show_keystrokes { "ON" } else { "OFF" }
                ),
                TextStyle {
                    font_size: 20.0,
                    color: Color::WHITE,
                    ..default()
                },
            ));

            parent.spawn(TextBundle::from_section(
                format!("Forward: {:?}", settings.keybinds.forward),
                TextStyle {
                    font_size: 20.0,
                    color: Color::WHITE,
                    ..default()
                },
            ));

            parent.spawn(TextBundle::from_section(
                format!("Backward: {:?}", settings.keybinds.backward),
                TextStyle {
                    font_size: 20.0,
                    color: Color::WHITE,
                    ..default()
                },
            ));

            parent.spawn(TextBundle::from_section(
                format!("Left: {:?}", settings.keybinds.left),
                TextStyle {
                    font_size: 20.0,
                    color: Color::WHITE,
                    ..default()
                },
            ));

            parent.spawn(TextBundle::from_section(
                format!("Right: {:?}", settings.keybinds.right),
                TextStyle {
                    font_size: 20.0,
                    color: Color::WHITE,
                    ..default()
                },
            ));

            parent.spawn(TextBundle::from_section(
                format!("Jump: {:?}", settings.keybinds.jump),
                TextStyle {
                    font_size: 20.0,
                    color: Color::WHITE,
                    ..default()
                },
            ));

            parent.spawn(TextBundle::from_section(
                format!("Sprint: {:?}", settings.keybinds.sprint),
                TextStyle {
                    font_size: 20.0,
                    color: Color::WHITE,
                    ..default()
                },
            ));

            parent.spawn(TextBundle::from_section(
                format!("Press 1-6 to change binds:"),
                TextStyle {
                    font_size: 20.0,
                    color: Color::WHITE,
                    ..default()
                },
            ));
        });
}

fn main_menu(
    mut commands: Commands,
    mut next_state: ResMut<NextState<GameState>>,
    mut windows: Query<&mut Window>,
    interaction_query: Query<&Interaction, (Changed<Interaction>, With<Button>)>,
) {
    let mut window = windows.single_mut();
    window.cursor.visible = true;
    window.cursor.grab_mode = CursorGrabMode::None;

    for interaction in interaction_query.iter() {
        if *interaction == Interaction::Pressed {
            next_state.set(GameState::Loading);
            return;
        }
    }

    commands
        .spawn((
            NodeBundle {
                style: Style {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(20.0),
                    ..default()
                },
                ..default()
            },
            MainMenuUI,
        ))
        .with_children(|parent| {
            parent.spawn(TextBundle::from_section(
                "MINCERAFT",
                TextStyle {
                    font_size: 80.0,
                    color: Color::GREEN,
                    ..default()
                },
            ));

            parent.spawn(ButtonBundle {
                style: Style {
                    padding: UiRect::all(Val::Px(20.0)),
                    ..default()
                },
                background_color: Color::rgb(0.2, 0.2, 0.2).into(),
                ..default()
            })
            .with_children(|parent| {
                parent.spawn(TextBundle::from_section(
                    "Play",
                    TextStyle {
                        font_size: 30.0,
                        color: Color::WHITE,
                        ..default()
                    },
                ));
            });
        });
}

fn cleanup_main_menu(
    mut commands: Commands,
    menu_query: Query<Entity, With<MainMenuUI>>,
) {
    for entity in menu_query.iter() {
        commands.entity(entity).despawn_recursive();
    }
}

fn cleanup_loading_screen(
    mut commands: Commands,
    loading_query: Query<Entity, With<LoadingScreenUI>>,
) {
    for entity in loading_query.iter() {
        commands.entity(entity).despawn_recursive();
    }
}

fn cleanup_pause_menu(
    mut commands: Commands,
    pause_menu_query: Query<Entity, With<PauseMenu>>,
) {
    for entity in pause_menu_query.iter() {
        commands.entity(entity).despawn_recursive();
    }
}

fn loading_screen(
    mut commands: Commands,
    mut next_state: ResMut<NextState<GameState>>,
    mut progress: ResMut<WorldGenProgress>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    loading_query: Query<Entity, With<LoadingScreenUI>>,
) {
    for entity in loading_query.iter() {
        commands.entity(entity).despawn_recursive();
    }

    let cube_mesh = meshes.add(Mesh::from(shape::Cube { size: 1.0 }));
    let dirt_material = materials.add(Color::rgb(0.5, 0.3, 0.2).into());
    let grass_material = materials.add(Color::rgb(0.3, 0.5, 0.3).into());
    let perlin = Perlin::new(42);

    let size = WORLD_SIZE / 2;
    let blocks_per_frame = 100;

    for _ in 0..blocks_per_frame {
        if progress.blocks_completed >= progress.total_blocks {
            next_state.set(GameState::Playing);
            return;
        }

        let x = -size + (progress.blocks_completed as i32 % WORLD_SIZE);
        let z = -size + (progress.blocks_completed as i32 / WORLD_SIZE);

        let px = x as f64 * 0.1;
        let pz = z as f64 * 0.1;
        let height = (perlin.get([px, pz]) * 5.0).max(0.0) as i32;

        for y in -5..=height {
            commands.spawn((
                PbrBundle {
                    mesh: cube_mesh.clone(),
                    material: if y == height { grass_material.clone() } else { dirt_material.clone() },
                    transform: Transform::from_xyz(x as f32, y as f32, z as f32),
                    ..default()
                },
                Block,
            ));
        }

        progress.blocks_completed += 1;
    }

    let percentage = (progress.blocks_completed as f32 / progress.total_blocks as f32 * 100.0) as i32;
    
    commands.spawn((
        NodeBundle {
            style: Style {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            ..default()
        },
        LoadingScreenUI,
    ))
    .with_children(|parent| {
        parent.spawn(TextBundle::from_section(
            format!("Generating Terrain: {}%", percentage),
            TextStyle {
                font_size: 40.0,
                color: Color::WHITE,
                ..default()
            },
        ));
    });
}

fn spawn_crosshair(mut commands: Commands) {
    commands
        .spawn((
            NodeBundle {
                style: Style {
                    width: Val::Percent(100.0),
                    height: Val::Percent(100.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    position_type: PositionType::Absolute,
                    ..default()
                },
                ..default()
            },
            Crosshair,
        ))
        .with_children(|parent| {
            parent
                .spawn(NodeBundle {
                    style: Style {
                        width: Val::Px(20.0),
                        height: Val::Px(20.0),
                        position_type: PositionType::Absolute,
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    ..default()
                })
                .with_children(|parent| {
                    parent.spawn(NodeBundle {
                        style: Style {
                            width: Val::Px(2.0),
                            height: Val::Px(20.0),
                            ..default()
                        },
                        background_color: Color::WHITE.into(),
                        ..default()
                    });
                    parent.spawn(NodeBundle {
                        style: Style {
                            width: Val::Px(20.0),
                            height: Val::Px(2.0),
                            ..default()
                        },
                        background_color: Color::WHITE.into(),
                        ..default()
                    });
                });
        });
}

fn adjust_fov(
    keyboard_input: Res<Input<KeyCode>>,
    mut settings: ResMut<GameSettings>,
    mut query: Query<&mut Projection, With<Camera3d>>,
) {
    let mut changed = false;
    
    if keyboard_input.pressed(KeyCode::Up) {
        settings.fov = (settings.fov + 1.0).min(140.0);
        changed = true;
    }
    if keyboard_input.pressed(KeyCode::Down) {
        settings.fov = (settings.fov - 1.0).max(30.0);
        changed = true;
    }

    if changed {
        if let Ok(mut projection) = query.get_single_mut() {
            if let Projection::Perspective(ref mut perspective) = *projection {
                perspective.fov = settings.fov.to_radians();
            }
        }
    }
}

fn spawn_fps_counter(mut commands: Commands) {
    commands.spawn((
        TextBundle::from_section(
            "FPS: ",
            TextStyle {
                font_size: 20.0,
                color: Color::WHITE,
                ..default()
            },
        )
        .with_style(Style {
            position_type: PositionType::Absolute,
            top: Val::Px(10.0),
            left: Val::Px(10.0),
            ..default()
        }),
        FpsText,
    ));
}

fn update_fps_text(
    diagnostics: Res<bevy::diagnostic::DiagnosticsStore>,
    mut query: Query<&mut Text, With<FpsText>>,
) {
    if let Some(fps) = diagnostics.get(FrameTimeDiagnosticsPlugin::FPS) {
        if let Some(value) = fps.smoothed() {
            if let Ok(mut text) = query.get_single_mut() {
                text.sections[0].value = format!("FPS: {value:.0}");
            }
        }
    }
}

fn keystrokes_display(
    mut commands: Commands,
    keyboard: Res<Input<KeyCode>>,
    settings: Res<GameSettings>,
    existing_display: Query<Entity, With<KeystrokesDisplay>>,
) {
    for entity in existing_display.iter() {
        commands.entity(entity).despawn_recursive();
    }

    if !settings.show_keystrokes {
        return;
    }

    commands
        .spawn((
            NodeBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    right: Val::Px(20.0),
                    top: Val::Px(20.0),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(2.0),
                    ..default()
                },
                ..default()
            },
            KeystrokesDisplay,
        ))
        .with_children(|parent| {
            // W key
            parent.spawn(NodeBundle {
                style: Style {
                    width: Val::Px(40.0),
                    height: Val::Px(40.0),
                    border: UiRect::all(Val::Px(2.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    margin: UiRect::new(Val::Px(42.0), Val::Px(0.0), Val::Px(0.0), Val::Px(0.0)),
                    ..default()
                },
                border_color: Color::WHITE.into(),
                background_color: if keyboard.pressed(settings.keybinds.forward) {
                    Color::rgb(0.5, 0.5, 0.5)
                } else {
                    Color::rgba(0.0, 0.0, 0.0, 0.5)
                }.into(),
                ..default()
            }).with_children(|parent| {
                parent.spawn(TextBundle::from_section(
                    format!("{:?}", settings.keybinds.forward),
                    TextStyle {
                        font_size: 20.0,
                        color: Color::WHITE,
                        ..default()
                    },
                ));
            });

            // ASD row
            parent.spawn(NodeBundle {
                style: Style {
                    flex_direction: FlexDirection::Row,
                    column_gap: Val::Px(2.0),
                    ..default()
                },
                ..default()
            })
            .with_children(|parent| {
                for (key, _) in [
                    (settings.keybinds.left, "Left"),
                    (settings.keybinds.backward, "Back"),
                    (settings.keybinds.right, "Right")
                ] {
                    parent.spawn(NodeBundle {
                        style: Style {
                            width: Val::Px(40.0),
                            height: Val::Px(40.0),
                            border: UiRect::all(Val::Px(2.0)),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        border_color: Color::WHITE.into(),
                        background_color: if keyboard.pressed(key) {
                            Color::rgb(0.5, 0.5, 0.5)
                        } else {
                            Color::rgba(0.0, 0.0, 0.0, 0.5)
                        }.into(),
                        ..default()
                    }).with_children(|parent| {
                        parent.spawn(TextBundle::from_section(
                            format!("{:?}", key),
                            TextStyle {
                                font_size: 20.0,
                                color: Color::WHITE,
                                ..default()
                            },
                        ));
                    });
                }
            });

            // Space bar
            parent.spawn(NodeBundle {
                style: Style {
                    width: Val::Px(124.0),
                    height: Val::Px(40.0),
                    border: UiRect::all(Val::Px(2.0)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                border_color: Color::WHITE.into(),
                background_color: if keyboard.pressed(settings.keybinds.jump) {
                    Color::rgb(0.5, 0.5, 0.5)
                } else {
                    Color::rgba(0.0, 0.0, 0.0, 0.5)
                }.into(),
                ..default()
            }).with_children(|parent| {
                parent.spawn(TextBundle::from_section(
                    format!("{:?}", settings.keybinds.jump),
                    TextStyle {
                        font_size: 20.0,
                        color: Color::WHITE,
                        ..default()
                    },
                ));
            });
        });
}

fn save_settings(settings: &GameSettings) -> std::io::Result<()> {
    let mut content = String::new();
    content.push_str(&format!("forward={:?}\n", settings.keybinds.forward));
    content.push_str(&format!("backwards={:?}\n", settings.keybinds.backward));
    content.push_str(&format!("strafe_left={:?}\n", settings.keybinds.left));
    content.push_str(&format!("strafe_right={:?}\n", settings.keybinds.right));
    content.push_str(&format!("jump={:?}\n", settings.keybinds.jump));
    content.push_str(&format!("sprint={:?}\n", settings.keybinds.sprint));

    std::fs::create_dir_all("assets")?;
    std::fs::write("assets/options.txt", content)
}

fn load_settings() -> Option<KeyBinds> {
    let content = std::fs::read_to_string("assets/options.txt").ok()?;
    let mut keybinds = KeyBinds::default();

    for line in content.lines() {
        let mut parts = line.split('=');
        let key = parts.next()?;
        let value = parts.next()?;
        let keycode = match value.trim() {
            "KeyCode::A" => bevy::prelude::KeyCode::A,
            "KeyCode::B" => bevy::prelude::KeyCode::B,
            "KeyCode::C" => bevy::prelude::KeyCode::C,
            "KeyCode::D" => bevy::prelude::KeyCode::D,
            "KeyCode::E" => bevy::prelude::KeyCode::E,
            "KeyCode::F" => bevy::prelude::KeyCode::F,
            "KeyCode::G" => bevy::prelude::KeyCode::G,
            "KeyCode::H" => bevy::prelude::KeyCode::H,
            "KeyCode::I" => bevy::prelude::KeyCode::I,
            "KeyCode::J" => bevy::prelude::KeyCode::J,
            "KeyCode::K" => bevy::prelude::KeyCode::K,
            "KeyCode::L" => bevy::prelude::KeyCode::L,
            "KeyCode::M" => bevy::prelude::KeyCode::M,
            "KeyCode::N" => bevy::prelude::KeyCode::N,
            "KeyCode::O" => bevy::prelude::KeyCode::O,
            "KeyCode::P" => bevy::prelude::KeyCode::P,
            "KeyCode::Q" => bevy::prelude::KeyCode::Q,
            "KeyCode::R" => bevy::prelude::KeyCode::R,
            "KeyCode::S" => bevy::prelude::KeyCode::S,
            "KeyCode::T" => bevy::prelude::KeyCode::T,
            "KeyCode::U" => bevy::prelude::KeyCode::U,
            "KeyCode::V" => bevy::prelude::KeyCode::V,
            "KeyCode::W" => bevy::prelude::KeyCode::W,
            "KeyCode::X" => bevy::prelude::KeyCode::X,
            "KeyCode::Y" => bevy::prelude::KeyCode::Y,
            "KeyCode::Z" => bevy::prelude::KeyCode::Z,
            "KeyCode::Space" => bevy::prelude::KeyCode::Space,
            "KeyCode::LeftShift" => bevy::prelude::KeyCode::ShiftLeft,
            _ => continue,
        };
        match key {
            "forward" => keybinds.forward = keycode,
            "backwards" => keybinds.backward = keycode,
            "strafe_left" => keybinds.left = keycode,
            "strafe_right" => keybinds.right = keycode,
            "jump" => keybinds.jump = keycode,
            "sprint" => keybinds.sprint = keycode,
            _ => {}
        }
    }

    Some(keybinds)
}

fn update_window_title(
    mut windows: Query<&mut Window>,
    state: Res<State<GameState>>,
) {
    let mut window = windows.single_mut();
    let state_text = match state.get() {
        GameState::MainMenu => "Main Menu",
        GameState::Loading => "Loading",
        GameState::Playing => "In Game",
        GameState::Paused => "Paused",
    };
    window.title = format!("Minceraft - {}", state_text);
}
