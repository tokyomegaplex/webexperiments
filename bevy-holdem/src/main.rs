//! Cartoon Hold'em — Bevy 3D (native desktop), phase 1: the table scene.
//!
//! This sets up a real 3D scene — angled perspective camera, lighting, a felt
//! table with a wooden rail, the 5 characters seated around it as upright
//! "billboard" standees (PNG textures with colored fallback), the human's seat
//! at the front, and a row of community-card slots.
//!
//! The poker rules + AI + betting UI are the next phase; this proves the 3D
//! approach and the art pipeline. Run with `cargo run` (see README).

use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::image::{ImageAddressMode, ImageLoaderSettings, ImageSampler, ImageSamplerDescriptor};
use bevy::light::NotShadowCaster;
use bevy::math::Affine2;
use bevy::post_process::bloom::Bloom;
use bevy::render::view::Hdr;
use bevy::ui::RelativeCursorPosition;
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use std::collections::HashMap;
use std::env;

mod characters;
mod poker;
use characters::roster;
use poker::{ai_decide, Action, Game, Player, Street};

/// A character standee: always faces the camera (yaw only) and gets a subtle,
/// stepped (~12fps) noise wobble so it feels hand-animated / alive.
#[derive(Component)]
struct Standee {
    base: Vec3,
    base_scale: Vec3,
    seed: f32,
    /// Extra yaw applied after facing the camera (turns the end seats slightly
    /// inward so they sit naturally in their chairs).
    yaw_offset: f32,
    /// Game seat index this standee represents.
    seat: usize,
    /// 0 = seated, ramps to 1 as a busted player walks away from the table.
    walk: f32,
}

/// Tags a per-seat scene visual (name plate) so it can be hidden when the
/// player busts and leaves.
#[derive(Component)]
struct SeatVisual(usize);

/// Tags one of the two bottom-left UI hole-card images for the human.
#[derive(Component)]
struct HoleCardUi(usize);

/// The bottom-right "Your money" counter.
#[derive(Component)]
struct MyMoney;

/// The container holding the human's action buttons (shown on your turn).
#[derive(Component)]
struct BettingBar;

/// A betting action button. `CheckFold` shows "Check" when checking is legal
/// (and never lets you fold for free) and "Fold" when facing a bet.
#[derive(Component, Clone, Copy)]
enum ActBtn {
    CheckFold,
    Call,
    Raise,
}

/// The bet-size slider position (0 = min raise, 1 = all-in).
#[derive(Resource)]
struct BetSlider {
    frac: f32,
}

#[derive(Component)]
struct SliderTrack;
#[derive(Component)]
struct SliderFill;
#[derive(Component)]
struct SliderHandle;

/// A drifting cigarette-smoke puff: rises, sways, grows and fades on a loop.
#[derive(Component)]
struct Smoke {
    origin: Vec3,
    phase: f32,
    speed: f32,
}

/// Tags an entity that is spawned from the poker engine state and re-created
/// whenever the table is redrawn (cards, chips, the dealer button).
#[derive(Component)]
struct TableProp;

/// The HUD text node (street / pot / players / last action).
#[derive(Component)]
struct HudText;

/// The live poker game plus the geometry it needs to lay itself out.
#[derive(Resource)]
struct Poker {
    game: Game,
    /// World-space table angle (radians) for each seat: seat 0 is the human at
    /// the front; seats 1.. are the AI standees around the back.
    seat_angles: Vec<f32>,
    rx: f32,
    rz: f32,
    /// Standee placement ellipse (where the characters' heads are), for the
    /// floating money labels.
    prx: f32,
    prz: f32,
    felt_top: f32,
    /// Where each AI seat (index = seat-1) goes to stand at the bar after
    /// busting out.
    bar_seats: Vec<Vec3>,
    /// Time between AI actions, and the pause shown after a hand ends.
    act_timer: Timer,
    over_timer: Timer,
    waiting_next: bool,
    /// When true the auto-play loop is frozen (used for deterministic
    /// screenshots so the captured frame matches the prepared state).
    paused: bool,
    log: String,
}

/// Shared meshes/materials the redraw system uses for cards and chips.
#[derive(Resource)]
struct PokerAssets {
    card_quad: Handle<Mesh>,
    chip_mesh: Handle<Mesh>,
    ring_mesh: Handle<Mesh>,
    card_back: Handle<StandardMaterial>,
    button_mat: Handle<StandardMaterial>,
    ring_mat: Handle<StandardMaterial>,
    shadow_mat: Handle<StandardMaterial>,
    chip_mats: Vec<Handle<StandardMaterial>>,
}

/// A floating screen-space label showing a seated player's stack, above their
/// head (seat index into the game's players).
#[derive(Component)]
struct MoneyLabel(usize);

/// The big centre-screen banner shown at showdown.
#[derive(Component)]
struct WinBanner;

/// Card-face materials, cached by code ("As") so we don't leak one per redraw.
#[derive(Resource, Default)]
struct CardFaces(HashMap<String, Handle<StandardMaterial>>);

/// Sound effects, loaded once.
#[derive(Resource)]
struct Sfx {
    chip: Handle<AudioSource>,
    knock: Handle<AudioSource>,
    card: Handle<AudioSource>,
    deal: Handle<AudioSource>,
    win: Handle<AudioSource>,
}

/// Set true after the game state changes; the redraw system rebuilds the props.
#[derive(Resource)]
struct NeedsRedraw(bool);

/// When SCREENSHOT=<path> is set, the app renders a few frames, saves a PNG to
/// that path, and exits — used for automated visual checks (headless via Xvfb).
#[derive(Resource)]
struct ShotState {
    path: String,
    frame: u32,
}

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "Cartoon Hold'em (Bevy)".into(),
            resolution: [1100u32, 760].into(),
            ..default()
        }),
        ..default()
    }))
    .insert_resource(ClearColor(Color::srgb(0.04, 0.06, 0.07)))
    .insert_resource(build_poker())
    .insert_resource(NeedsRedraw(true))
    .insert_resource(BetSlider { frac: 0.5 })
    .init_resource::<CardFaces>()
    .add_systems(Startup, setup)
    .add_systems(
        Update,
        (
            standee_system,
            smoke_system,
            auto_play,
            slider_system,
            betting_ui,
            redraw_table,
            hud,
            money_labels,
            my_money,
            human_cards_ui,
            seat_visibility,
        ),
    );

    if let Ok(path) = env::var("SCREENSHOT") {
        app.insert_resource(ShotState { path, frame: 0 })
            .add_systems(Update, screenshot_system);
    }

    app.run();
}

/// Build the initial game: human in seat 0 (front), the 5 AI characters around
/// the back, 1000-chip stacks, 5/10 blinds. Seeded for reproducibility.
fn build_poker() -> Poker {
    let rx = 5.4_f32;
    let rz = 3.9_f32;
    let cast = roster();

    let mut players = vec![Player::new("You", 1000, true, [0.5, 0.5, 0.2, 0.2])];
    for c in &cast {
        players.push(Player::new(c.name, 1000, false, c.profile));
    }

    // Seat angles: human at the front (+z, toward the camera), AI spread across
    // the far half (matching the standee placement in `setup`).
    let mut seat_angles = vec![90.0_f32.to_radians()];
    let count = cast.len().max(1);
    for i in 0..cast.len() {
        let deg = 200.0 + 140.0 * i as f32 / (count as f32 - 1.0).max(1.0);
        seat_angles.push(deg.to_radians());
    }

    let seed = if env::var("SCREENSHOT").is_ok() {
        7 // deterministic state for visual checks
    } else {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(1)
    };

    let mut game = Game::new(players, 5, 10, seed);
    game.start_hand();

    // For screenshots, fast-forward to the human's turn on the flop so the
    // captured frame shows the community cards, the pot, and the action buttons.
    if env::var("SCREENSHOT").is_ok() {
        let mut steps = 0;
        loop {
            if steps > 600 {
                break;
            }
            steps += 1;
            if game.street == Street::HandOver {
                game.start_hand();
            } else if game.to_act == 0 {
                if game.community.len() >= 3 {
                    break; // your turn on the flop — stop here
                }
                game.apply(Action::Call); // advance preflop without busting out
            } else {
                let seat = game.to_act;
                let action = ai_decide(&mut game, seat);
                game.apply(action);
            }
        }
    }

    // Where busted players go to nurse a drink: standing at the bar stools in
    // the back. One slot per AI seat.
    let bar_seats: Vec<Vec3> = [-5.5_f32, -2.5, 0.5, 3.5, 6.5]
        .iter()
        .map(|&x| Vec3::new(x, 1.6, -8.6))
        .collect();

    Poker {
        game,
        seat_angles,
        rx,
        rz,
        prx: rx + 0.35,
        prz: rz + 0.55,
        felt_top: 1.15,
        bar_seats,
        act_timer: Timer::from_seconds(0.9, TimerMode::Repeating),
        over_timer: Timer::from_seconds(3.0, TimerMode::Once),
        waiting_next: false,
        paused: env::var("SCREENSHOT").is_ok(),
        log: "New hand".to_string(),
    }
}

