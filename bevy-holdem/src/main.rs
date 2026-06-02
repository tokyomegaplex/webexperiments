//! Cartoon Hold'em — Bevy 3D (native desktop), phase 1: the table scene.
//!
//! This sets up a real 3D scene — angled perspective camera, lighting, a felt
//! table with a wooden rail, the 5 characters seated around it as upright
//! "billboard" standees (PNG textures with colored fallback), the human's seat
//! at the front, and a row of community-card slots.
//!
//! The poker rules + AI + betting UI are the next phase; this proves the 3D
//! approach and the art pipeline. Run with `cargo run` (see README).

use bevy::prelude::*;

mod characters;
use characters::roster;

/// Marks a quad that should always face the camera (kept upright).
#[derive(Component)]
struct Billboard;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Cartoon Hold'em (Bevy)".into(),
                resolution: (1100.0, 760.0).into(),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(Color::srgb(0.03, 0.05, 0.04)))
        .add_systems(Startup, setup)
        .add_systems(Update, billboard_system)
        .run();
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
) {
    // --- camera: looking down at the felt from the player's side ---
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 6.5, 8.5).looking_at(Vec3::new(0.0, 0.3, -0.6), Vec3::Y),
    ));

    // --- lighting ---
    commands.spawn((
        DirectionalLight {
            illuminance: 7000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(5.0, 12.0, 6.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.insert_resource(AmbientLight {
        color: Color::WHITE,
        brightness: 350.0,
        ..default()
    });

    let table_radius = 4.0;

    // --- felt (a flattened cylinder) ---
    commands.spawn((
        Mesh3d(meshes.add(Cylinder::new(table_radius, 0.4))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb_u8(22, 120, 76),
            perceptual_roughness: 0.95,
            ..default()
        })),
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));

    // --- wooden rail (slightly larger, darker, just below) ---
    commands.spawn((
        Mesh3d(meshes.add(Cylinder::new(table_radius + 0.35, 0.34))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb_u8(64, 40, 22),
            perceptual_roughness: 0.7,
            ..default()
        })),
        Transform::from_xyz(0.0, -0.16, 0.0),
    ));

    // --- floor ---
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(60.0, 60.0))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb_u8(14, 28, 20),
            ..default()
        })),
        Transform::from_xyz(0.0, -0.45, 0.0),
    ));

    // --- characters: seated around the back arc as billboards ---
    let chars = roster();
    let n = chars.len();
    let quad = meshes.add(Rectangle::new(2.0, 2.6));
    for (i, c) in chars.iter().enumerate() {
        // Spread evenly across the rear arc (roughly 200deg .. 340deg).
        let t = if n > 1 { i as f32 / (n - 1) as f32 } else { 0.5 };
        let angle = std::f32::consts::PI * (1.18 + 0.64 * t);
        let r = table_radius + 1.15;
        let x = angle.cos() * r;
        let z = angle.sin() * r;

        let material = materials.add(StandardMaterial {
            base_color: c.color,
            base_color_texture: Some(asset_server.load(c.file)),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            double_sided: true,
            cull_mode: None,
            ..default()
        });

        commands.spawn((
            Mesh3d(quad.clone()),
            MeshMaterial3d(material),
            Transform::from_xyz(x, 1.3, z),
            Billboard,
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
    for i in 0..5 {
        let x = (i as f32 - 2.0) * 0.86;
        commands.spawn((
            Mesh3d(card.clone()),
            MeshMaterial3d(card_mat.clone()),
            Transform::from_xyz(x, 0.22, -0.2),
        ));
    }

    // --- the human's two hole cards, near the front edge ---
    for i in 0..2 {
        let x = (i as f32 - 0.5) * 0.86;
        commands.spawn((
            Mesh3d(card.clone()),
            MeshMaterial3d(card_mat.clone()),
            Transform::from_xyz(x, 0.22, 3.0),
        ));
    }
}

/// Rotate every billboard to face the camera, but only around Y so the
/// standees stay upright (like cardboard cutouts at a table).
fn billboard_system(
    camera: Query<&Transform, (With<Camera3d>, Without<Billboard>)>,
    mut billboards: Query<&mut Transform, With<Billboard>>,
) {
    let Some(cam) = camera.iter().next() else {
        return;
    };
    let cam_pos = cam.translation;
    for mut t in &mut billboards {
        let mut target = cam_pos;
        target.y = t.translation.y; // yaw only — keep them upright
        t.look_at(target, Vec3::Y);
    }
}
