use std::collections::VecDeque;
use std::f32::consts::PI;
use std::time::Duration;

use bevy::math::Vec3Swizzles;
use bevy::prelude::*;
use bevy::time::Stopwatch;
use bevy::window::{PrimaryWindow, WindowResolution};
use bevy::diagnostic::{FrameTimeDiagnosticsPlugin, LogDiagnosticsPlugin};

use bevy_xpbd_2d::prelude::*;

const WINDOW_SIZE: Vec2 = Vec2 { x: 1280., y: 720. };

const PADDLE_SIZE: Vec2 = Vec2 { x: 15., y: 60. };

const P1_LEFT_BOUND: f32 = -(-PADDLE_SIZE.x + WINDOW_SIZE.x * 0.5);
const P1_RIGHT_BOUND: f32 = -(PADDLE_SIZE.x + WINDOW_SIZE.x * 0.25);
const P1_TOP_BOUND: f32 = PADDLE_SIZE.y + WINDOW_SIZE.y * 0.5;
const P1_BOTTOM_BOUND: f32 = -(PADDLE_SIZE.y + WINDOW_SIZE.y * 0.5);
const LEFT_WALL: f32 = -(WINDOW_SIZE.x * 0.5);
const RIGHT_WALL: f32 = -(WINDOW_SIZE.x * 0.25);
const TOP_WALL: f32 = WINDOW_SIZE.y * 0.5;
const BOTTOM_WALL: f32 = -(WINDOW_SIZE.y * 0.5);

const BALL_RADIUS: f32 = 15.;

const INITIAL_FORCE: f32 = 20000000.;
const PADDLE_SPEED: f32 = 5000.;
const PADDLE_SPEED_AI: f32 = 500.;

#[derive(Resource, Default)]
struct Score {
    first_player: usize,
    second_player: usize,
}

#[derive(Component, Debug)]
struct Paddle {
    first_player: bool,
    side: Side,
}

#[derive(Component)]
struct Goal {
    first_player: bool,
    side: Side,
}

#[derive(Component)]
struct Ball {
    points: usize,
}

impl Ball {
    fn get_radius(&self) -> f32 {
        if self.points > 0 {
            BALL_RADIUS * 0.85
        } else {
            BALL_RADIUS
        }
    }
}

#[derive(Component)]
struct BallTextMarker;
#[derive(Component)]
struct Player1ScoreMarker;
#[derive(Component)]
struct Player2ScoreMarker;

#[derive(Component)]
struct DelayedExternalForce(pub ExternalForce);


#[derive(Resource, Default)]
struct PointBallCount(u8);

#[derive(Resource)]
struct BallAssets {
    point_ball: Handle<Image>,
}

#[derive(PartialEq, Eq, Clone, Debug)]
enum Side {
    Random,
    Left,
    Right,
}

impl Side {
    fn opposite(&self) -> Self {
        match self {
            Self::Left => Self::Right,
            Self::Right => Self::Left,
            _ => Self::Random,
        }
    }
}

#[derive(PhysicsLayer)]
enum Layer {
    Wall,
    Net,
    Paddle,
    Ball,
}

fn main() {
    let mut app = App::new();

    #[cfg(feature = "fps")]
    app.add_plugins((LogDiagnosticsPlugin::default(), FrameTimeDiagnosticsPlugin::default()));

    app
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    resolution: WindowResolution::new(WINDOW_SIZE.x, WINDOW_SIZE.y),
                    ..default()
                }),
                ..default()
            }),
            PhysicsPlugins::default(),
        ))
        .insert_resource(Gravity::ZERO)
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                apply_delayed_external_forces,
                (
                    check_goals,
                    spawn_ball,
                )
                    .chain(),
                move_paddle,
                update_score.run_if(resource_changed::<Score>()),
            ),
        )
        .run();
}

fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    // Assets
    commands.insert_resource(BallAssets {
        point_ball: assets.load("ball_blue_large.png"),
    });

    // Spawner
    commands.init_resource::<PointBallCount>();

    // Score
    commands.init_resource::<Score>();

    // Camera
    commands.spawn(Camera2dBundle { ..default() });

    // Net (middle line)
    let net_box = Vec2::new(5., WINDOW_SIZE.y);
    commands.spawn((
        SpriteBundle {
            sprite: Sprite {
                custom_size: Some(net_box),
                color: Color::WHITE,
                ..default()
            },
            ..default()
        },
        RigidBody::Static,
        Collider::cuboid(net_box.x, net_box.y),
        CollisionLayers::new([Layer::Net], [Layer::Paddle]),
    ));

    // Walls
    // Up wall
    spawn_wall(
        &mut commands,
        0.,
        WINDOW_SIZE.y / 2.,
        Vec2::NEG_Y,
        CollisionLayers::new([Layer::Wall], [Layer::Paddle, Layer::Ball]),
        false,
    );
    // Down wall
    spawn_wall(
        &mut commands,
        0.,
        -WINDOW_SIZE.y / 2.,
        Vec2::Y,
        CollisionLayers::new([Layer::Wall], [Layer::Paddle, Layer::Ball]),
        false,
    );
    // Left wall
    spawn_wall(
        &mut commands,
        -WINDOW_SIZE.x / 2.,
        0.,
        Vec2::X,
        CollisionLayers::new([Layer::Wall], [Layer::Paddle]),
        false,
    );
    // Right wall
    spawn_wall(
        &mut commands,
        WINDOW_SIZE.x / 2.,
        0.,
        Vec2::NEG_X,
        CollisionLayers::new([Layer::Wall], [Layer::Paddle]),
        false,
    );
    // Left goal
    spawn_wall(
        &mut commands,
        -(WINDOW_SIZE.x / 2. + 5.),
        0.,
        Vec2::X,
        CollisionLayers::new([Layer::Wall], [Layer::Ball]),
        true,
    );
    // Right goal
    spawn_wall(
        &mut commands,
        WINDOW_SIZE.x / 2. + 5.,
        0.,
        Vec2::NEG_X,
        CollisionLayers::new([Layer::Wall], [Layer::Ball]),
        true,
    );

    // Paddles
    spawn_paddle(&mut commands, true);
    spawn_paddle(&mut commands, false);

    // Game UI including Score Display
    commands.spawn(NodeBundle {
        style: Style { 
            width: Val::Percent(100.),
            height: Val::Percent(20.),
            display: Display::Flex,
            flex_direction: FlexDirection::Row,
            justify_content: JustifyContent::Center,
            ..default()
        },
        ..default()
    })
    .with_children(|builder| {
        builder.spawn(NodeBundle {
            style: Style { 
                width: Val::Percent(20.),
                height: Val::Percent(20.),
                display: Display::Flex,
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::SpaceBetween,
                ..default()
            },
            ..default()
        })
        .with_children(|builder| {
            builder.spawn((TextBundle {
                text: Text::from_section(
                    "0",
                    TextStyle {
                        font_size: 50.0,
                        color: Color::WHITE,
                        ..default()
                    },
                ).with_alignment(TextAlignment::Left),
                ..default()
            }, Player1ScoreMarker));

            builder.spawn((TextBundle {
                text: Text::from_section(
                    "0",
                    TextStyle {
                        font_size: 50.0,
                        color: Color::WHITE,
                        ..default()
                    },
                ).with_alignment(TextAlignment::Right),
                ..default()
            }, Player2ScoreMarker));
        });
    });
}

fn mirror_transform(transform: &mut Transform) {
    transform.translation.x = -transform.translation.x;
}

fn spawn_paddle(commands: &mut Commands, first_player: bool) {
    let mut transform = Transform::from_xyz(-(WINDOW_SIZE.x / 2.) + 20., 0., 5.);
    let (color, side) = if first_player {
        // Not colorblind friendly, use images that look different in black and white
        (Color::ORANGE, Side::Left)
    } else {
        mirror_transform(&mut transform);
        (Color::PURPLE, Side::Right)
    };
    commands.spawn((
        SpriteBundle {
            sprite: Sprite {
                color,
                custom_size: Some(PADDLE_SIZE),
                ..default()
            },
            transform,
            ..default()
        },
        RigidBody::Kinematic,
        Collider::cuboid(PADDLE_SIZE.x, PADDLE_SIZE.y),
        CollisionLayers::new([Layer::Paddle], [Layer::Ball, Layer::Wall, Layer::Net]),
        Restitution::new(0.8),
        Paddle { first_player, side },
    ));
}