fn screenshot_system(mut state: ResMut<ShotState>, mut commands: Commands) {
    state.frame += 1;
    // Give the renderer + async texture loads a few frames to settle.
    if state.frame == 30 {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(state.path.clone()));
    }
    // The screenshot is written asynchronously a frame or two after capture;
    // by now it's safely on disk, so just end the process.
    if state.frame >= 60 {
        std::process::exit(0);
    }
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
    poker: Res<Poker>,
) {
    // Oval table dimensions (x radius, z radius), felt surface height, floor.
    let rx = 5.4_f32;
    let rz = 3.9_f32;
    let felt_top = 1.15_f32; // real table height, so the edge hides seated legs
    let floor_y = -0.5_f32;

    // --- camera: low, table-filling player's-eye view ---
    // Tonemapping::None keeps the flat cartoon art at its true sRGB colors
    // (the default tonemapper was shifting them — that was the "red tint").
    // (AmbientLight is a per-camera component in Bevy 0.18, not a resource.)
    let cam_pos = Vec3::new(0.0, 4.7, 10.4);
    commands.spawn((
        Camera3d::default(),
        // HDR + bloom so the lamp bulb (and its emissive shade) actually glow.
        Hdr,
        Tonemapping::None,
        Bloom { intensity: 0.14, ..Bloom::NATURAL },
        // Atmospheric distance fog: the table + players stay clear, while the
        // bar and wall pictures fade into soft dark haze with distance, so the
        // background recedes behind the crisp foreground. (Reliable on every
        // GPU, unlike the post-process depth-of-field pass.)
        DistanceFog {
            color: Color::srgb(0.04, 0.045, 0.06),
            // Players sit ~15 units out (kept clear); the bar (~22) and wall
            // pictures (~24) ramp into haze so they recede behind the foreground.
            falloff: FogFalloff::Linear {
                start: 16.0,
                end: 25.0,
            },
            ..default()
        },
        Transform::from_translation(cam_pos).looking_at(Vec3::new(0.0, 1.8, -2.0), Vec3::Y),
        AmbientLight {
            color: Color::srgb(0.78, 0.82, 1.0),
            // Lower ambient darkens the room generally; the foreground is then
            // lifted by a dedicated front fill so the players stay bright/crisp.
            brightness: 70.0,
            ..default()
        },
    ));

    // --- lighting: warm key + cool fill + an overhead lamp pool ---
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(1.0, 0.96, 0.88),
            // Dialed down: this global key also lit the back bar. The foreground
            // is carried by the range-limited point light over the table.
            illuminance: 1900.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(6.0, 12.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(0.7, 0.8, 1.0),
            illuminance: 900.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_xyz(-7.0, 6.0, -4.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    // Cool back/hair light from behind the table: rims the rail and the
    // characters' edges so the players separate from the backdrop.
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(0.5, 0.66, 1.0),
            illuminance: 1500.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_xyz(0.0, 7.5, -11.0).looking_at(Vec3::new(0.0, 2.6, 3.0), Vec3::Y),
    ));
    // Foreground key: a range-limited point light over the table. It lifts the
    // players and felt but falls off before the bar (15m+ away), so the
    // background stays dark — unlike a directional fill, which would light the
    // bar too (everything there faces the camera).
    commands.spawn((
        PointLight {
            intensity: 2_300_000.0,
            color: Color::srgb(1.0, 0.95, 0.86),
            range: 14.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_xyz(0.0, 5.0, 4.0),
    ));
    // Warm hanging lamp casting a pool of light onto the felt (the mood key).
    // Kept below clipping so the felt centre doesn't blow out to white.
    commands.spawn((
        SpotLight {
            intensity: 4_500_000.0,
            color: Color::srgb(1.0, 0.84, 0.58),
            shadows_enabled: true,
            range: 45.0,
            outer_angle: 0.7,
            inner_angle: 0.4,
            ..default()
        },
        Transform::from_xyz(0.0, 9.5, -0.3).looking_at(Vec3::new(0.0, felt_top, -0.5), Vec3::Y),
    ));

    // --- the hanging lamp: chain, bronze fitting, a glowing cloth shade ---
    let lamp_x = 0.0;
    let lamp_z = -0.4;
    let shade_y = 6.5;
    let bronze = materials.add(StandardMaterial {
        base_color: Color::srgb_u8(120, 86, 40),
        metallic: 0.8,
        perceptual_roughness: 0.4,
        ..default()
    });
    // chain up out of frame
    commands.spawn((
        Mesh3d(meshes.add(Cylinder::new(0.035, 4.0))),
        MeshMaterial3d(bronze.clone()),
        Transform::from_xyz(lamp_x, shade_y + 2.6, lamp_z),
        NotShadowCaster,
    ));
    // top fitting / finial
    commands.spawn((
        Mesh3d(meshes.add(Cylinder::new(0.16, 0.22))),
        MeshMaterial3d(bronze.clone()),
        Transform::from_xyz(lamp_x, shade_y + 0.62, lamp_z),
        NotShadowCaster,
    ));
    // the cloth shade: a wide bronze-framed frustum, lit warm from within
    commands.spawn((
        Mesh3d(meshes.add(ConicalFrustum {
            radius_top: 0.5,
            radius_bottom: 1.6,
            height: 1.15,
        })),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(asset_server.load("lampshade.png")),
            emissive: LinearRgba::rgb(0.62, 0.46, 0.2),
            double_sided: true,
            cull_mode: None,
            perceptual_roughness: 0.7,
            ..default()
        })),
        Transform::from_xyz(lamp_x, shade_y, lamp_z),
        NotShadowCaster,
    ));
    // glowing diffuser disc across the bottom opening (the lit underside)
    commands.spawn((
        Mesh3d(meshes.add(Circle::new(1.5))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.86, 0.6),
            emissive: LinearRgba::rgb(0.72, 0.52, 0.26),
            unlit: true,
            ..default()
        })),
        Transform::from_xyz(lamp_x, shade_y - 0.56, lamp_z)
            .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
        NotShadowCaster,
    ));
    // bulb glow inside (kept modest so it reads as a bulb, not a flare)
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(0.18))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.92, 0.72),
            emissive: LinearRgba::rgb(1.4, 1.12, 0.62),
            ..default()
        })),
        Transform::from_xyz(lamp_x, shade_y - 0.2, lamp_z),
        NotShadowCaster,
    ));

    // --- table top (felt), raised to real table height ---
    commands.spawn((
        Mesh3d(meshes.add(Cylinder::new(1.0, 1.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(asset_server.load("felt.png")),
            // Mostly matte cloth with only a hint of sheen, so the lamp pool
            // doesn't clip to white.
            perceptual_roughness: 0.9,
            reflectance: 0.18,
            ..default()
        })),
        Transform::from_xyz(0.0, felt_top - 0.15, 0.0).with_scale(Vec3::new(rx, 0.3, rz)),
    ));

    // --- gold inlay line where the felt meets the rail (bloom catches it) ---
    commands.spawn((
        Mesh3d(meshes.add(Torus { minor_radius: 0.013, major_radius: 1.0 })),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb_u8(214, 176, 96),
            emissive: LinearRgba::rgb(0.28, 0.21, 0.07),
            metallic: 0.9,
            perceptual_roughness: 0.28,
            ..default()
        })),
        Transform::from_xyz(0.0, felt_top + 0.004, 0.0)
            .with_scale(Vec3::new(rx * 0.985, 1.0, rz * 0.985)),
        NotShadowCaster,
    ));

    // --- padded rail (a larger, darker oval at the felt edge) ---
    commands.spawn((
        Mesh3d(meshes.add(Cylinder::new(1.0, 1.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(asset_server.load("wood.png")),
            perceptual_roughness: 0.45,
            ..default()
        })),
        Transform::from_xyz(0.0, felt_top - 0.22, 0.0).with_scale(Vec3::new(rx + 0.4, 0.34, rz + 0.4)),
    ));

    // --- betting line (thin ring on the felt) ---
    commands.spawn((
        Mesh3d(meshes.add(Torus { minor_radius: 0.03, major_radius: 1.0 })),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb_u8(16, 92, 60),
            unlit: true,
            ..default()
        })),
        Transform::from_xyz(0.0, felt_top + 0.005, -0.3)
            .with_scale(Vec3::new(rx * 0.6, 1.0, rz * 0.58)),
    ));

    // --- pedestal down to the floor ---
    let ped_top = felt_top - 0.3;
    commands.spawn((
        Mesh3d(meshes.add(Cylinder::new(1.0, 1.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb_u8(48, 31, 18),
            perceptual_roughness: 0.85,
            ..default()
        })),
        Transform::from_xyz(0.0, (floor_y + ped_top) / 2.0, 0.0)
            .with_scale(Vec3::new(rx * 0.4, ped_top - floor_y, rz * 0.4)),
    ));

    // Load an image set to tile (repeat) so big surfaces don't stretch.
    let tiled = |path: &str| -> Handle<Image> {
        asset_server.load_with_settings(path.to_string(), |s: &mut ImageLoaderSettings| {
            s.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
                address_mode_u: ImageAddressMode::Repeat,
                address_mode_v: ImageAddressMode::Repeat,
                ..ImageSamplerDescriptor::linear()
            });
        })
    };

    // --- floor: tiled patterned carpet ---
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(80.0, 80.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.7, 0.7, 0.72),
            base_color_texture: Some(tiled("carpet.png")),
            // Tile the 80x80 floor so the damask reads at a believable scale.
            uv_transform: Affine2::from_scale(Vec2::splat(20.0)),
            perceptual_roughness: 0.95,
            ..default()
        })),
        Transform::from_xyz(0.0, -0.5, 0.0),
    ));

    // --- a patterned rug under the table (sits on top of the carpet) ---
    commands.spawn((
        Mesh3d(meshes.add(Rectangle::new(15.5, 11.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.9, 0.9, 0.9),
            base_color_texture: Some(asset_server.load("rug.png")),
            perceptual_roughness: 0.95,
            ..default()
        })),
        Transform::from_xyz(0.0, floor_y + 0.02, 0.6)
            .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
    ));

    // --- the room: textured back/side walls + a coffered ceiling ---
    let ceiling_y = 9.5_f32;
    let back_z = -13.5_f32;
    let wall_x = 14.0_f32;
    let side_len = 21.5_f32; // z extent of the side walls / ceiling
    let side_cz = -2.75_f32; // their z centre
    let wall_h = ceiling_y - floor_y;
    let wall_cy = (ceiling_y + floor_y) / 2.0;
    let wall_mat = |tex: Handle<Image>, sx: f32, sy: f32| StandardMaterial {
        base_color: Color::srgb(0.85, 0.85, 0.85),
        base_color_texture: Some(tex),
        uv_transform: Affine2::from_scale(Vec2::new(sx, sy)),
        perceptual_roughness: 0.95,
        ..default()
    };
    // back wall
    commands.spawn((
        Mesh3d(meshes.add(Rectangle::new(2.0 * wall_x, wall_h))),
        MeshMaterial3d(materials.add(wall_mat(tiled("wall.png"), 9.0, 3.0))),
        Transform::from_xyz(0.0, wall_cy, back_z),
    ));
    // left wall (normal faces +X, inward)
    commands.spawn((
        Mesh3d(meshes.add(Rectangle::new(side_len, wall_h))),
        MeshMaterial3d(materials.add(wall_mat(tiled("wall.png"), 7.0, 3.0))),
        Transform::from_xyz(-wall_x, wall_cy, side_cz)
            .with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)),
    ));
    // right wall (normal faces -X, inward)
    commands.spawn((
        Mesh3d(meshes.add(Rectangle::new(side_len, wall_h))),
        MeshMaterial3d(materials.add(wall_mat(tiled("wall.png"), 7.0, 3.0))),
        Transform::from_xyz(wall_x, wall_cy, side_cz)
            .with_rotation(Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2)),
    ));
    // ceiling (normal faces down)
    commands.spawn((
        Mesh3d(meshes.add(Rectangle::new(2.0 * wall_x, side_len))),
        MeshMaterial3d(materials.add(wall_mat(tiled("ceiling.png"), 7.0, 5.0))),
        Transform::from_xyz(0.0, ceiling_y, side_cz)
            .with_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
    ));

    // --- framed pictures on the back wall, up above the bar (gilt + unlit) ---
    let frame_mat = materials.add(StandardMaterial {
        base_color: Color::srgb_u8(110, 84, 38),
        metallic: 0.8,
        perceptual_roughness: 0.4,
        ..default()
    });
    // (art, width, height, x, y) on the back wall (faces +Z toward the room)
    let pics = [
        ("art_landscape.png", 3.0_f32, 2.3_f32, -7.0_f32, 6.4_f32),
        ("art_portrait.png", 2.0, 2.6, -2.6, 6.4),
        ("art_stilllife.png", 2.0, 2.6, 2.6, 6.4),
        ("art_landscape.png", 3.0, 2.3, 7.0, 6.4),
    ];
    for (art, w, h, px, py) in pics {
        let frame_pos = Vec3::new(px, py, back_z + 0.12);
        let art_pos = Vec3::new(px, py, back_z + 0.23);
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(w + 0.24, h + 0.24, 0.12))),
            MeshMaterial3d(frame_mat.clone()),
            Transform::from_translation(frame_pos),
        ));
        commands.spawn((
            Mesh3d(meshes.add(Rectangle::new(w, h))),
            MeshMaterial3d(materials.add(StandardMaterial {
                // Dim the art so the background recedes behind the players.
                base_color: Color::srgb(0.45, 0.45, 0.45),
                base_color_texture: Some(asset_server.load(art)),
                unlit: true,
                ..default()
            })),
            Transform::from_translation(art_pos),
        ));
    }

    // --- background bar: a back cabinet, a counter, and rows of bottles ---
    let bar_z = -11.5;
    let dark_wood = materials.add(StandardMaterial {
        base_color: Color::srgb_u8(24, 16, 12),
        perceptual_roughness: 0.8,
        ..default()
    });
    // tall back cabinet
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(24.0, 5.4, 0.5))),
        MeshMaterial3d(dark_wood.clone()),
        Transform::from_xyz(0.0, floor_y + 2.7, bar_z - 0.7),
    ));
    // two shelves
    for sh in [1.5_f32, 2.7] {
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(22.0, 0.12, 0.7))),
            MeshMaterial3d(dark_wood.clone()),
            Transform::from_xyz(0.0, floor_y + sh, bar_z - 0.45),
        ));
    }
    // counter front + top
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(22.0, 1.5, 0.4))),
        MeshMaterial3d(dark_wood.clone()),
        Transform::from_xyz(0.0, floor_y + 0.75, bar_z + 1.4),
    ));
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(22.4, 0.22, 1.5))),
        MeshMaterial3d(materials.add(StandardMaterial {
            // Polished lacquered bar top: glossy so lights/bottles glint on it.
            base_color: Color::srgb_u8(40, 24, 16),
            perceptual_roughness: 0.08,
            metallic: 0.2,
            reflectance: 0.7,
            ..default()
        })),
        Transform::from_xyz(0.0, floor_y + 1.55, bar_z + 1.4),
    ));
    // bottles: muted glass, varied sizes, each with a small paper label.
    let bottle_cols = [
        Color::srgb_u8(26, 43, 30), // dark green
        Color::srgb_u8(48, 33, 20), // brown
        Color::srgb_u8(78, 80, 76), // smoke / clear
        Color::srgb_u8(62, 43, 20), // amber
        Color::srgb_u8(46, 27, 26), // dark red-brown
    ];
    let bottle_mats: Vec<_> = bottle_cols
        .iter()
        .map(|c| {
            materials.add(StandardMaterial {
                base_color: *c,
                perceptual_roughness: 0.3,
                reflectance: 0.3,
                ..default()
            })
        })
        .collect();
    let label_mat = materials.add(StandardMaterial {
        // Lit (not unlit) so the labels go dark with the dim bar instead of
        // staying bright cream specks.
        base_color: Color::srgb(0.7, 0.7, 0.7),
        base_color_texture: Some(asset_server.load("label.png")),
        alpha_mode: AlphaMode::Blend,
        perceptual_roughness: 0.9,
        ..default()
    });
    // Deterministic PRNG so the clutter is randomized but stable across runs.
    let mut seed: u32 = 0x9E37_79B9;
    let mut rng = move || {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (seed >> 9) as f32 / 8_388_608.0
    };
    let bz = bar_z - 0.45;
    for shelf_top in [floor_y + 1.56, floor_y + 2.76] {
        let mut bx = -9.9;
        while bx < 9.5 {
            let mat = bottle_mats[(rng() * 5.0) as usize % bottle_mats.len()].clone();
            // randomized silhouette
            let rad = 0.085 + rng() * 0.06; // 0.085 .. 0.145
            let h = 0.42 + rng() * 0.5; // 0.42 .. 0.92
            let neck_h = 0.18 + rng() * 0.16;
            let jz = (rng() - 0.5) * 0.14; // depth jitter so they don't line up
            let cy = shelf_top + h / 2.0;
            bx += rad; // advance to this bottle's centre
            commands.spawn((
                Mesh3d(meshes.add(Cylinder::new(rad, h))),
                MeshMaterial3d(mat.clone()),
                Transform::from_xyz(bx, cy, bz + jz),
                NotShadowCaster,
            ));
            commands.spawn((
                Mesh3d(meshes.add(Cylinder::new(rad * 0.4, neck_h))),
                MeshMaterial3d(mat),
                Transform::from_xyz(bx, cy + h / 2.0 + neck_h / 2.0 - 0.01, bz + jz),
                NotShadowCaster,
            ));
            if rng() > 0.18 {
                commands.spawn((
                    Mesh3d(meshes.add(Rectangle::new(rad * 1.5, h * 0.5))),
                    MeshMaterial3d(label_mat.clone()),
                    Transform::from_xyz(bx, cy, bz + jz + rad + 0.006),
                    NotShadowCaster,
                ));
            }
            // tight, uneven gap to the next bottle (often nearly touching)
            bx += rad + 0.005 + rng() * 0.05;
        }
    }
    // a dim warm wash on the back bar (kept low so the bar reads moodier
    // than the brighter foreground)
    commands.spawn((
        PointLight {
            intensity: 70_000.0,
            color: Color::srgb(1.0, 0.7, 0.42),
            range: 12.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_xyz(0.0, floor_y + 2.4, bar_z + 1.0),
    ));

    // --- bartender behind the counter (stylised: shirt, vest, bow-tie) ---
    {
        let bx = -7.5;
        let bbz = bar_z + 0.4; // between the back shelves and the counter
        let shirt = materials.add(StandardMaterial {
            base_color: Color::srgb_u8(232, 230, 224),
            perceptual_roughness: 0.7,
            ..default()
        });
        let vest = materials.add(StandardMaterial {
            base_color: Color::srgb_u8(28, 28, 34),
            perceptual_roughness: 0.6,
            ..default()
        });
        let skin = materials.add(StandardMaterial {
            base_color: Color::srgb_u8(232, 196, 150),
            perceptual_roughness: 0.8,
            ..default()
        });
        let hair = materials.add(StandardMaterial {
            base_color: Color::srgb_u8(48, 32, 22),
            perceptual_roughness: 0.9,
            ..default()
        });
        // torso (white shirt) with a dark vest panel in front
        commands.spawn((
            Mesh3d(meshes.add(Cylinder::new(0.42, 1.5))),
            MeshMaterial3d(shirt.clone()),
            Transform::from_xyz(bx, floor_y + 2.25, bbz),
        ));
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(0.66, 1.35, 0.2))),
            MeshMaterial3d(vest.clone()),
            Transform::from_xyz(bx, floor_y + 2.2, bbz + 0.34),
        ));
        // bow-tie
        commands.spawn((
            Mesh3d(meshes.add(Cuboid::new(0.22, 0.09, 0.08))),
            MeshMaterial3d(vest.clone()),
            Transform::from_xyz(bx, floor_y + 2.92, bbz + 0.4),
        ));
        // head + hair
        commands.spawn((
            Mesh3d(meshes.add(Sphere::new(0.34))),
            MeshMaterial3d(skin.clone()),
            Transform::from_xyz(bx, floor_y + 3.32, bbz + 0.05),
        ));
        commands.spawn((
            Mesh3d(meshes.add(Sphere::new(0.36))),
            MeshMaterial3d(hair.clone()),
            Transform::from_xyz(bx, floor_y + 3.52, bbz + 0.0)
                .with_scale(Vec3::new(1.0, 0.6, 1.0)),
        ));
        // two arms resting toward the counter
        for s in [-1.0_f32, 1.0] {
            commands.spawn((
                Mesh3d(meshes.add(Cylinder::new(0.12, 1.1))),
                MeshMaterial3d(shirt.clone()),
                Transform::from_xyz(bx + s * 0.5, floor_y + 2.0, bbz + 0.25)
                    .with_rotation(Quat::from_rotation_z(s * 0.5) * Quat::from_rotation_x(0.6)),
            ));
        }
    }

    // --- clutter on the bar top: coasters, glasses, a beer tap ---
    let counter_y = floor_y + 1.55 + 0.11; // top surface of the bar
    let counter_z = bar_z + 1.4;
    let coaster_mat = materials.add(StandardMaterial {
        base_color: Color::srgb_u8(58, 40, 26),
        perceptual_roughness: 0.95,
        ..default()
    });
    let glass_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.85, 0.7, 0.4, 0.4),
        alpha_mode: AlphaMode::Blend,
        perceptual_roughness: 0.05,
        reflectance: 0.6,
        ..default()
    });
    let glass_mesh = meshes.add(Cylinder::new(0.1, 0.24));
    let coaster_mesh = meshes.add(Cylinder::new(1.0, 1.0));
    for (cx, has_glass) in [(-6.5_f32, true), (-2.2, false), (2.2, true), (4.0, true), (6.5, false)] {
        commands.spawn((
            Mesh3d(coaster_mesh.clone()),
            MeshMaterial3d(coaster_mat.clone()),
            Transform::from_xyz(cx, counter_y + 0.012, counter_z + 0.1)
                .with_scale(Vec3::new(0.32, 0.025, 0.32)),
        ));
        if has_glass {
            commands.spawn((
                Mesh3d(glass_mesh.clone()),
                MeshMaterial3d(glass_mat.clone()),
                Transform::from_xyz(cx, counter_y + 0.14, counter_z + 0.1),
                NotShadowCaster,
            ));
        }
    }
    // beer-tap tower with three coloured handles
    let chrome = materials.add(StandardMaterial {
        base_color: Color::srgb_u8(180, 184, 190),
        metallic: 0.9,
        perceptual_roughness: 0.18,
        ..default()
    });
    let tap_x = -4.4;
    commands.spawn((
        Mesh3d(meshes.add(Cylinder::new(0.08, 0.6))),
        MeshMaterial3d(chrome.clone()),
        Transform::from_xyz(tap_x, counter_y + 0.3, counter_z),
    ));
    let handle_cols = [
        Color::srgb_u8(150, 40, 40),
        Color::srgb_u8(40, 90, 60),
        Color::srgb_u8(190, 150, 60),
    ];
    for (hi, hc) in handle_cols.iter().enumerate() {
        let hx = tap_x + (hi as f32 - 1.0) * 0.16;
        // little spout
        commands.spawn((
            Mesh3d(meshes.add(Cylinder::new(0.02, 0.16))),
            MeshMaterial3d(chrome.clone()),
            Transform::from_xyz(hx, counter_y + 0.18, counter_z + 0.14)
                .with_rotation(Quat::from_rotation_x(0.5)),
        ));
        // handle knob
        commands.spawn((
            Mesh3d(meshes.add(Sphere::new(0.07))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: *hc,
                perceptual_roughness: 0.5,
                ..default()
            })),
            Transform::from_xyz(hx, counter_y + 0.62, counter_z + 0.02),
        ));
        commands.spawn((
            Mesh3d(meshes.add(Cylinder::new(0.018, 0.3))),
            MeshMaterial3d(chrome.clone()),
            Transform::from_xyz(hx, counter_y + 0.47, counter_z + 0.02),
        ));
    }

    // --- bar stools in front of the counter ---
    let stool_metal = materials.add(StandardMaterial {
        base_color: Color::srgb_u8(38, 38, 42),
        metallic: 0.6,
        perceptual_roughness: 0.45,
        ..default()
    });
    let stool_seat_mat = materials.add(StandardMaterial {
        base_color: Color::srgb_u8(96, 28, 30),
        perceptual_roughness: 0.5,
        ..default()
    });
    let stool_seat_mesh = meshes.add(Cylinder::new(0.36, 0.14));
    let stool_post_mesh = meshes.add(Cylinder::new(0.07, 1.5));
    let stool_ring_mesh = meshes.add(Torus { minor_radius: 0.028, major_radius: 0.26 });
    let stool_y = floor_y + 1.5;
    // One stool per AI seat, at the spots busted players walk to.
    for slot in &poker.bar_seats {
        let (sx, sz) = (slot.x, slot.z);
        commands.spawn((
            Mesh3d(stool_seat_mesh.clone()),
            MeshMaterial3d(stool_seat_mat.clone()),
            Transform::from_xyz(sx, stool_y, sz),
        ));
        commands.spawn((
            Mesh3d(stool_post_mesh.clone()),
            MeshMaterial3d(stool_metal.clone()),
            Transform::from_xyz(sx, floor_y + 0.75, sz),
        ));
        commands.spawn((
            Mesh3d(stool_ring_mesh.clone()),
            MeshMaterial3d(stool_metal.clone()),
            Transform::from_xyz(sx, floor_y + 0.45, sz),
            NotShadowCaster,
        ));
    }

    // Shared meshes/materials for chairs behind the seated players.
    let chair_wood = materials.add(StandardMaterial {
        base_color: Color::srgb_u8(58, 36, 22),
        perceptual_roughness: 0.6,
        ..default()
    });
    let chair_leather = materials.add(StandardMaterial {
        base_color: Color::srgb_u8(104, 30, 32),
        perceptual_roughness: 0.55,
        ..default()
    });
    let chair_back_mesh = meshes.add(Cuboid::new(1.9, 1.3, 0.14));
    let chair_post_mesh = meshes.add(Cylinder::new(0.08, 2.6));

    // Shared meshes/materials for chips and soft "blob" shadows.
    let disc = meshes.add(Cylinder::new(1.0, 1.0)); // reused, scaled per use
    let blob_mat = materials.add(StandardMaterial {
        base_color: Color::srgba(0.0, 0.0, 0.0, 0.34),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    let chip_colors = [
        Color::srgb_u8(200, 46, 46),
        Color::srgb_u8(44, 92, 200),
        Color::srgb_u8(34, 150, 84),
        Color::srgb_u8(232, 232, 224),
        Color::srgb_u8(150, 70, 190),
    ];
    let chip_mats: Vec<_> = chip_colors
        .iter()
        .map(|c| materials.add(StandardMaterial {
            base_color: *c,
            perceptual_roughness: 0.5,
            ..default()
        }))
        .collect();

    // --- cup-holders recessed into the padded rail (gold rim + dark hole) ---
    let holder_y = felt_top - 0.045;
    let holder_rim = materials.add(StandardMaterial {
        base_color: Color::srgb_u8(206, 170, 96),
        emissive: LinearRgba::rgb(0.16, 0.12, 0.04),
        metallic: 0.9,
        perceptual_roughness: 0.3,
        ..default()
    });
    let holder_hole = materials.add(StandardMaterial {
        base_color: Color::srgb_u8(12, 9, 7),
        perceptual_roughness: 0.95,
        ..default()
    });
    let holder_ring = meshes.add(Torus { minor_radius: 0.03, major_radius: 0.15 });
    for j in 0..9 {
        // Spread around the rail, leaving a ~40deg gap at the front (the camera seat).
        let a = (110.0 + 320.0 * j as f32 / 8.0).to_radians();
        let hx = a.cos() * (rx + 0.18);
        let hz = a.sin() * (rz + 0.30);
        commands.spawn((
            Mesh3d(holder_ring.clone()),
            MeshMaterial3d(holder_rim.clone()),
            Transform::from_xyz(hx, holder_y + 0.01, hz),
            NotShadowCaster,
        ));
        commands.spawn((
            Mesh3d(disc.clone()),
            MeshMaterial3d(holder_hole.clone()),
            Transform::from_xyz(hx, holder_y, hz).with_scale(Vec3::new(0.15, 0.02, 0.15)),
        ));
    }

    // --- characters: seated around the back + sides as upright standees ---
    // Front-center (toward the camera) is left open for the human.
    let chars = roster();
    let quad_w = 3.3_f32;
    let quad_h = 4.2_f32;
    let quad = meshes.add(Rectangle::new(quad_w, quad_h));
    let plate_quad = meshes.add(Rectangle::new(1.7, 0.45));
    let prx = rx + 0.35; // placement ellipse, hugging the outer rail
    let prz = rz + 0.55;
    let count = chars.len().max(1);
    for (i, c) in chars.iter().enumerate() {
        // Wrap evenly across the far half of the table (200deg .. 340deg),
        // leaving the near edge (toward the camera) open for the human.
        let span = 140.0;
        let angle = (200.0 + span * i as f32 / (count as f32 - 1.0).max(1.0)).to_radians();
        let x = angle.cos() * prx;
        let z = angle.sin() * prz;

        // A chair behind each player: a leather backrest on two posts. It's
        // placed directly behind the standee *as seen from the camera* and
        // turned to face the camera, so the flat billboard never clips through
        // it and the chair's edges peek out at the sides.
        let to_cam = Vec2::new(cam_pos.x - x, cam_pos.z - z).normalize();
        let cyaw = to_cam.x.atan2(to_cam.y);
        let crot = Quat::from_rotation_y(cyaw);
        let cright = crot * Vec3::X;
        let cback = Vec3::new(x - to_cam.x * 0.55, floor_y + 2.0, z - to_cam.y * 0.55);
        commands.spawn((
            Mesh3d(chair_back_mesh.clone()),
            MeshMaterial3d(chair_leather.clone()),
            Transform::from_translation(cback).with_rotation(crot),
        ));
        for s in [-1.0_f32, 1.0] {
            commands.spawn((
                Mesh3d(chair_post_mesh.clone()),
                MeshMaterial3d(chair_wood.clone()),
                Transform::from_translation(
                    Vec3::new(cback.x, floor_y + 1.3, cback.z) + cright * (0.84 * s),
                )
                .with_rotation(crot),
            ));
        }

        let tex = asset_server.load(c.file);
        let material = materials.add(StandardMaterial {
            // White base so the PNG shows its true colors (no tint).
            base_color: Color::WHITE,
            base_color_texture: Some(tex.clone()),
            // Self-light the art so the flat cartoon colours read true and vivid
            // (not washed out) even as the room is dim. Transparent pixels are
            // black in the source PNGs, so this adds no halo.
            emissive: LinearRgba::rgb(0.7, 0.7, 0.7),
            emissive_texture: Some(tex),
            // Fully matte, zero specular: kills the grey sheen that was lifting
            // dark areas (e.g. the inside of Hoodguy's hood) to grey.
            perceptual_roughness: 1.0,
            reflectance: 0.0,
            metallic: 0.0,
            alpha_mode: AlphaMode::Blend,
            double_sided: true,
            cull_mode: None,
            ..default()
        });

        // Feet on the floor; the raised table edge crosses the lower body so
        // they read as seated rather than floating.
        let base = Vec3::new(x, floor_y + quad_h / 2.0, z);
        commands.spawn((
            Mesh3d(quad.clone()),
            MeshMaterial3d(material),
            Transform::from_translation(base),
            // Flat quads make ugly shadows; use a blob shadow instead.
            NotShadowCaster,
            Standee {
                base,
                base_scale: Vec3::ONE,
                seed: i as f32 * 1.7 + 0.3,
                yaw_offset: if i == 0 {
                    0.16
                } else if i + 1 == count {
                    -0.16
                } else {
                    0.0
                },
                seat: i + 1,
                walk: 0.0,
            },
            Name::new(c.name),
        ));

        // Soft contact/blob shadow on the floor beneath the standee.
        commands.spawn((
            Mesh3d(disc.clone()),
            MeshMaterial3d(blob_mat.clone()),
            Transform::from_xyz(x, floor_y + 0.02, z)
                .with_scale(Vec3::new(1.7, 0.02, 1.1)),
        ));

        // Name plate floating above the head, facing the camera (front side
        // toward the camera so the text isn't mirrored).
        let plate_mat = materials.add(StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(
                asset_server.load(format!("ui/plate_{}.png", c.name.to_lowercase())),
            ),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            double_sided: true,
            cull_mode: None,
            ..default()
        });
        let plate_pos = Vec3::new(x, floor_y + quad_h + 0.15, z);
        let away = plate_pos + (plate_pos - cam_pos); // so +Z faces the camera
        commands.spawn((
            Mesh3d(plate_quad.clone()),
            MeshMaterial3d(plate_mat),
            Transform::from_translation(plate_pos).looking_at(away, Vec3::Y),
            NotShadowCaster,
            SeatVisual(i + 1),
        ));
    }

    // --- poker: shared card/chip assets used by the redraw system ---
    // (The pot, bets, community + hole cards, and dealer button are now spawned
    // dynamically from the engine state — see `redraw_table`.)
    let card_quad = meshes.add(Rectangle::new(0.78, 1.08));
    let card_back = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(asset_server.load("cards/back.png")),
        perceptual_roughness: 0.5,
        ..default()
    });

    // The deck: a face-down stack of cards waiting to be dealt, by the dealer.
    let flat = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
    for k in 0..16 {
        commands.spawn((
            Mesh3d(card_quad.clone()),
            MeshMaterial3d(card_back.clone()),
            Transform::from_xyz(-3.5, felt_top + 0.02 + k as f32 * 0.012, -1.4)
                .with_rotation(flat)
                .with_scale(Vec3::splat(0.78)),
        ));
    }

    commands.insert_resource(PokerAssets {
        card_quad: card_quad.clone(),
        chip_mesh: disc.clone(),
        ring_mesh: meshes.add(Torus {
            minor_radius: 0.05,
            major_radius: 0.95,
        }),
        card_back: card_back.clone(),
        button_mat: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(asset_server.load("button_d.png")),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        }),
        ring_mat: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.85, 0.3),
            emissive: LinearRgba::rgb(0.9, 0.65, 0.1),
            unlit: true,
            ..default()
        }),
        shadow_mat: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(asset_server.load("cards/shadow.png")),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        }),
        chip_mats: chip_mats.clone(),
    });

    commands.insert_resource(Sfx {
        chip: asset_server.load("sfx/chip.wav"),
        knock: asset_server.load("sfx/knock.wav"),
        card: asset_server.load("sfx/card.wav"),
        deal: asset_server.load("sfx/deal.wav"),
        win: asset_server.load("sfx/win.wav"),
    });

    // Floating money labels above each AI head, and a showdown banner.
    for s in 1..=roster().len() {
        commands.spawn((
            Text::new(""),
            TextFont {
                font_size: 20.0,
                ..default()
            },
            TextColor(Color::srgb(1.0, 0.92, 0.55)),
            Node {
                position_type: PositionType::Absolute,
                ..default()
            },
            MoneyLabel(s),
        ));
    }
    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: 40.0,
            ..default()
        },
        TextColor(Color::srgb(1.0, 0.95, 0.6)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(70.0),
            left: Val::Px(0.0),
            right: Val::Px(0.0),
            justify_content: JustifyContent::Center,
            ..default()
        },
        WinBanner,
    ));

    // --- HUD overlay (street / pot / players / last action) ---
    commands.spawn((
        Text::new("dealing..."),
        TextFont {
            font_size: 18.0,
            ..default()
        },
        TextColor(Color::srgb(0.95, 0.95, 0.9)),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(12.0),
            left: Val::Px(12.0),
            ..default()
        },
        HudText,
    ));

    // Your two hole cards, drawn flat in the bottom-left corner (2D overlay).
    for k in 0..2 {
        commands.spawn((
            ImageNode {
                image: asset_server.load("cards/back.png"),
                ..default()
            },
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(22.0 + k as f32 * 86.0),
                bottom: Val::Px(20.0),
                width: Val::Px(112.0),
                height: Val::Px(156.0),
                ..default()
            },
            HoleCardUi(k),
        ));
    }

    // Your money, shown just above your cards in the bottom-left.
    commands.spawn((
        Text::new("$1000"),
        TextFont {
            font_size: 28.0,
            ..default()
        },
        TextColor(Color::srgb(1.0, 0.92, 0.55)),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(24.0),
            bottom: Val::Px(184.0),
            ..default()
        },
        MyMoney,
    ));

    // Your action area (slider + buttons), hidden until it's your turn.
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(24.0),
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(10.0),
                ..default()
            },
            Visibility::Hidden,
            BettingBar,
        ))
        .with_children(|bar| {
            // Bet-size slider (track + fill + handle).
            bar.spawn((
                Node {
                    width: Val::Px(360.0),
                    height: Val::Px(22.0),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.12, 0.12, 0.15)),
                RelativeCursorPosition::default(),
                SliderTrack,
            ))
            .with_children(|track| {
                track.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(0.0),
                        top: Val::Px(0.0),
                        bottom: Val::Px(0.0),
                        width: Val::Percent(50.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.26, 0.5, 0.32)),
                    SliderFill,
                ));
                track.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Percent(50.0),
                        top: Val::Px(-5.0),
                        width: Val::Px(14.0),
                        height: Val::Px(32.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.95, 0.95, 0.9)),
                    SliderHandle,
                ));
            });

            // Buttons row.
            bar.spawn(Node {
                column_gap: Val::Px(10.0),
                ..default()
            })
            .with_children(|row| {
                for kind in [ActBtn::CheckFold, ActBtn::Call, ActBtn::Raise] {
                    row.spawn((
                        Button,
                        Interaction::default(),
                        Node {
                            width: Val::Px(150.0),
                            height: Val::Px(56.0),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.16, 0.16, 0.2)),
                        kind,
                    ))
                    .with_children(|b| {
                        b.spawn((
                            Text::new(""),
                            TextFont {
                                font_size: 22.0,
                                ..default()
                            },
                            TextColor(Color::WHITE),
                        ));
                    });
                }
            });
        });

    // --- ashtray with two cigarettes, off to the back-right of the felt ---
    let ash_x = 3.0;
    let ash_z = -1.5;
    let glass = materials.add(StandardMaterial {
        base_color: Color::srgb_u8(40, 44, 48),
        perceptual_roughness: 0.2,
        reflectance: 0.6,
        metallic: 0.1,
        ..default()
    });
    // shallow dish + rim
    commands.spawn((
        Mesh3d(disc.clone()),
        MeshMaterial3d(glass.clone()),
        Transform::from_xyz(ash_x, felt_top + 0.03, ash_z).with_scale(Vec3::new(0.42, 0.06, 0.42)),
    ));
    commands.spawn((
        Mesh3d(meshes.add(Torus { minor_radius: 0.03, major_radius: 0.4 })),
        MeshMaterial3d(glass.clone()),
        Transform::from_xyz(ash_x, felt_top + 0.07, ash_z),
        NotShadowCaster,
    ));
    // a little grey ash pile in the dish
    commands.spawn((
        Mesh3d(disc.clone()),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb_u8(120, 116, 110),
            perceptual_roughness: 1.0,
            ..default()
        })),
        Transform::from_xyz(ash_x, felt_top + 0.055, ash_z).with_scale(Vec3::new(0.22, 0.04, 0.22)),
    ));
    // two cigarettes resting across the rim
    let cig_paper = materials.add(StandardMaterial {
        base_color: Color::srgb_u8(238, 234, 226),
        perceptual_roughness: 0.9,
        ..default()
    });
    let cig_filter = materials.add(StandardMaterial {
        base_color: Color::srgb_u8(196, 150, 90),
        perceptual_roughness: 0.9,
        ..default()
    });
    let cig_ember = materials.add(StandardMaterial {
        base_color: Color::srgb_u8(80, 30, 12),
        emissive: LinearRgba::rgb(1.2, 0.35, 0.05),
        ..default()
    });
    let cig_mesh = meshes.add(Cylinder::new(0.028, 0.62));
    let smoke_tex = asset_server.load("smoke.png");
    let smoke_quad = meshes.add(Rectangle::new(1.0, 1.0));
    for (sx, ang) in [(-0.18_f32, 0.5_f32), (0.16, -0.7)] {
        let rot = Quat::from_rotation_y(ang) * Quat::from_rotation_z(std::f32::consts::FRAC_PI_2);
        let base = Vec3::new(ash_x + sx, felt_top + 0.08, ash_z);
        let dir = rot * Vec3::Y; // cylinder long axis after rotation
        commands.spawn((
            Mesh3d(cig_mesh.clone()),
            MeshMaterial3d(cig_paper.clone()),
            Transform::from_translation(base).with_rotation(rot),
            NotShadowCaster,
        ));
        // filter end
        commands.spawn((
            Mesh3d(meshes.add(Cylinder::new(0.03, 0.16))),
            MeshMaterial3d(cig_filter.clone()),
            Transform::from_translation(base - dir * 0.36).with_rotation(rot),
            NotShadowCaster,
        ));
        // glowing ember at the far tip
        let ember = base + dir * 0.33;
        commands.spawn((
            Mesh3d(meshes.add(Cylinder::new(0.028, 0.05))),
            MeshMaterial3d(cig_ember.clone()),
            Transform::from_translation(ember).with_rotation(rot),
            NotShadowCaster,
        ));
        // a column of drifting smoke puffs rising from the ember (animated)
        let origin = Vec3::new(ember.x, felt_top + 0.14, ember.z);
        for p in 0..5 {
            let smat = materials.add(StandardMaterial {
                base_color: Color::srgba(0.72, 0.76, 0.82, 0.0),
                base_color_texture: Some(smoke_tex.clone()),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                double_sided: true,
                cull_mode: None,
                ..default()
            });
            commands.spawn((
                Mesh3d(smoke_quad.clone()),
                MeshMaterial3d(smat),
                Transform::from_translation(origin),
                NotShadowCaster,
                Smoke {
                    origin,
                    phase: p as f32 * 0.2 + sx.abs(),
                    speed: 0.22,
                },
            ));
        }
    }
}

