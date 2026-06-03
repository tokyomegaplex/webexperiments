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
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use std::env;

mod characters;
use characters::roster;

/// A character standee: always faces the camera (yaw only) and gets a subtle,
/// stepped (~12fps) noise wobble so it feels hand-animated / alive.
#[derive(Component)]
struct Standee {
    base: Vec3,
    base_scale: Vec3,
    seed: f32,
}

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
    .add_systems(Startup, setup)
    .add_systems(Update, standee_system);

    if let Ok(path) = env::var("SCREENSHOT") {
        app.insert_resource(ShotState { path, frame: 0 })
            .add_systems(Update, screenshot_system);
    }

    app.run();
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
    commands.spawn((
        Camera3d::default(),
        Tonemapping::None,
        Transform::from_xyz(0.0, 4.7, 10.4).looking_at(Vec3::new(0.0, 1.8, -2.0), Vec3::Y),
        AmbientLight {
            color: Color::srgb(0.9, 0.92, 1.0),
            brightness: 700.0,
            ..default()
        },
    ));

    // --- lighting: warm key + cool fill ---
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(1.0, 0.96, 0.88),
            illuminance: 8500.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(6.0, 12.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(0.7, 0.8, 1.0),
            illuminance: 2500.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_xyz(-7.0, 6.0, -4.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    // --- table top (felt), raised to real table height ---
    commands.spawn((
        Mesh3d(meshes.add(Cylinder::new(1.0, 1.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb_u8(26, 124, 80),
            perceptual_roughness: 0.95,
            ..default()
        })),
        Transform::from_xyz(0.0, felt_top - 0.15, 0.0).with_scale(Vec3::new(rx, 0.3, rz)),
    ));

    // --- padded rail (a larger, darker oval at the felt edge) ---
    commands.spawn((
        Mesh3d(meshes.add(Cylinder::new(1.0, 1.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb_u8(74, 46, 26),
            perceptual_roughness: 0.6,
            ..default()
        })),
        Transform::from_xyz(0.0, felt_top - 0.22, 0.0).with_scale(Vec3::new(rx + 0.4, 0.34, rz + 0.4)),
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

    // --- floor ---
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(80.0, 80.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb_u8(18, 26, 22),
            ..default()
        })),
        Transform::from_xyz(0.0, -0.5, 0.0),
    ));

    // --- backdrop wall (kills the black void; lit for a soft gradient) ---
    commands.spawn((
        Mesh3d(meshes.add(Rectangle::new(90.0, 44.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb_u8(28, 40, 52),
            perceptual_roughness: 1.0,
            ..default()
        })),
        Transform::from_xyz(0.0, 12.0, -16.0),
    ));

    // --- characters: seated around the back + sides as upright standees ---
    // Front-center (toward the camera) is left open for the human.
    let chars = roster();
    let quad_w = 3.3_f32;
    let quad_h = 4.2_f32;
    let quad = meshes.add(Rectangle::new(quad_w, quad_h));
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

        let material = materials.add(StandardMaterial {
            // White base so the PNG shows its true colors (no tint); `color`
            // stays only as a conceptual fallback and is not multiplied in.
            base_color: Color::WHITE,
            base_color_texture: Some(asset_server.load(c.file)),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
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
            Standee {
                base,
                base_scale: Vec3::ONE,
                seed: i as f32 * 1.7 + 0.3,
            },
            Name::new(c.name),
        ));
    }

    // --- community card slots on the felt ---
    let card = meshes.add(Cuboid::new(0.72, 0.04, 1.02));
    let card_mat = materials.add(StandardMaterial {
        base_color: Color::srgb_u8(250, 250, 244),
        perceptual_roughness: 0.6,
        ..default()
    });
    let card_y = felt_top + 0.02;
    for i in 0..5 {
        let x = (i as f32 - 2.0) * 0.9;
        commands.spawn((
            Mesh3d(card.clone()),
            MeshMaterial3d(card_mat.clone()),
            Transform::from_xyz(x, card_y, -0.4),
        ));
    }

    // --- the human's two hole cards, near the front edge ---
    for i in 0..2 {
        let x = (i as f32 - 0.5) * 0.9;
        commands.spawn((
            Mesh3d(card.clone()),
            MeshMaterial3d(card_mat.clone()),
            Transform::from_xyz(x, card_y, 2.6),
        ));
    }
}

/// Each frame: face the camera (yaw only, stays upright) and apply a subtle,
/// stepped (~12fps) noise wobble in position, lean, and scale so the standees
/// feel hand-animated / "boiling" like stop-motion rather than dead-still.
fn standee_system(
    time: Res<Time>,
    camera: Query<&Transform, (With<Camera3d>, Without<Standee>)>,
    mut standees: Query<(&Standee, &mut Transform)>,
) {
    let Some(cam) = camera.iter().next() else {
        return;
    };
    let cam_pos = cam.translation;

    // Quantize time to ~12fps so the motion is stepped (stop-motion), not smooth.
    let step = (time.elapsed_secs() * 12.0).floor();

    for (s, mut t) in &mut standees {
        // Stepped pseudo-noise in [-1, 1], unique per standee via its seed.
        let nx = hash11(s.seed * 1.3 + step * 0.0137) * 2.0 - 1.0;
        let ny = hash11(s.seed * 2.1 + step * 0.0211) * 2.0 - 1.0;
        let nlean = hash11(s.seed * 3.7 + step * 0.0090) * 2.0 - 1.0;
        let nsc = hash11(s.seed * 5.2 + step * 0.0051) * 2.0 - 1.0;

        // Subtle wobble amounts.
        let pos = s.base + Vec3::new(nx * 0.05, ny * 0.04, 0.0);
        t.translation = pos;

        // Face the camera, yaw only (target at the standee's own height).
        let target = Vec3::new(cam_pos.x, pos.y, cam_pos.z);
        t.look_at(target, Vec3::Y);

        // A tiny lean + scale pulse on top of the facing rotation.
        t.rotate_local_z(nlean * 0.025);
        t.scale = s.base_scale * (1.0 + nsc * 0.02);
    }
}

/// Cheap deterministic hash → pseudo-noise in [0, 1).
fn hash11(x: f32) -> f32 {
    let v = (x * 127.1).sin() * 43758.5453;
    (v - v.floor()).abs()
}