fn spawn_wall(
    commands: &mut Commands,
    x: f32,
    y: f32,
    outward_normal: Vec2,
    collision_layers: CollisionLayers,
    goal: bool,
) {
    let mut wall = commands.spawn((
        Transform::from_xyz(x, y, 0.),
        GlobalTransform::default(),
        RigidBody::Static,
        Collider::halfspace(outward_normal),
        collision_layers,
        Restitution::new(0.8),
        Friction::ZERO,
    ));

    if goal {
        let left = x < 0.;
        wall.insert(Goal {
            first_player: left,
            side: if left { Side::Left } else { Side::Right },
        });
    }
}

fn spawn_ball(
    mut commands: Commands,
    ball_assets: Res<BallAssets>,
    spatial_query: SpatialQuery,
    mut timer: Local<Timer>,
    time: Res<Time>,
) {
    timer.tick(time.delta());
    if timer.finished() {
        let ball_collider = Collider::ball(BALL_RADIUS);
        let ball_position = Vec2::ZERO;
        let intersections = spatial_query.shape_intersections(
            &ball_collider,
            ball_position,
            0.,
            SpatialQueryFilter::new().with_masks([Layer::Ball, Layer::Paddle]),
        );

        if intersections.is_empty() {
            let spawn_direction = Side::Random;

            timer.set_duration(Duration::from_millis(10));
            timer.reset();

            let direction_angle = rand::random::<f32>() * (PI / 2. - PI / 4.);
            let mut direction = Vec2::from_angle(direction_angle);
            if spawn_direction == Side::Left
                || (spawn_direction == Side::Random && rand::random::<bool>())
            {
                direction.x = -direction.x;
            }

            commands.spawn((
                SpriteBundle {
                    texture: ball_assets.point_ball.clone(),
                    sprite: Sprite {
                        color: Color::WHITE,
                        custom_size: Some(Vec2::ONE * (BALL_RADIUS * 2.)),
                        ..default()
                    },
                    transform: Transform::from_translation(ball_position.extend(4.)),
                    ..default()
                },
                RigidBody::Dynamic,
                ball_collider,
                CollisionLayers::new([Layer::Ball], [Layer::Ball, Layer::Paddle, Layer::Wall]),
                DelayedExternalForce(
                    ExternalForce::new(direction * INITIAL_FORCE).with_persistence(false),
                ),
                Restitution::new(0.7),
                Friction::ZERO,
                LockedAxes::ROTATION_LOCKED,
                Ball { points: 0 },
            ))
            .with_children(|builder| {
                builder.spawn((
                    Text2dBundle {
                        text: Text {
                            sections: vec![TextSection::new(
                                "0",
                                TextStyle {
                                    font_size: 20.0,
                                    color: Color::WHITE,
                                    ..default()
                                },
                            )],
                            ..Default::default()
                        },
                        transform: Transform::from_xyz(0.0, 0.0, 99.0),
                        ..default()
                    }
                ,BallTextMarker));
            });
        }
    }
}

fn apply_delayed_external_forces(
    mut commands: Commands,
    q: Query<(Entity, &DelayedExternalForce)>,
) {
    for (e, d) in q.iter() {
        commands
            .entity(e)
            .insert(d.0)
            .remove::<DelayedExternalForce>();
    }
}