/// Animate the cigarette smoke: each puff rises, sways, grows and fades on a
/// loop, billboarding to face the camera.
fn smoke_system(
    time: Res<Time>,
    camera: Query<&Transform, (With<Camera3d>, Without<Smoke>)>,
    mut puffs: Query<(&Smoke, &mut Transform, &MeshMaterial3d<StandardMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(cam) = camera.iter().next() else {
        return;
    };
    let cam_pos = cam.translation;
    let t = time.elapsed_secs();
    for (s, mut tr, mat) in &mut puffs {
        let life = (t * s.speed + s.phase).fract(); // 0..1
        let rise = life * 1.5;
        let sway = (life * 7.0 + s.phase * 6.0).sin() * 0.12 * life;
        let pos = s.origin + Vec3::new(sway, rise, 0.0);
        tr.translation = pos;
        // billboard to camera, then size it (growing as it rises)
        let target = Vec3::new(cam_pos.x, pos.y, cam_pos.z);
        tr.look_at(target, Vec3::Y);
        tr.scale = Vec3::splat(0.08 + life * 0.42);
        // fade in then out across the lifetime
        let alpha = (life * std::f32::consts::PI).sin() * 0.52;
        if let Some(m) = materials.get_mut(&mat.0) {
            m.base_color = Color::srgba(0.72, 0.76, 0.82, alpha);
        }
    }
}

/// Each frame: face the camera (yaw only, stays upright) and apply a very
/// subtle, stepped (~4fps) noise wobble in position, lean, and scale so the
/// standees feel hand-animated / "boiling" like stop-motion, not dead-still.
fn standee_system(
    time: Res<Time>,
    poker: Res<Poker>,
    camera: Query<&Transform, (With<Camera3d>, Without<Standee>)>,
    mut standees: Query<(&mut Standee, &mut Transform, &mut Visibility)>,
) {
    let Some(cam) = camera.iter().next() else {
        return;
    };
    let cam_pos = cam.translation;

    // Quantize time to ~4fps so the motion is stepped (stop-motion), not smooth.
    let step = (time.elapsed_secs() * 4.0).floor();
    let dt = time.delta_secs();

    for (mut s, mut t, mut vis) in &mut standees {
        // Players only get up and leave once they're truly out: no chips and
        // not merely all-in (an all-in player also has a 0 stack).
        let busted = poker.game.players[s.seat].busted();
        if busted {
            s.walk = (s.walk + dt * 0.4).min(1.0);
        } else {
            s.walk = (s.walk - dt * 1.5).max(0.0);
        }
        *vis = Visibility::Visible;

        // Stepped pseudo-noise in [-1, 1], unique per standee via its seed.
        let nx = hash11(s.seed * 1.3 + step * 0.0137) * 2.0 - 1.0;
        let ny = hash11(s.seed * 2.1 + step * 0.0211) * 2.0 - 1.0;
        let nlean = hash11(s.seed * 3.7 + step * 0.0090) * 2.0 - 1.0;
        let nsc = hash11(s.seed * 5.2 + step * 0.0051) * 2.0 - 1.0;

        // Very subtle idle wobble.
        let mut pos = s.base + Vec3::new(nx * 0.011, ny * 0.009, 0.0);

        // Busted: walk over to a stool at the bar in the back and stay there.
        if s.walk > 0.0 {
            let target = poker.bar_seats[(s.seat - 1) % poker.bar_seats.len()];
            let e = s.walk * s.walk * (3.0 - 2.0 * s.walk); // smoothstep
            pos = s.base.lerp(target, e);
            let stride = (time.elapsed_secs() * 9.0).sin() * 0.13 * (s.walk * (1.0 - s.walk) * 4.0);
            pos.y += stride.max(0.0);
        }
        t.translation = pos;

        // Face the camera, yaw only (target at the standee's own height).
        let target = Vec3::new(cam_pos.x, pos.y, cam_pos.z);
        t.look_at(target, Vec3::Y);

        // A tiny lean + scale pulse on top of the facing rotation.
        t.rotate_local_y(s.yaw_offset);
        t.rotate_local_z(nlean * 0.005);
        t.scale = s.base_scale * (1.0 + nsc * 0.004);
    }
}

/// Show the human's two hole cards as a flat 2D overlay in the bottom-left.
fn human_cards_ui(
    poker: Res<Poker>,
    asset_server: Res<AssetServer>,
    mut q: Query<(&HoleCardUi, &mut ImageNode, &mut Visibility)>,
) {
    let human = &poker.game.players[0];
    for (slot, mut img, mut vis) in &mut q {
        if human.in_hand() {
            img.image = asset_server.load(format!("cards/{}.png", human.hole[slot.0].code()));
            *vis = Visibility::Visible;
        } else {
            *vis = Visibility::Hidden;
        }
    }
}

/// Update the bottom-right money counter (your stack, plus your current bet).
fn my_money(poker: Res<Poker>, mut q: Query<&mut Text, With<MyMoney>>) {
    let me = &poker.game.players[0];
    let s = if me.bet > 0 {
        format!("${}  (bet ${})", me.stack, me.bet)
    } else {
        format!("${}", me.stack)
    };
    for mut t in &mut q {
        *t = Text::new(s.clone());
    }
}

/// Hide a busted seat's name plate (it leaves with the character).
fn seat_visibility(poker: Res<Poker>, mut q: Query<(&SeatVisual, &mut Visibility)>) {
    for (sv, mut vis) in &mut q {
        *vis = if poker.game.players[sv.0].busted() {
            Visibility::Hidden
        } else {
            Visibility::Visible
        };
    }
}

/// Cheap deterministic hash → pseudo-noise in [0, 1).
fn hash11(x: f32) -> f32 {
    let v = (x * 127.1).sin() * 43758.5453;
    (v - v.floor()).abs()
}

// ===========================================================================
// Poker: auto-play loop, table redraw from engine state, and the HUD.
// ===========================================================================

/// Drive the table with the AI: one action every `act_timer`, then a pause and
/// a fresh deal once the hand is over.
/// Apply one action for the player to act: mutate the game, play the matching
/// sounds, write the log, and request a redraw. Shared by the AI loop and the
/// human betting UI.
fn do_action(
    poker: &mut Poker,
    action: Action,
    needs: &mut NeedsRedraw,
    commands: &mut Commands,
    sfx: &Sfx,
) {
    let seat = poker.game.to_act;
    let name = poker.game.players[seat].name.clone();
    let desc = describe_action(&poker.game, seat, action);
    let call_amt = poker.game.call_amount(seat);
    let pre_community = poker.game.community.len();
    poker.game.apply(action);

    let mut play = |h: &Handle<AudioSource>| {
        commands.spawn((AudioPlayer(h.clone()), PlaybackSettings::DESPAWN));
    };
    match action {
        Action::Fold => play(&sfx.card),
        Action::Check => play(&sfx.knock),
        Action::Call => play(if call_amt > 0 { &sfx.chip } else { &sfx.knock }),
        Action::Raise(_) => play(&sfx.chip),
    }
    if poker.game.community.len() > pre_community {
        play(&sfx.card);
    }
    if poker.game.street == Street::HandOver {
        play(&sfx.win);
    }
    poker.log = format!("{name} {desc}");
    needs.0 = true;
}

/// Drive the AI seats: one action every `act_timer`, then a pause and a fresh
/// deal once the hand is over. The human's turn is left to the betting UI.
fn auto_play(
    time: Res<Time>,
    mut poker: ResMut<Poker>,
    mut needs: ResMut<NeedsRedraw>,
    mut commands: Commands,
    sfx: Res<Sfx>,
) {
    if poker.paused {
        return;
    }
    let dt = time.delta();
    if poker.game.street == Street::HandOver {
        if !poker.waiting_next {
            poker.over_timer.reset();
            poker.waiting_next = true;
        }
        if poker.over_timer.tick(dt).is_finished() {
            poker.waiting_next = false;
            poker.game.start_hand();
            poker.log = "New hand".to_string();
            needs.0 = true;
            commands.spawn((AudioPlayer(sfx.deal.clone()), PlaybackSettings::DESPAWN));
        }
        return;
    }

    // The human (seat 0) decides via the betting UI — don't auto-act for them.
    if poker.game.to_act == 0 {
        return;
    }

    if poker.act_timer.tick(dt).just_finished() {
        let seat = poker.game.to_act;
        let action = ai_decide(&mut poker.game, seat);
        do_action(&mut poker, action, &mut needs, &mut commands, &sfx);
    }
}

/// The raise amount currently selected on the slider (clamped legal).
fn slider_amount(g: &Game, frac: f32) -> u32 {
    let max = g.max_raise_to(0);
    let min = g.min_raise_to(0).unwrap_or(max);
    if max <= min {
        return max;
    }
    let amt = min as f32 + (max - min) as f32 * frac.clamp(0.0, 1.0);
    (amt.round() as u32).clamp(min, max)
}

/// Drag the bet-size slider and reflect it in the fill + handle position.
fn slider_system(
    mouse: Res<ButtonInput<MouseButton>>,
    mut slider: ResMut<BetSlider>,
    track: Query<&RelativeCursorPosition, With<SliderTrack>>,
    mut fill: Query<&mut Node, (With<SliderFill>, Without<SliderHandle>)>,
    mut handle: Query<&mut Node, (With<SliderHandle>, Without<SliderFill>)>,
) {
    if mouse.pressed(MouseButton::Left) {
        if let Ok(rel) = track.single() {
            if let Some(n) = rel.normalized {
                slider.frac = n.x.clamp(0.0, 1.0);
            }
        }
    }
    let pct = slider.frac * 100.0;
    if let Ok(mut f) = fill.single_mut() {
        f.width = Val::Percent(pct);
    }
    if let Ok(mut h) = handle.single_mut() {
        h.left = Val::Percent(pct);
    }
}

/// Show the human's action UI on their turn and apply the chosen action.
/// CheckFold = "Check" when checking is free (never a free fold) else "Fold".
/// Keys: C = check/call, F = fold (only when facing a bet), R = raise (slider),
/// A = all-in.
fn betting_ui(
    mut poker: ResMut<Poker>,
    mut needs: ResMut<NeedsRedraw>,
    mut commands: Commands,
    sfx: Res<Sfx>,
    slider: Res<BetSlider>,
    keys: Res<ButtonInput<KeyCode>>,
    mut bar: Query<&mut Visibility, With<BettingBar>>,
    mut buttons: Query<
        (
            &Interaction,
            &ActBtn,
            &mut BackgroundColor,
            &mut Visibility,
            &Children,
        ),
        (With<Button>, Without<BettingBar>),
    >,
    mut texts: Query<&mut Text>,
) {
    let g = &poker.game;
    let my_turn = g.street != Street::HandOver
        && g.to_act == 0
        && !g.players[0].folded
        && !g.players[0].all_in;

    for mut vis in &mut bar {
        *vis = if my_turn {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    if !my_turn {
        return;
    }

    let can_check = g.can_check(0);
    let amount = slider_amount(g, slider.frac);

    let mut chosen: Option<Action> = None;
    for (interaction, kind, mut bg, mut vis, children) in &mut buttons {
        // The Call button only appears when there's a bet to call.
        if matches!(kind, ActBtn::Call) {
            *vis = if can_check {
                Visibility::Hidden
            } else {
                Visibility::Inherited
            };
        }
        let label = match kind {
            ActBtn::CheckFold => {
                if can_check {
                    "Check".to_string()
                } else {
                    "Fold".to_string()
                }
            }
            ActBtn::Call => format!("Call ${}", g.call_amount(0)),
            ActBtn::Raise => {
                if can_check {
                    format!("Bet ${amount}")
                } else {
                    format!("Raise to ${amount}")
                }
            }
        };
        if let Some(&child) = children.first() {
            if let Ok(mut t) = texts.get_mut(child) {
                *t = Text::new(label);
            }
        }
        let action = match kind {
            ActBtn::CheckFold => {
                if can_check {
                    Action::Check
                } else {
                    Action::Fold
                }
            }
            ActBtn::Call => Action::Call,
            ActBtn::Raise => Action::Raise(amount),
        };
        match *interaction {
            Interaction::Pressed => {
                *bg = BackgroundColor(Color::srgb(0.20, 0.45, 0.25));
                chosen = Some(action);
            }
            Interaction::Hovered => *bg = BackgroundColor(Color::srgb(0.30, 0.30, 0.36)),
            Interaction::None => *bg = BackgroundColor(Color::srgb(0.16, 0.16, 0.2)),
        }
    }

    // Keyboard shortcuts.
    if keys.just_pressed(KeyCode::KeyC) {
        chosen = Some(if can_check { Action::Check } else { Action::Call });
    } else if keys.just_pressed(KeyCode::KeyF) && !can_check {
        chosen = Some(Action::Fold);
    } else if keys.just_pressed(KeyCode::KeyR) {
        chosen = Some(Action::Raise(amount));
    } else if keys.just_pressed(KeyCode::KeyA) {
        chosen = Some(Action::Raise(g.max_raise_to(0)));
    }

    if let Some(action) = chosen {
        do_action(&mut poker, action, &mut needs, &mut commands, &sfx);
    }
}

fn describe_action(game: &Game, seat: usize, action: Action) -> String {
    match action {
        Action::Fold => "folds".to_string(),
        Action::Check | Action::Call => {
            let amt = game.call_amount(seat);
            if amt == 0 {
                "checks".to_string()
            } else {
                format!("calls ${amt}")
            }
        }
        Action::Raise(to) => format!("raises to ${to}"),
    }
}

/// Rebuild all engine-driven props (cards, chips, dealer button) from the
/// current game state whenever something changed.
fn redraw_table(
    mut commands: Commands,
    mut needs: ResMut<NeedsRedraw>,
    poker: Res<Poker>,
    assets: Res<PokerAssets>,
    mut faces: ResMut<CardFaces>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
    props: Query<Entity, With<TableProp>>,
) {
    if !needs.0 {
        return;
    }
    needs.0 = false;
    for e in &props {
        commands.entity(e).despawn();
    }

    let g = &poker.game;
    let ft = poker.felt_top;
    // Lie flat, face up. Top edge points away from the camera (-Z) so the ranks
    // read right-side-up for the player sitting at the near (+Z) edge.
    let flat = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);

    // Community cards, centred, a touch larger, and tilted up toward the camera.
    let c = g.community.len() as f32;
    for (i, card) in g.community.iter().enumerate() {
        let x = (i as f32 - (c - 1.0) / 2.0) * 1.06;
        let mat = face_material(&mut faces, &mut materials, &asset_server, &card.code());
        spawn_table_card(&mut commands, &assets, mat, flat, x, -0.7, ft, 1.25);
    }

    // The collected pot (everything except the current street's live bets).
    let live_bets: u32 = g.players.iter().map(|p| p.bet).sum();
    let collected = g.pot().saturating_sub(live_bets);
    if collected > 0 {
        spawn_chips(&mut commands, &assets, 0.0, 0.9, ft, collected, 3);
    }

    // Per-seat: bets, hole cards, dealer button.
    for (s, p) in g.players.iter().enumerate() {
        let a = poker.seat_angles[s];
        let (cosv, sinv) = (a.cos(), a.sin());

        if p.bet > 0 {
            let bx = cosv * poker.rx * 0.5;
            let bz = sinv * poker.rz * 0.5;
            spawn_chips(&mut commands, &assets, bx, bz, ft, p.bet, s);
        }

        // The human's cards are drawn first-person (held to the screen), below.
        if p.in_hand() && s != 0 {
            let hx = cosv * poker.rx * 0.72;
            let hz = sinv * poker.rz * 0.72;
            let (tx, tz) = (-sinv, cosv); // tangent, to lay the two cards side by side
            let reveal = g.street == Street::HandOver;
            for (k, card) in p.hole.iter().enumerate() {
                let off = (k as f32 - 0.5) * 0.46;
                let mat = if reveal {
                    face_material(&mut faces, &mut materials, &asset_server, &card.code())
                } else {
                    assets.card_back.clone()
                };
                spawn_table_card(
                    &mut commands,
                    &assets,
                    mat,
                    flat,
                    hx + tx * off,
                    hz + tz * off,
                    ft,
                    0.78,
                );
            }
        }
    }

    // (The human's own hole cards are drawn as a flat 2D UI overlay in the
    // bottom-left — see `human_cards_ui`.)

    // Dealer button next to the button seat.
    let a = poker.seat_angles[g.button];
    commands.spawn((
        Mesh3d(assets.chip_mesh.clone()),
        MeshMaterial3d(assets.button_mat.clone()),
        Transform::from_xyz(a.cos() * poker.rx * 0.6, ft + 0.02, a.sin() * poker.rz * 0.6)
            .with_scale(Vec3::new(0.34, 0.04, 0.34)),
        TableProp,
    ));

    // "Action on you" — a glowing ring in front of the player to act.
    if g.street != Street::HandOver {
        let a = poker.seat_angles[g.to_act];
        commands.spawn((
            Mesh3d(assets.ring_mesh.clone()),
            MeshMaterial3d(assets.ring_mat.clone()),
            Transform::from_xyz(a.cos() * poker.rx * 0.72, ft + 0.012, a.sin() * poker.rz * 0.72)
                .with_scale(Vec3::new(0.62, 1.0, 0.62)),
            TableProp,
        ));
    }
}

fn face_material(
    faces: &mut CardFaces,
    materials: &mut Assets<StandardMaterial>,
    asset_server: &AssetServer,
    code: &str,
) -> Handle<StandardMaterial> {
    if let Some(h) = faces.0.get(code) {
        return h.clone();
    }
    let h = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(asset_server.load(format!("cards/{code}.png"))),
        // Unlit so the rank/suit colours read true (no cool-light tint).
        unlit: true,
        ..default()
    });
    faces.0.insert(code.to_string(), h.clone());
    h
}

