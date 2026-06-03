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
use bevy::light::NotShadowCaster;
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
    let cam_pos = Vec3::new(0.0, 4.7, 10.4);
    commands.spawn((
        Camera3d::default(),
        Tonemapping::None,
        Transform::from_translation(cam_pos).looking_at(Vec3::new(0.0, 1.8, -2.0), Vec3::Y),
        AmbientLight {
            color: Color::srgb(0.78, 0.82, 1.0),
            brightness: 280.0,
            ..default()
        },
    ));

    // --- lighting: warm key + cool fill + an overhead lamp pool ---
    commands.spawn((
        DirectionalLight {
            color: Color::srgb(1.0, 0.96, 0.88),
            illuminance: 3500.0,
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
    // Warm hanging lamp casting a pool of light onto the felt (the mood key).
    commands.spawn((
        SpotLight {
            intensity: 8_000_000.0,
            color: Color::srgb(1.0, 0.89, 0.7),
            shadows_enabled: true,
            range: 45.0,
            outer_angle: 0.7,
            inner_angle: 0.38,
            ..default()
        },
        Transform::from_xyz(0.0, 9.5, -0.3).looking_at(Vec3::new(0.0, felt_top, -0.5), Vec3::Y),
    ));
    // Hanging lamp shade (apex down) + a glowing bulb, for atmosphere.
    commands.spawn((
        Mesh3d(meshes.add(Cone { radius: 1.2, height: 1.3 })),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb_u8(30, 26, 22),
            emissive: LinearRgba::rgb(0.25, 0.18, 0.08),
            perceptual_roughness: 0.5,
            ..default()
        })),
        Transform::from_xyz(0.0, 6.4, -0.4).with_rotation(Quat::from_rotation_x(std::f32::consts::PI)),
        NotShadowCaster,
    ));
    commands.spawn((
        Mesh3d(meshes.add(Sphere::new(0.22))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.92, 0.7),
            emissive: LinearRgba::rgb(3.0, 2.4, 1.2),
            ..default()
        })),
        Transform::from_xyz(0.0, 5.95, -0.4),
        NotShadowCaster,
    ));

    // --- table top (felt), raised to real table height ---
    commands.spawn((
        Mesh3d(meshes.add(Cylinder::new(1.0, 1.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb_u8(30, 135, 86),
            perceptual_roughness: 0.9,
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
            base_color: Color::srgb_u8(14, 18, 26),
            perceptual_roughness: 1.0,
            ..default()
        })),
        Transform::from_xyz(0.0, 12.0, -16.0),
    ));

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
    let card_y = felt_top + 0.02;

    // Spawn a small stack of `n` chips centered at (x, z).
    let chip_stack = |commands: &mut Commands, x: f32, z: f32, n: usize, mat: usize| {
        for k in 0..n {
            commands.spawn((
                Mesh3d(disc.clone()),
                MeshMaterial3d(chip_mats[mat % chip_mats.len()].clone()),
                Transform::from_xyz(x, card_y + 0.022 + k as f32 * 0.045, z)
                    .with_scale(Vec3::new(0.24, 0.04, 0.24)),
            ));
        }
    };

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

        let material = materials.add(StandardMaterial {
            // White base so the PNG shows its true colors (no tint); now lit so
            // the standees sit in the scene's lighting (the lamp pool).
            base_color: Color::WHITE,
            base_color_texture: Some(asset_server.load(c.file)),
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

        // This player's chip stacks, just inside the rail in front of them.
        let cxp = angle.cos() * rx * 0.64;
        let czp = angle.sin() * rz * 0.64;
        chip_stack(&mut commands, cxp, czp, 4 + i % 3, i);
        chip_stack(&mut commands, cxp + 0.32, czp + 0.04, 3 + i % 2, (i + 2) % 5);

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
        ));
    }

    // --- the pot, in the middle ---
    chip_stack(&mut commands, -0.3, 0.5, 6, 0);
    chip_stack(&mut commands, 0.1, 0.55, 5, 1);
    chip_stack(&mut commands, -0.1, 0.2, 4, 3);

    // --- community cards on the felt (flat quads with real face textures) ---
    let card_face_quad = meshes.add(Rectangle::new(0.78, 1.08));
    // Lie flat (face up), top edge toward the camera so ranks read right-side-up.
    let flat = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)
        * Quat::from_rotation_z(std::f32::consts::PI);
    let faces = ["cards/As.png", "cards/Kh.png", "cards/Qd.png", "cards/Jc.png", "cards/Ts.png"];
    for (i, face) in faces.iter().enumerate() {
        let x = (i as f32 - 2.0) * 0.88;
        let m = materials.add(StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(asset_server.load(*face)),
            perceptual_roughness: 0.5,
            ..default()
        });
        commands.spawn((
            Mesh3d(card_face_quad.clone()),
            MeshMaterial3d(m),
            Transform::from_xyz(x, card_y, -0.6).with_rotation(flat),
        ));
    }

    // --- the human's two hole cards (face down), near the front edge ---
    let card = meshes.add(Cuboid::new(0.72, 0.04, 1.02));
    let card_back_mat = materials.add(StandardMaterial {
        base_color: Color::srgb_u8(150, 40, 52),
        perceptual_roughness: 0.5,
        ..default()
    });
    for i in 0..2 {
        let x = (i as f32 - 0.5) * 0.84;
        commands.spawn((
            Mesh3d(card.clone()),
            MeshMaterial3d(card_back_mat.clone()),
            Transform::from_xyz(x, card_y, 2.7),
        ));
    }
}

/// Each frame: face the camera (yaw only, stays upright) and apply a very
/// subtle, stepped (~4fps) noise wobble in position, lean, and scale so the
/// standees feel hand-animated / "boiling" like stop-motion, not dead-still.
fn standee_system(
    time: Res<Time>,
    camera: Query<&Transform, (With<Camera3d>, Without<Standee>)>,
    mut standees: Query<(&Standee, &mut Transform)>,
) {
    let Some(cam) = camera.iter().next() else {
        return;
    };
    let cam_pos = cam.translation;

    // Quantize time to ~4fps so the motion is stepped (stop-motion), not smooth.
    let step = (time.elapsed_secs() * 4.0).floor();

    for (s, mut t) in &mut standees {
        // Stepped pseudo-noise in [-1, 1], unique per standee via its seed.
        let nx = hash11(s.seed * 1.3 + step * 0.0137) * 2.0 - 1.0;
        let ny = hash11(s.seed * 2.1 + step * 0.0211) * 2.0 - 1.0;
        let nlean = hash11(s.seed * 3.7 + step * 0.0090) * 2.0 - 1.0;
        let nsc = hash11(s.seed * 5.2 + step * 0.0051) * 2.0 - 1.0;

        // Very subtle wobble amounts.
        let pos = s.base + Vec3::new(nx * 0.011, ny * 0.009, 0.0);
        t.translation = pos;

        // Face the camera, yaw only (target at the standee's own height).
        let target = Vec3::new(cam_pos.x, pos.y, cam_pos.z);
        t.look_at(target, Vec3::Y);

        // A tiny lean + scale pulse on top of the facing rotation.
        t.rotate_local_z(nlean * 0.005);
        t.scale = s.base_scale * (1.0 + nsc * 0.004);
    }
}

/// Cheap deterministic hash → pseudo-noise in [0, 1).
fn hash11(x: f32) -> f32 {
    let v = (x * 127.1).sin() * 43758.5453;
    (v - v.floor()).abs()
}
