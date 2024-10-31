use bevy::{
    prelude::*,
    input::mouse::MouseMotion,
    window::CursorGrabMode,
};
use noise::{NoiseFn, Perlin};

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

const WORLD_SIZE: i32 = 2;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_state::<GameState>()
        .insert_resource(WorldGenProgress {
            blocks_completed: 0,
            total_blocks: (WORLD_SIZE * WORLD_SIZE) as usize,
        })
        .add_systems(Startup, setup)
        .add_systems(Update, (
            main_menu.run_if(in_state(GameState::MainMenu)),
            loading_screen.run_if(in_state(GameState::Loading)),
            player_control.run_if(in_state(GameState::Playing)),
            physics_system.run_if(in_state(GameState::Playing)),
            toggle_pause,
            pause_menu.run_if(in_state(GameState::Paused))
        ))
        .add_systems(OnEnter(GameState::Loading), cleanup_main_menu)
        .add_systems(OnEnter(GameState::Playing), (cleanup_loading_screen, cleanup_pause_menu, spawn_crosshair))
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut windows: Query<&mut Window>,
) {
    let mut window = windows.single_mut();
    window.cursor.visible = false;
    window.cursor.grab_mode = CursorGrabMode::Locked;

    let cube_mesh = meshes.add(Mesh::from(shape::Cube { size: 1.0 }));
    let dirt_material = materials.add(Color::rgb(0.5, 0.3, 0.2).into());
    let grass_material = materials.add(Color::rgb(0.3, 0.5, 0.3).into());
    
    let perlin = Perlin::new(42);
    
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
            ..default()
        },
        Player {
            yaw: 0.0,
            pitch: 0.0,
        },
        Velocity(Vec3::ZERO),
        Gravity(20.0),
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
    mut query: Query<(&mut Transform, &mut Player, &mut Velocity)>,
    keyboard_input: Res<Input<KeyCode>>,
    mut motion_evr: EventReader<MouseMotion>,
    time: Res<Time>,
) {
    let (mut transform, mut player, mut velocity) = query.single_mut();
    
    const SENSITIVITY: f32 = 0.002;
    for ev in motion_evr.read() {
        player.pitch -= ev.delta.y * SENSITIVITY;
        player.yaw -= ev.delta.x * SENSITIVITY;
    }

    player.pitch = player.pitch.clamp(-1.5, 1.5);
    transform.rotation = Quat::from_euler(EulerRot::YXZ, player.yaw, player.pitch, 0.0);

    let speed = 200.0;
    let forward = -transform.forward();
    let right = transform.right();

    let forward = Vec3::new(forward.x, 0.0, forward.z).normalize();
    let right = Vec3::new(right.x, 0.0, right.z).normalize();

    let mut movement = Vec3::ZERO;

    if keyboard_input.pressed(KeyCode::W) {
        movement -= forward;
    }
    if keyboard_input.pressed(KeyCode::S) {
        movement += forward;
    }
    if keyboard_input.pressed(KeyCode::D) {
        movement += right;
    }
    if keyboard_input.pressed(KeyCode::A) {
        movement -= right;
    }

    if keyboard_input.just_pressed(KeyCode::Space) && velocity.0.y.abs() < 0.1 {
        velocity.0.y = 10.0;
    }

    if movement != Vec3::ZERO {
        movement = movement.normalize() * speed * time.delta_seconds();
        velocity.0.x = movement.x;
        velocity.0.z = movement.z;
    } else {
        velocity.0.x = 0.0;
        velocity.0.z = 0.0;
    }
}

fn physics_system(
    time: Res<Time>,
    mut player_query: Query<(&mut Transform, &mut Velocity, &Gravity), With<Player>>,
    blocks: Query<&Transform, (With<Block>, Without<Player>)>,
) {
    let (mut player_transform, mut velocity, gravity) = player_query.single_mut();
    let dt = time.delta_seconds();

    velocity.0.y -= gravity.0 * dt;

    let mut new_pos = player_transform.translation + velocity.0 * dt;

    for block_transform in &blocks {
        let block_pos = block_transform.translation;
        let diff = new_pos - block_pos;

        let player_size = Vec3::new(0.5, 2.0, 0.5);
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
) {
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
) {
    for entity in existing_menu.iter() {
        commands.entity(entity).despawn_recursive();
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