/// A small banded chip stack whose height scales with the chip amount.
/// Spawn a table card lying flat on the felt with a subtle soft shadow peeking
/// out beneath it. `flat` is the lie-flat (face-up) orientation.
fn spawn_table_card(
    commands: &mut Commands,
    assets: &PokerAssets,
    mat: Handle<StandardMaterial>,
    flat: Quat,
    x: f32,
    z: f32,
    felt_top: f32,
    scale: f32,
) {
    // Soft shadow, slightly larger than the card and nudged away from the
    // camera so a thin edge shows around the card on the felt.
    commands.spawn((
        Mesh3d(assets.card_quad.clone()),
        MeshMaterial3d(assets.shadow_mat.clone()),
        Transform::from_xyz(x + 0.05, felt_top + 0.012, z - 0.08)
            .with_rotation(flat)
            .with_scale(Vec3::new(scale * 1.12, 1.0, scale * 1.12)),
        TableProp,
    ));
    // The card, lying flat on the felt.
    commands.spawn((
        Mesh3d(assets.card_quad.clone()),
        MeshMaterial3d(mat),
        Transform::from_xyz(x, felt_top + 0.022, z)
            .with_rotation(flat)
            .with_scale(Vec3::splat(scale)),
        TableProp,
    ));
}

fn spawn_chips(
    commands: &mut Commands,
    assets: &PokerAssets,
    x: f32,
    z: f32,
    felt_top: f32,
    amount: u32,
    color: usize,
) {
    let n = ((amount as f32).sqrt() * 0.7).round().clamp(1.0, 16.0) as usize;
    let base = color % assets.chip_mats.len();
    let accent = (color + 2) % assets.chip_mats.len();
    for k in 0..n {
        let mat = if k % 4 == 3 {
            assets.chip_mats[accent].clone()
        } else {
            assets.chip_mats[base].clone()
        };
        commands.spawn((
            Mesh3d(assets.chip_mesh.clone()),
            MeshMaterial3d(mat),
            Transform::from_xyz(x, felt_top + 0.022 + k as f32 * 0.045, z)
                .with_scale(Vec3::new(0.24, 0.04, 0.24)),
            TableProp,
        ));
    }
}