fn check_goals(
    mut commands: Commands,
    mut collision_event_reader: EventReader<Collision>,
    mut goals: Query<&mut Goal>,
    balls: Query<&Ball>,
    mut score: ResMut<Score>,
    mut point_ball_count: ResMut<PointBallCount>,
    mut paddles: Query<(&mut Position, &mut Paddle)>,
) {
    for Collision(contact) in collision_event_reader.iter() {
        if let Some((goal, _goal_entity, ball, ball_entity)) =
            if let Ok(goal) = goals.get(contact.entity1) {
                if let Ok(ball) = balls.get(contact.entity2) {
                    Some((goal, contact.entity1, ball, contact.entity2))
                } else {
                    None
                }
            } else if let Ok(goal) = goals.get(contact.entity2) {
                if let Ok(ball) = balls.get(contact.entity1) {
                    Some((goal, contact.entity2, ball, contact.entity1))
                } else {
                    None
                }
            } else {
                None
            }
        {
            if goal.first_player {
                score.first_player += 1;
            } else {
                score.second_player += 1;
            }
            if let Some(new_score) = point_ball_count.0.checked_sub(1) {
                point_ball_count.0 = new_score;
            }

            if let Some(entity) = commands.get_entity(ball_entity) {
                entity.despawn_recursive();
            }
        }

//      if let Some((goal, _goal_entity, ball, ball_entity)) =
//          if let Ok((_, paddle)) = paddles.get(contact.entity1) {
//              if let Ok(ball) = balls.get(contact.entity2) {
//                  Some((goal, contact.entity1, ball, contact.entity2))
//              } else {
//                  None
//              }
//          } else if let Ok(goal) = goals.get(contact.entity2) {
//              if let Ok(ball) = balls.get(contact.entity1) {
//                  Some((goal, contact.entity2, ball, contact.entity1))
//              } else {
//                  None
//              }
//          } else {
//              None
//          }
//      {
//      }
    }
}

fn move_paddle(
    time: Res<Time>,
    q_windows: Query<&Window, With<PrimaryWindow>>,
    buttons: Res<Input<MouseButton>>,
    camera: Query<(&GlobalTransform, &Camera)>,
    mut paddles: Query<(&Transform, &mut LinearVelocity, &mut Position, &Paddle)>,
    balls: Query<&Transform, With<Ball>>,
) {
    for (paddle_transform, mut velocity, mut paddle_position, paddle) in paddles.iter_mut() {
        if paddle.first_player {
            if !buttons.pressed(MouseButton::Left) {
                *velocity = LinearVelocity(Vec2::ZERO);
                continue;
            }
            let Some(position) = q_windows.single().cursor_position() else {
                    continue;
                };
            let Some((camera_transform, camera)) = camera.iter().next() else {
                continue;
                };
            let Some(position) = camera.viewport_to_world_2d(camera_transform, position) else {
                continue;
                };
            let to_target_position = position - paddle_transform.translation.xy();
            let new_velocity = to_target_position.normalize_or_zero()
                * PADDLE_SPEED.min(to_target_position.length() / time.delta_seconds());
            velocity.0 = new_velocity;

            // keeping paddle within bounds 
            if paddle_position.x < P1_LEFT_BOUND && velocity.x < 0.0 {
                velocity.x = 0.0;
                paddle_position.x = LEFT_WALL;
            }
            if paddle_position.x > P1_RIGHT_BOUND && velocity.x > 0.0 {
                paddle_position.x = RIGHT_WALL;
                velocity.x = 0.0;
            }
            if paddle_position.y > P1_TOP_BOUND && velocity.y > 0.0 {
                velocity.y = 0.0;
                paddle_position.y = TOP_WALL;
            }
            if paddle_position.y < P1_BOTTOM_BOUND && velocity.y < 0.0 {
                velocity.y = 0.0;
                paddle_position.y = BOTTOM_WALL;
            }
        } else {
            let Some(t) = balls.iter().next() else {
                continue;
            };
            let to_target_position = Vec2::new(paddle_transform.translation.x, t.translation.y)
                - paddle_transform.translation.xy();

            let new_velocity = to_target_position.normalize_or_zero()
                * PADDLE_SPEED_AI.min(to_target_position.length() / time.delta_seconds());
            *velocity = LinearVelocity(new_velocity);
        }
    }
}

fn update_score(
    score: Res<Score>, 
    mut player_1_score: Query<&mut Text, (With<Player1ScoreMarker>, Without<Player2ScoreMarker>)>,
    mut player_2_score: Query<&mut Text, (With<Player2ScoreMarker>, Without<Player1ScoreMarker>)>,
) {
    let mut p1_text = player_1_score.single_mut();
    p1_text.sections[0].value = format!("{}", score.first_player);

    let mut p2_text = player_2_score.single_mut();
    p2_text.sections[0].value = format!("{}", score.second_player);
}
