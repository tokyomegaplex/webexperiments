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
        // HDR + bloom so the lamp bulb (and its emissive shade) actually glow.
        Hdr,
        Tonemapping::None,
        Bloom { intensity: 0.14, ..Bloom::NATURAL },
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

    // --- background bar: a back cabinet, a counter, and rows of bottles ---
    let bar_z = -11.5;
    let dark_wood = materials.add(StandardMaterial {
        base_color: Color::srgb_u8(34, 22, 16),
        perceptual_roughness: 0.7,
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
            base_color: Color::srgb_u8(48, 30, 20),
            perceptual_roughness: 0.35,
            reflectance: 0.4,
            ..default()
        })),
        Transform::from_xyz(0.0, floor_y + 1.55, bar_z + 1.4),
    ));
    // bottles: muted glass, varied sizes, each with a small paper label.
    let bottle_cols = [
        Color::srgb_u8(40, 66, 46),    // dark green
        Color::srgb_u8(74, 50, 30),    // brown
        Color::srgb_u8(120, 124, 118), // smoke / clear
        Color::srgb_u8(96, 66, 30),    // amber
        Color::srgb_u8(70, 42, 40),    // dark red-brown
    ];
    let bottle_mats: Vec<_> = bottle_cols
        .iter()
        .map(|c| {
            let lin = c.to_linear();
            materials.add(StandardMaterial {
                base_color: *c,
                emissive: LinearRgba::rgb(lin.red * 0.05, lin.green * 0.05, lin.blue * 0.05),
                perceptual_roughness: 0.25,
                reflectance: 0.45,
                ..default()
            })
        })
        .collect();
    let label_mat = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        base_color_texture: Some(asset_server.load("label.png")),
        alpha_mode: AlphaMode::Blend,
        unlit: true,
        ..default()
    });
    let bz = bar_z - 0.45;
    for (si, shelf_top) in [floor_y + 1.56, floor_y + 2.76].iter().enumerate() {
        let mut bx = -9.6;
        let mut k = si * 2;
        while bx < 9.6 {
            let mat = bottle_mats[k % bottle_mats.len()].clone();
            // vary the silhouette: height, body radius, neck length.
            let h = 0.46 + (k % 4) as f32 * 0.13; // 0.46 .. 0.85
            let rad = 0.1 + (k % 3) as f32 * 0.022; // 0.10 .. 0.144
            let neck_h = 0.22 + (k % 2) as f32 * 0.12;
            let cy = shelf_top + h / 2.0;
            commands.spawn((
                Mesh3d(meshes.add(Cylinder::new(rad, h))),
                MeshMaterial3d(mat.clone()),
                Transform::from_xyz(bx, cy, bz),
                NotShadowCaster,
            ));
            commands.spawn((
                Mesh3d(meshes.add(Cylinder::new(rad * 0.4, neck_h))),
                MeshMaterial3d(mat),
                Transform::from_xyz(bx, cy + h / 2.0 + neck_h / 2.0 - 0.01, bz),
                NotShadowCaster,
            ));
            // paper label on the front face (toward the camera)
            commands.spawn((
                Mesh3d(meshes.add(Rectangle::new(rad * 1.5, h * 0.5))),
                MeshMaterial3d(label_mat.clone()),
                Transform::from_xyz(bx, cy, bz + rad + 0.006),
                NotShadowCaster,
            ));
            bx += 0.62;
            k += 1;
        }
    }
    // a warm wash on the back bar (limited range so nothing clips)
    commands.spawn((
        PointLight {
            intensity: 900_000.0,
            color: Color::srgb(1.0, 0.82, 0.55),
            range: 16.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_xyz(0.0, floor_y + 2.4, bar_z + 1.0),
    ));

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
    for sx in [-6.5_f32, -2.2, 2.2, 6.5] {
        let sz = bar_z + 2.7;
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
    let chair_back_mesh = meshes.add(Cuboid::new(1.5, 1.2, 0.12));
    let chair_post_mesh = meshes.add(Cylinder::new(0.07, 2.4));

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

    // Spawn a small stack of `n` chips centered at (x, z). Every 4th chip uses
    // a contrasting colour so the stacks read as banded denominations.
    let chip_stack = |commands: &mut Commands, x: f32, z: f32, n: usize, mat: usize| {
        let base = mat % chip_mats.len();
        let accent = (mat + 2) % chip_mats.len();
        for k in 0..n {
            let m = if k % 4 == 3 { &chip_mats[accent] } else { &chip_mats[base] };
            commands.spawn((
                Mesh3d(disc.clone()),
                MeshMaterial3d(m.clone()),
                Transform::from_xyz(x, card_y + 0.022 + k as f32 * 0.045, z)
                    .with_scale(Vec3::new(0.24, 0.04, 0.24)),
            ));
        }
    };

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

        // A chair behind each player: a leather backrest on two posts, facing
        // the table so it reads as the seat they're sitting in.
        let cback = Vec3::new(angle.cos() * (prx + 0.5), floor_y + 2.0, angle.sin() * (prz + 0.7));
        let yaw = (-angle.cos()).atan2(-angle.sin());
        let crot = Quat::from_rotation_y(yaw);
        let cright = crot * Vec3::X;
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
                    Vec3::new(cback.x, floor_y + 1.3, cback.z) + cright * (0.66 * s),
                )
                .with_rotation(crot),
            ));
        }

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

    // --- the pot, in the middle: a banded mound of varied stacks ---
    chip_stack(&mut commands, -0.3, 0.5, 8, 0);
    chip_stack(&mut commands, 0.12, 0.56, 6, 1);
    chip_stack(&mut commands, -0.08, 0.18, 5, 3);
    chip_stack(&mut commands, 0.34, 0.26, 4, 4);
    chip_stack(&mut commands, -0.42, 0.16, 3, 2);

    // --- dealer button, lying flat on the felt near a player ---
    commands.spawn((
        Mesh3d(disc.clone()),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(asset_server.load("button_d.png")),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        })),
        Transform::from_xyz(-1.7, card_y + 0.01, 1.0)
            .with_scale(Vec3::new(0.34, 0.04, 0.34)),
    ));

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

    // --- burn card: a lone face-down card beside the flop ---
    commands.spawn((
        Mesh3d(card.clone()),
        MeshMaterial3d(card_back_mat.clone()),
        Transform::from_xyz(-2.75, card_y, -0.6).with_rotation(Quat::from_rotation_y(0.14)),
    ));

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
        commands.spawn((
            Mesh3d(meshes.add(Cylinder::new(0.028, 0.05))),
            MeshMaterial3d(cig_ember.clone()),
            Transform::from_translation(base + dir * 0.33).with_rotation(rot),
            NotShadowCaster,
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