/// Update the HUD text every frame from the game state.
fn hud(
    poker: Res<Poker>,
    mut q: Query<&mut Text, (With<HudText>, Without<WinBanner>)>,
    mut banner: Query<&mut Text, (With<WinBanner>, Without<HudText>)>,
) {
    let g = &poker.game;
    let mut s = format!("{}    Pot ${}\n\n", street_name(g.street), g.pot());
    for (i, p) in g.players.iter().enumerate() {
        let turn = if i == g.to_act && g.street != Street::HandOver {
            ">"
        } else {
            " "
        };
        let dealer = if i == g.button { " (D)" } else { "" };
        let status = if p.folded {
            "  folded"
        } else if p.all_in {
            "  all-in"
        } else {
            ""
        };
        let bet = if p.bet > 0 {
            format!("  bet ${}", p.bet)
        } else {
            String::new()
        };
        s += &format!("{turn} {:<9} ${:<5}{bet}{dealer}{status}\n", p.name, p.stack);
    }
    s += &format!("\n{}", poker.log);
    for mut text in &mut q {
        *text = Text::new(s.clone());
    }

    // Big centre banner at showdown.
    let banner_text = if g.street == Street::HandOver {
        g.last_payouts
            .iter()
            .map(|pay| {
                let p = &g.players[pay.seat];
                let verb = if p.is_human { "win" } else { "wins" };
                format!("{} {verb} ${}", p.name, pay.amount)
            })
            .collect::<Vec<_>>()
            .join("   ·   ")
    } else {
        String::new()
    };
    for mut text in &mut banner {
        *text = Text::new(banner_text.clone());
    }
}

/// Float each seated player's stack above their head (screen-space, projected
/// from the world each frame).
fn money_labels(
    poker: Res<Poker>,
    camera: Query<(&Camera, &GlobalTransform)>,
    mut labels: Query<(&MoneyLabel, &mut Node, &mut Text, &mut Visibility)>,
) {
    let Ok((cam, cam_t)) = camera.single() else {
        return;
    };
    let g = &poker.game;
    for (label, mut node, mut text, mut vis) in &mut labels {
        let s = label.0;
        let player = &g.players[s];
        // Busted players have left the table — drop their label entirely.
        if player.busted() {
            *vis = Visibility::Hidden;
            continue;
        }
        let a = poker.seat_angles[s];
        let head = Vec3::new(a.cos() * poker.prx, 3.7, a.sin() * poker.prz);
        match cam.world_to_viewport(cam_t, head) {
            Ok(p) => {
                node.left = Val::Px(p.x - 28.0);
                node.top = Val::Px(p.y);
                let tag = if player.all_in {
                    " all-in"
                } else if player.folded {
                    " (folded)"
                } else {
                    ""
                };
                *text = Text::new(format!("${}{}", player.stack, tag));
                *vis = Visibility::Visible;
            }
            Err(_) => *vis = Visibility::Hidden,
        }
    }
}

fn street_name(s: Street) -> &'static str {
    match s {
        Street::Preflop => "Preflop",
        Street::Flop => "Flop",
        Street::Turn => "Turn",
        Street::River => "River",
        Street::Showdown => "Showdown",
        Street::HandOver => "Hand over",
    }
}
