//! Cartoon Hold'em — Bevy 3D (native desktop), phase 1: the table scene.
//!
//! This sets up a real 3D scene — angled perspective camera, lighting, a felt
//! table with a wooden rail, the 5 characters seated around it as upright
//! "billboard" standees (PNG textures with colored fallback), the human's seat
//! at the front, and a row of community-card slots.
//!
//! The poker rules + AI + betting UI are the next phase; this proves the 3D
//! approach and the art pipeline. Run with `cargo run` (see README).

use bevy::audio::{PlaybackMode, Volume};
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
    /// Seconds left of a little squash-and-stretch hop (set when they speak).
    bounce: f32,
    /// Seconds left of the subtle squash-stretch pulse played on frame switches.
    squash: f32,
}

/// Per-character animation frames, discovered from `characters/<id>/` by
/// filename: `*default*` (idle, randomly cycled), `*talk*` (while speaking),
/// `*back*` (walking to the bar after busting), `*sit*` (seated at the bar).
/// Falls back to the single base sprite when a folder/frame is missing.
#[derive(Component)]
struct CharAnim {
    default_mats: Vec<Handle<StandardMaterial>>,
    talk_mats: Vec<Handle<StandardMaterial>>,
    back_mat: Option<Handle<StandardMaterial>>,
    sit_mat: Option<Handle<StandardMaterial>>,
    /// Index into default_mats currently showing.
    cur: usize,
    /// Wiggle step at which to switch to a new random default frame.
    next_switch: f32,
    /// While `now < talk_until`, show the chosen talk frame.
    talk_until: f32,
    talk_idx: usize,
}

/// Tags a per-seat scene visual (name plate) so it can be hidden when the
/// player busts and leaves.
#[derive(Component)]
struct SeatVisual(usize);

/// One of the two bottom-left UI hole-card images for the human. `t` ramps
/// 0->1 to slide the card from the table up to its resting corner spot.
#[derive(Component)]
struct HoleCardUi {
    slot: usize,
    t: f32,
}

/// The bottom-right "Your money" counter.
#[derive(Component)]
struct MyMoney;

/// The container holding the human's action buttons (shown on your turn).
#[derive(Component)]
struct BettingBar;

/// Container + button for advancing to the next hand at showdown.
#[derive(Component)]
struct NextRoundBar;
#[derive(Component)]
struct NextButton;

/// What Bubble the bartender is doing right now.
#[derive(Clone, Copy, PartialEq, Debug)]
enum BarState {
    /// Facing the room (front sprite), idling.
    Front,
    /// Turned around to fiddle with the bar (back sprite).
    Bar,
    /// Strolling along the bar (side sprites).
    WalkLeft,
    WalkRight,
}

/// Bubble the bartender: a billboard that faces the camera with an idle wobble,
/// and a little life of his own — he randomly idles, turns to the bar, and
/// strolls left/right behind the counter (see `bar_standee`).
#[derive(Component)]
struct BarStandee {
    base: Vec3,
    seed: f32,
    state: BarState,
    /// When to pick the next state.
    state_until: f32,
    /// Walk segment: x from→to over walk_start→state_until.
    walk_from: f32,
    walk_to: f32,
    walk_start: f32,
    /// Seconds left of the squash-and-stretch pulse played on a state switch.
    squash: f32,
    /// Materials per facing: [front, back, left, right].
    mats: [Handle<StandardMaterial>; 4],
}

/// The single bobbing arrow that points at whoever's turn it is.
#[derive(Component)]
struct TurnArrow;

/// A chip flying from a player into the pot when they bet. Thrown with a
/// parabolic arc, a tumble, and a small settle-bounce for a bit of fake physics.
#[derive(Component)]
struct FlyingChip {
    from: Vec3,
    to: Vec3,
    start: f32,
    dur: f32,
    arc: f32,       // peak height of the throw
    spin: f32,      // total tumble (radians) over the flight; lands flat
    axis: Vec3,     // tumble axis (perpendicular to the throw, in the table plane)
}

/// A chip that has landed in the pot and now rests in the middle of the table.
/// These persist (they're not `TableProp`, so a redraw doesn't wipe them) and
/// accumulate as players bet; they're cleared when the next hand is dealt.
#[derive(Component)]
struct PotChip;

/// A chip animating its intro drop: rains down from above to `rest_y`.
#[derive(Component)]
struct ChipDrop {
    rest_y: f32,
    start: f32,
    dur: f32,
}

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

/// The HUD detail panel; hidden unless Tab is held.
#[derive(Component)]
struct HudPanel;

/// World-anchored "Pot $X" label drawn just under the pot chips.
#[derive(Component)]
struct PotLabel;

/// The transient last-action line at the top ("Hoodguy folds"): fades in,
/// drifts upward, fades out.
#[derive(Component)]
struct ActionTicker;

/// The main 3D camera (so the spectate view can move it to the bar).
#[derive(Component)]
struct MainCamera;

/// Tags entities that only exist in the penthouse environment (the city window
/// wall, the glossy floor) so a system can show/hide them with the room.
#[derive(Component)]
struct EnvPenthouse;

/// A name plate floating over a standee; re-oriented every frame to face the
/// live camera (text side toward the camera so it never reads mirrored).
#[derive(Component)]
struct NamePlate;

/// The warm winner glow billboard; faces the live camera each frame so it reads
/// from any viewpoint (anchored at the winner's standee).
#[derive(Component)]
struct WinGlow {
    anchor: Vec3,
}

/// The beer that sits in front of you while you spectate from the bar (a 3D prop
/// pinned in camera space, shown only when busted).
#[derive(Component)]
struct SpectateBeer;

/// One piece of the chair behind a standee. The whole chair swivels around the
/// standee to stay directly behind it *as seen from the live camera*, so the
/// flat billboard never clips through it from any viewpoint.
#[derive(Component)]
struct ChairPart {
    /// The standee's floor anchor (x, floor_y, z).
    anchor: Vec3,
    /// 0 = backrest, ±1 = the two posts.
    post: f32,
    is_post: bool,
}

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
    /// Time between AI actions.
    act_timer: Timer,
    /// When true the auto-play loop is frozen (used for deterministic
    /// screenshots so the captured frame matches the prepared state).
    paused: bool,
    /// A fresh hand needs its deal animation kicked off.
    pending_deal: bool,
    /// The deal animation is currently playing (hole cards hidden until done).
    dealing: bool,
    /// Absolute times (secs): when cards start sliding, and when the deal ends.
    deal_start: f32,
    deal_end: f32,
    /// Last seat we played a turn voice for (so we only play on a change).
    last_actor: usize,
    /// 0..1 ramp that stands the cards up at showdown.
    showdown_raise: f32,
    /// How many community cards have finished their deal-in animation.
    comm_shown: usize,
    /// A community-card deal-in animation is playing; ends at `comm_end`.
    comm_anim: bool,
    comm_end: f32,
    /// True once we've played the winner's celebration this showdown.
    celebrated: bool,
    log: String,
}

/// A card sliding from the deck out to a seat during the deal animation.
#[derive(Component)]
struct DealingCard {
    from: Vec3,
    to: Vec3,
    start: f32,
    dur: f32,
    scale: f32,
    played: bool,
}

/// A card in the face-down deck stack (jiggled during the shuffle).
#[derive(Component)]
struct DeckCard {
    base: Vec3,
}

/// Shared meshes/materials the redraw system uses for cards and chips.
#[derive(Resource)]
struct PokerAssets {
    card_quad: Handle<Mesh>,
    chip_mesh: Handle<Mesh>,
    ring_mesh: Handle<Mesh>,
    arrow_mesh: Handle<Mesh>,
    card_back: Handle<StandardMaterial>,
    button_mat: Handle<StandardMaterial>,
    ring_mat: Handle<StandardMaterial>,
    shadow_mat: Handle<StandardMaterial>,
    glow_mat: Handle<StandardMaterial>,
    chip_mats: Vec<Handle<StandardMaterial>>,
}

/// A floating screen-space label showing a seated player's stack, above their
/// head (seat index into the game's players).
#[derive(Component)]
struct MoneyLabel(usize);

/// The big centre-screen banner shown at showdown (text + its framed root).
#[derive(Component)]
struct WinBanner;
#[derive(Component)]
struct WinBannerRoot;

/// Card-face materials, cached by code ("As") so we don't leak one per redraw.
#[derive(Resource, Default)]
struct CardFaces(HashMap<String, Handle<StandardMaterial>>);

/// Sound effects, discovered from the `sfx/` folder at startup. Lists are
/// chosen from at random; missing files fall back to the generated ones.
#[derive(Resource)]
struct Sfx {
    chips: Vec<Handle<AudioSource>>,
    card: Handle<AudioSource>,
    knock: Handle<AudioSource>,
    deal: Vec<Handle<AudioSource>>,
    win_small: Vec<Handle<AudioSource>>,
    win_medium: Vec<Handle<AudioSource>>,
    win_big: Vec<Handle<AudioSource>>,
    /// Per-seat character voices (seat index; seat 0 = human, usually empty).
    chars: Vec<Vec<Handle<AudioSource>>>,
}

/// Small RNG for picking/jittering sound effects (independent of the deck RNG).
/// Also carries the live SFX master volume + on/off so every `play_sfx` honours
/// the audio settings without threading a separate resource everywhere.
#[derive(Resource)]
struct SfxRng {
    state: u64,
    sfx_vol: f32,
    sfx_on: bool,
}

impl SfxRng {
    fn new(seed: u64) -> Self {
        SfxRng {
            state: seed,
            sfx_vol: 0.8,
            sfx_on: true,
        }
    }
    fn next_u32(&mut self) -> u32 {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.state >> 33) as u32
    }
    fn unit(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / (1u32 << 24) as f32
    }
    fn pick<'a, T>(&mut self, v: &'a [T]) -> Option<&'a T> {
        if v.is_empty() {
            None
        } else {
            Some(&v[self.next_u32() as usize % v.len()])
        }
    }
    /// A (pitch, volume) variation so repeats don't sound identical.
    fn jitter(&mut self) -> (f32, f32) {
        (0.82 + self.unit() * 0.36, 0.6 + self.unit() * 0.4)
    }
}

/// Spawn a one-shot sound with randomized pitch + volume, scaled by the master
/// SFX volume (and skipped entirely when SFX are switched off).
fn play_sfx(commands: &mut Commands, h: &Handle<AudioSource>, rng: &mut SfxRng) {
    if !rng.sfx_on || rng.sfx_vol <= 0.0 {
        return;
    }
    let (speed, vol) = rng.jitter();
    commands.spawn((
        AudioPlayer(h.clone()),
        PlaybackSettings {
            mode: PlaybackMode::Despawn,
            volume: Volume::Linear(vol * rng.sfx_vol),
            speed,
            ..default()
        },
    ));
}

/// Which overlay the app is showing: the title screen, the pause menu, or
/// neither (playing). `screenshot` keeps the game frozen for headless captures.
#[derive(Resource)]
struct AppUi {
    on_title: bool,
    paused: bool,
    screenshot: bool,
}

/// The two independently-controllable audio channels in the options menu.
#[derive(Clone, Copy, PartialEq)]
enum VolKind {
    Sfx,
    Music,
}

#[derive(Component)]
struct TitleScreen;
#[derive(Component)]
struct PauseMenu;
/// A draggable volume bar (the track) for one channel.
#[derive(Component)]
struct VolSlider(VolKind);
/// The green fill inside a volume bar.
#[derive(Component)]
struct VolFill(VolKind);
/// An on/off checkbox button for one channel.
#[derive(Component)]
struct VolToggle(VolKind);
/// The check-mark text inside a toggle.
#[derive(Component)]
struct VolToggleMark(VolKind);

/// Background music discovered from the `music/` folder. A fresh song plays at
/// the start of each round, loops if it finishes mid-round, and fades out when
/// the round ends.
#[derive(Resource)]
struct Music {
    songs: Vec<Handle<AudioSource>>,
    /// A shuffled play order; we step through it and reshuffle once exhausted so
    /// every song plays once per cycle, in a fresh random order each cycle.
    order: Vec<usize>,
    pos: usize,
    vol: f32,
    on: bool,
    last_street: Street,
    /// Current fade level (1 = full, 0 = silent) and whether we're fading out.
    fade: f32,
    fading: bool,
}
#[derive(Component)]
struct MusicTrack;

/// Where the human stands in the game: still playing, busted out (now watching
/// the AI from the bar), or having cleaned everyone out (offered the penthouse).
#[derive(Resource, Clone, Copy, PartialEq)]
enum GameMode {
    Playing,
    Busted,
    Won,
}

/// Which room the game is played in.
#[derive(Resource, Clone, Copy, PartialEq)]
enum Environment {
    Bar,
    Penthouse,
}

/// Escalating blind schedule (small, big). The clock advances one level every
/// `BLIND_INTERVAL` of *active* play.
const BLIND_LEVELS: [(u32, u32); 12] = [
    (5, 10),
    (10, 20),
    (15, 30),
    (25, 50),
    (40, 80),
    (60, 120),
    (100, 200),
    (150, 300),
    (250, 500),
    (400, 800),
    (600, 1200),
    (1000, 2000),
];
const BLIND_INTERVAL: f32 = 300.0; // 5 minutes of active play per level

/// Tracks time toward the next blind increase. The clock only advances while the
/// game is unpaused and the player is active (it stalls after 30s idle).
#[derive(Resource)]
struct BlindClock {
    active: f32,
    last_input: f32,
    level: usize,
}

/// The end-of-game overlay (lose / win) and its parts.
#[derive(Component)]
struct GameOverRoot;
#[derive(Component)]
struct GameOverTitle;
#[derive(Component)]
struct GameOverSub;
/// A button in the game-over overlay.
#[derive(Component, Clone, Copy, PartialEq)]
enum GameOverBtn {
    NewGame,
    Penthouse,
}

/// Set true after the game state changes; the redraw system rebuilds the props.
#[derive(Resource)]
struct NeedsRedraw(bool);

/// True for one stack-redraw at the start of a game, so the chips rain in.
#[derive(Resource)]
struct IntroDrop(bool);

/// When SCREENSHOT=<path> is set, the app renders a few frames, saves a PNG to
/// that path, and exits — used for automated visual checks (headless via Xvfb).
#[derive(Resource)]
struct ShotState {
    path: String,
    frame: u32,
}

fn main() {
    // When launched as a bundled macOS .app (cwd = "/"), assets sit next to the
    // executable. Switch to that directory so the asset server and the folder
    // scanners find them. Only do this if `assets/` is actually beside the exe,
    // so `cargo run` (assets in the project root) is unaffected.
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            if dir.join("assets").is_dir() {
                let _ = std::env::set_current_dir(dir);
            }
        }
    }

    // Headless captures (SCREENSHOT / DEAL_DEMO) skip the title screen.
    let screenshot_env = env::var("SCREENSHOT").is_ok();
    let demo_env = env::var("DEAL_DEMO").is_ok();
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "Pokermon Hold 'em".into(),
            resolution: [1100u32, 760].into(),
            ..default()
        }),
        ..default()
    }))
    .insert_resource(ClearColor(Color::srgb(0.04, 0.06, 0.07)))
    .insert_resource(build_poker())
    .insert_resource(NeedsRedraw(true))
    .insert_resource(BetSlider { frac: 0.5 })
    .insert_resource(AppUi {
        // SHOW_TITLE / SHOW_PAUSE force an overlay on for headless capture.
        on_title: (!screenshot_env && !demo_env) || env::var("SHOW_TITLE").is_ok(),
        paused: env::var("SHOW_PAUSE").is_ok(),
        screenshot: screenshot_env && !demo_env,
    })
    .insert_resource(if env::var("SHOW_WIN").is_ok() {
        GameMode::Won
    } else if env::var("SHOW_BUST").is_ok() {
        GameMode::Busted
    } else {
        GameMode::Playing
    })
    .insert_resource(if env::var("SHOW_PENTHOUSE").is_ok() {
        Environment::Penthouse
    } else {
        Environment::Bar
    })
    .insert_resource(BlindClock {
        active: 0.0,
        last_input: 0.0,
        level: 0,
    })
    .insert_resource(IntroDrop(true))
    .insert_resource(SfxRng::new(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64 | 1)
            .unwrap_or(1),
    ))
    .init_resource::<CardFaces>()
    .add_systems(Startup, setup)
    .add_systems(
        Update,
        (
            standee_system,
            char_anim,
            smoke_system,
            auto_play,
            deal_system,
            turn_sounds,
            slider_system,
            betting_ui,
            next_round,
            win_celebrate,
            turn_arrow,
            bar_standee,
            chip_fly,
            chip_drop,
            redraw_table,
            hud,
            money_labels,
            my_money,
            human_cards_ui,
            seat_visibility,
        ),
    )
    .add_systems(
        Update,
        (
            sync_pause,
            ui_input,
            ui_overlays,
            music_system,
            volume_controls,
            volume_toggle,
            game_over_detect,
            spectate_advance,
            game_over_ui,
            camera_rig,
            chair_rig,
            name_plates,
            win_glow,
            spectate_beer,
            environment_visibility,
            hud_panel_toggle,
            pot_label,
            action_ticker,
            blind_clock,
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

    let seed = if let Some(s) = env::var("SEED").ok().and_then(|s| s.parse::<u64>().ok()) {
        s
    } else if env::var("SCREENSHOT").is_ok() {
        7 // deterministic state for visual checks
    } else {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(1)
    };

    let mut game = Game::new(players, 5, 10, seed);
    game.start_hand();

    // DEAL_DEMO runs the deal animation live (no fast-forward, not paused) so a
    // timed screenshot can catch cards in flight.
    let demo = env::var("DEAL_DEMO").is_ok();

    // For screenshots, fast-forward to a representative state: SHOWDOWN stops at
    // a multi-way showdown; otherwise the human's turn on the flop.
    let want_showdown = env::var("SHOWDOWN").is_ok();
    if env::var("SCREENSHOT").is_ok() && !demo {
        let mut steps = 0;
        loop {
            if steps > 600 {
                break;
            }
            steps += 1;
            if game.street == Street::HandOver {
                if want_showdown && game.live_count() > 1 {
                    break; // a real showdown — stop here
                }
                game.start_hand();
            } else if want_showdown {
                game.apply(Action::Call); // everyone calls down to showdown
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
        paused: env::var("SCREENSHOT").is_ok() && !demo,
        pending_deal: true,
        dealing: false,
        deal_start: 0.0,
        deal_end: 0.0,
        last_actor: usize::MAX,
        showdown_raise: 0.0,
        comm_shown: 0,
        comm_anim: false,
        comm_end: 0.0,
        celebrated: false,
        log: "New hand".to_string(),
    }
}

fn screenshot_system(mut state: ResMut<ShotState>, mut commands: Commands) {
    state.frame += 1;
    // In DEAL_DEMO, capture earlier to catch cards mid-flight. CAP_FRAME lets a
    // headless live run capture a later frame (e.g. after some betting).
    let cap = env::var("CAP_FRAME")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(if env::var("DEAL_DEMO").is_ok() { 6 } else { 30 });
    if state.frame == cap {
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(state.path.clone()));
    }
    // The screenshot is written asynchronously a frame or two after capture;
    // by now it's safely on disk, so just end the process.
    if state.frame >= cap + 30 {
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
        MainCamera,
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
        // A small ring around the pot, kept behind the community cards.
        Transform::from_xyz(0.0, felt_top + 0.005, -0.55)
            .with_scale(Vec3::new(rx * 0.3, 1.0, rz * 0.26)),
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

    // --- Bubble the bartender: a billboard standee behind the counter, with
    // directional sprites so he can idle, turn to the bar, and stroll around ---
    {
        // Behind the bar, raised so his whole body sits above the counter top —
        // his wide billboard then never sweeps through the bar when it turns to
        // face the camera (the counter/shelves are all below his feet).
        let base = Vec3::new(-7.5, floor_y + 3.4, bar_z - 0.1);
        let sprites = bubble_sprites();
        let mats: [Handle<StandardMaterial>; 4] = sprites.map(|path| {
            let tex = asset_server.load(path);
            materials.add(StandardMaterial {
                base_color: Color::WHITE,
                base_color_texture: Some(tex.clone()),
                emissive: LinearRgba::rgb(0.6, 0.6, 0.6),
                emissive_texture: Some(tex),
                perceptual_roughness: 1.0,
                reflectance: 0.0,
                alpha_mode: AlphaMode::Blend,
                double_sided: true,
                cull_mode: None,
                ..default()
            })
        });
        commands.spawn((
            Mesh3d(meshes.add(Rectangle::new(3.0, 4.06))),
            MeshMaterial3d(mats[0].clone()),
            Transform::from_translation(base),
            NotShadowCaster,
            BarStandee {
                base,
                seed: 4.2,
                state: BarState::Front,
                state_until: 0.0,
                walk_from: base.x,
                walk_to: base.x,
                walk_start: 0.0,
                squash: 0.0,
                mats,
            },
        ));
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
    // One material per chip denomination, indexed to match `CHIP_DENOMS`:
    // [0]=$5 red, [1]=$10 blue, [2]=$20 green, [3]=$50 orange, [4]=$100 purple.
    // Each carries a classic chip-face texture (coloured disc, cream edge spots,
    // dashed ring, denomination) baked into the art, so the colour comes from the
    // texture rather than a flat tint.
    let chip_textures = ["chip_5.png", "chip_10.png", "chip_20.png", "chip_50.png", "chip_100.png"];
    let chip_mats: Vec<_> = chip_textures
        .iter()
        .map(|tex| materials.add(StandardMaterial {
            base_color_texture: Some(asset_server.load(*tex)),
            perceptual_roughness: 0.55,
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

        // A chair behind each player: a leather backrest on two posts. The
        // chair_rig system swivels it each frame to stay directly behind the
        // standee *as seen from the live camera*, so the flat billboard never
        // clips through it from any viewpoint (front view or bar spectate).
        let to_cam = Vec2::new(cam_pos.x - x, cam_pos.z - z).normalize();
        let cyaw = to_cam.x.atan2(to_cam.y);
        let crot = Quat::from_rotation_y(cyaw);
        let cright = crot * Vec3::X;
        let cback = Vec3::new(x - to_cam.x * 0.55, floor_y + 2.0, z - to_cam.y * 0.55);
        let anchor = Vec3::new(x, floor_y, z);
        commands.spawn((
            Mesh3d(chair_back_mesh.clone()),
            MeshMaterial3d(chair_leather.clone()),
            Transform::from_translation(cback).with_rotation(crot),
            ChairPart {
                anchor,
                post: 0.0,
                is_post: false,
            },
        ));
        for s in [-1.0_f32, 1.0] {
            commands.spawn((
                Mesh3d(chair_post_mesh.clone()),
                MeshMaterial3d(chair_wood.clone()),
                Transform::from_translation(
                    Vec3::new(cback.x, floor_y + 1.3, cback.z) + cright * (0.84 * s),
                )
                .with_rotation(crot),
                ChairPart {
                    anchor,
                    post: s,
                    is_post: true,
                },
            ));
        }

        // Build one standee material per sprite. All frames share the same
        // params (white base, self-lit, matte, alpha-blended).
        let mut mk = |path: &str| {
            let tex: Handle<Image> = asset_server.load(path.to_string());
            materials.add(StandardMaterial {
                // White base so the PNG shows its true colors (no tint).
                base_color: Color::WHITE,
                base_color_texture: Some(tex.clone()),
                // Self-light the art so the flat cartoon colours read true and
                // vivid (not washed out) even as the room is dim.
                emissive: LinearRgba::rgb(0.7, 0.7, 0.7),
                emissive_texture: Some(tex),
                // Fully matte, zero specular: kills the grey sheen that was
                // lifting dark areas (e.g. inside Hoodguy's hood) to grey.
                perceptual_roughness: 1.0,
                reflectance: 0.0,
                metallic: 0.0,
                alpha_mode: AlphaMode::Blend,
                double_sided: true,
                cull_mode: None,
                ..default()
            })
        };

        // Animation frames from characters/<id>/ (default/talk/back/sit); the
        // single base sprite stands in for anything missing.
        let (def_f, talk_f, back_f, sit_f) = char_frame_files(c.id);
        let mut default_mats: Vec<_> = def_f.iter().map(|p| mk(p)).collect();
        if default_mats.is_empty() {
            default_mats.push(mk(c.file));
        }
        let talk_mats: Vec<_> = talk_f.iter().map(|p| mk(p)).collect();
        let back_mat = back_f.as_deref().map(&mut mk);
        let sit_mat = sit_f.as_deref().map(&mut mk);
        let material = default_mats[0].clone();

        // Feet on the floor; the raised table edge crosses the lower body so
        // they read as seated rather than floating. Bigger characters keep the
        // SAME centre height (so they still sit at the table the way they did
        // before) — the extra size just makes their head taller and lets the
        // larger lower body sink below the table.
        let char_scale = if c.id == "mejdk" { 1.5 } else { 1.0 };
        let flip_x = if c.id == "jaack" { -1.0 } else { 1.0 };
        let base_y = floor_y + quad_h / 2.0;
        let base = Vec3::new(x, base_y, z);
        commands.spawn((
            Mesh3d(quad.clone()),
            MeshMaterial3d(material),
            CharAnim {
                default_mats,
                talk_mats,
                back_mat,
                sit_mat,
                cur: 0,
                next_switch: 0.0,
                talk_until: 0.0,
                talk_idx: 0,
            },
            Transform::from_translation(base),
            // Flat quads make ugly shadows; use a blob shadow instead.
            NotShadowCaster,
            Standee {
                base,
                // Per-character size, with Jaack mirrored (negative X) so he
                // faces the other way.
                base_scale: Vec3::new(flip_x * char_scale, char_scale, 1.0),
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
                bounce: 0.0,
                squash: 0.0,
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
        let plate_pos = Vec3::new(x, base_y + quad_h * char_scale / 2.0 + 0.15, z);
        let away = plate_pos + (plate_pos - cam_pos); // so +Z faces the camera
        commands.spawn((
            Mesh3d(plate_quad.clone()),
            MeshMaterial3d(plate_mat),
            Transform::from_translation(plate_pos).looking_at(away, Vec3::Y),
            NotShadowCaster,
            SeatVisual(i + 1),
            NamePlate,
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
        // Cut out the rounded corners (transparent in the art) for a clean edge.
        alpha_mode: AlphaMode::Mask(0.5),
        ..default()
    });

    // The deck: a face-down stack of cards waiting to be dealt, by the dealer.
    let flat = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
    for k in 0..16 {
        // Dealer's deck, parked at the bottom-right of the felt (out of the way
        // of the left-seat players' cards).
        let base = Vec3::new(3.8, felt_top + 0.02 + k as f32 * 0.012, 1.7);
        commands.spawn((
            Mesh3d(card_quad.clone()),
            MeshMaterial3d(card_back.clone()),
            Transform::from_translation(base)
                .with_rotation(flat)
                .with_scale(Vec3::splat(0.78)),
            DeckCard { base },
        ));
    }

    // The single "whose turn" arrow (positioned/bobbed by `turn_arrow`).
    commands.spawn((
        Mesh3d(meshes.add(Cone {
            radius: 0.17,
            height: 0.34,
        })),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.82, 0.22),
            emissive: LinearRgba::rgb(0.9, 0.62, 0.12),
            unlit: true,
            ..default()
        })),
        Transform::from_xyz(0.0, -10.0, 0.0).with_rotation(Quat::from_rotation_x(std::f32::consts::PI)),
        Visibility::Hidden,
        NotShadowCaster,
        TurnArrow,
    ));

    commands.insert_resource(PokerAssets {
        card_quad: card_quad.clone(),
        chip_mesh: disc.clone(),
        ring_mesh: meshes.add(Torus {
            minor_radius: 0.05,
            major_radius: 0.95,
        }),
        arrow_mesh: meshes.add(Cone {
            radius: 0.17,
            height: 0.34,
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
        glow_mat: materials.add(StandardMaterial {
            base_color: Color::WHITE,
            base_color_texture: Some(asset_server.load("glow.png")),
            alpha_mode: AlphaMode::Add,
            unlit: true,
            ..default()
        }),
        chip_mats: chip_mats.clone(),
    });

    {
        // Discover sound files under assets/sfx and group them by convention.
        let files = scan_sfx();
        let fname = |p: &str| -> String { p.rsplit('/').next().unwrap_or(p).to_lowercase() };
        let pick_prefix = |pre: &str| -> Vec<Handle<AudioSource>> {
            files
                .iter()
                .filter(|f| fname(f).starts_with(pre))
                .map(|f| asset_server.load(f.clone()))
                .collect()
        };
        let or_else = |mut v: Vec<Handle<AudioSource>>, fallback: &str| {
            if v.is_empty() {
                v.push(asset_server.load(fallback.to_string()));
            }
            v
        };
        let cast = roster();
        let mut chars: Vec<Vec<Handle<AudioSource>>> = vec![Vec::new()]; // seat 0 = human
        for c in &cast {
            let key = format!("characters/{}/", c.id);
            chars.push(
                files
                    .iter()
                    .filter(|f| f.to_lowercase().contains(&key))
                    .map(|f| asset_server.load(f.clone()))
                    .collect(),
            );
        }
        commands.insert_resource(Sfx {
            chips: or_else(pick_prefix("chip"), "sfx/chip.wav"),
            card: asset_server.load("sfx/card.wav"),
            knock: asset_server.load("sfx/knock.wav"),
            deal: or_else(pick_prefix("deal"), "sfx/deal.wav"),
            win_small: or_else(pick_prefix("winsmall"), "sfx/win.wav"),
            win_medium: or_else(pick_prefix("winmedium"), "sfx/win.wav"),
            win_big: or_else(pick_prefix("winbig"), "sfx/win.wav"),
            chars,
        });
    }

    // Floating money labels under each AI's name plate. A fixed-width,
    // centre-justified node so the text always sits centred beneath the plate
    // (rather than drifting off to one side).
    for s in 1..=roster().len() {
        commands.spawn((
            Text::new(""),
            TextFont {
                font_size: 19.0,
                ..default()
            },
            TextColor(Color::srgb(1.0, 0.93, 0.6)),
            TextLayout::new_with_justify(Justify::Center),
            Node {
                position_type: PositionType::Absolute,
                width: Val::Px(MONEY_LABEL_W),
                ..default()
            },
            MoneyLabel(s),
        ));
    }
    // Winner banner: a gold-framed panel in the empty band at the top of the
    // screen, above the rest of the UI.
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(14.0),
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            Visibility::Hidden,
            GlobalZIndex(40),
            WinBannerRoot,
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    padding: UiRect::axes(Val::Px(26.0), Val::Px(10.0)),
                    border: UiRect::all(Val::Px(3.0)),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.09, 0.07, 0.03, 0.9)),
                BorderColor::all(Color::srgb(0.88, 0.72, 0.32)),
            ))
            .with_children(|panel| {
                panel.spawn((
                    Text::new(""),
                    TextFont {
                        font_size: 30.0,
                        ..default()
                    },
                    TextColor(Color::srgb(1.0, 0.92, 0.5)),
                    WinBanner,
                ));
            });
        });

    // --- HUD detail panel (street / players / stacks). Hidden by default; hold
    // Tab to peek at it. The always-on info lives elsewhere: the pot amount
    // under the pot, and the last action as a fading ticker up top. ---
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(12.0),
                left: Val::Px(12.0),
                padding: UiRect::axes(Val::Px(14.0), Val::Px(10.0)),
                border: UiRect::all(Val::Px(1.5)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.05, 0.06, 0.08, 0.72)),
            BorderColor::all(Color::srgba(0.85, 0.72, 0.35, 0.55)),
            Visibility::Hidden,
            HudPanel,
        ))
        .with_children(|panel| {
            panel.spawn((
                Text::new("dealing..."),
                TextFont {
                    font_size: 17.0,
                    ..default()
                },
                TextColor(Color::srgb(0.96, 0.95, 0.9)),
                HudText,
            ));
        });

    // Pot amount, anchored in world space just under the pot chips.
    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: 21.0,
            ..default()
        },
        TextColor(Color::srgb(1.0, 0.93, 0.6)),
        TextLayout::new_with_justify(Justify::Center),
        Node {
            position_type: PositionType::Absolute,
            width: Val::Px(MONEY_LABEL_W),
            ..default()
        },
        PotLabel,
    ));

    // Last-action ticker: fades in at the top, drifts, fades out.
    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: 26.0,
            ..default()
        },
        TextColor(Color::srgba(0.96, 0.95, 0.88, 0.0)),
        TextLayout::new_with_justify(Justify::Center),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(64.0),
            left: Val::Px(0.0),
            right: Val::Px(0.0),
            ..default()
        },
        ActionTicker,
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
            HoleCardUi { slot: k, t: 0.0 },
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
            // Bet-size slider: a tall, padded (easy-to-grab) money bar with a $
            // coin. The whole track is the hitbox, padded so the coin is always
            // inside it — clicking the coin anywhere along the bar grabs it.
            bar.spawn((
                Node {
                    width: Val::Px(SLIDER_TRACK_W),
                    height: Val::Px(SLIDER_TRACK_H),
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.0)),
                RelativeCursorPosition::default(),
                SliderTrack,
            ))
            .with_children(|track| {
                let groove_top = SLIDER_TRACK_H / 2.0 - 8.0;
                // the groove (inset to the coin's travel range)
                track.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(SLIDER_PAD),
                        right: Val::Px(SLIDER_PAD),
                        top: Val::Px(groove_top),
                        height: Val::Px(16.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.1, 0.12, 0.12)),
                ));
                // green "money" fill
                track.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(SLIDER_PAD),
                        top: Val::Px(groove_top),
                        height: Val::Px(16.0),
                        width: Val::Px(SLIDER_W / 2.0),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.24, 0.62, 0.34)),
                    SliderFill,
                ));
                // the $ coin handle
                track.spawn((
                    ImageNode {
                        image: asset_server.load("coin.png"),
                        ..default()
                    },
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::Px(SLIDER_W / 2.0),
                        top: Val::Px(SLIDER_TRACK_H / 2.0 - SLIDER_COIN / 2.0),
                        width: Val::Px(SLIDER_COIN),
                        height: Val::Px(SLIDER_COIN),
                        ..default()
                    },
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

    // "Next Hand" button (shown at showdown so the result stays up until you go).
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(28.0),
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            Visibility::Hidden,
            NextRoundBar,
        ))
        .with_children(|bar| {
            bar.spawn((
                Button,
                Interaction::default(),
                Node {
                    width: Val::Px(240.0),
                    height: Val::Px(60.0),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(Color::srgb(0.2, 0.42, 0.26)),
                NextButton,
            ))
            .with_children(|b| {
                b.spawn((
                    Text::new("Next Hand  >"),
                    TextFont {
                        font_size: 26.0,
                        ..default()
                    },
                    TextColor(Color::WHITE),
                ));
            });
        });

    // --- your beer (shown only while spectating from the bar after busting);
    // pinned in front of the camera by `spectate_beer` ---
    {
        let glass = materials.add(StandardMaterial {
            base_color: Color::srgba(0.95, 0.85, 0.55, 0.45),
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 0.1,
            reflectance: 0.5,
            ..default()
        });
        let beer = materials.add(StandardMaterial {
            base_color: Color::srgb_u8(214, 150, 28),
            emissive: LinearRgba::rgb(0.5, 0.32, 0.05),
            perceptual_roughness: 0.3,
            ..default()
        });
        let foam = materials.add(StandardMaterial {
            base_color: Color::srgb(0.98, 0.96, 0.9),
            perceptual_roughness: 0.9,
            ..default()
        });
        commands
            .spawn((
                Transform::from_translation(Vec3::new(0.0, -50.0, 0.0)),
                Visibility::Hidden,
                SpectateBeer,
            ))
            .with_children(|m| {
                // amber beer column
                m.spawn((
                    Mesh3d(meshes.add(Cylinder::new(0.12, 0.30))),
                    MeshMaterial3d(beer.clone()),
                    Transform::from_xyz(0.0, 0.0, 0.0),
                    NotShadowCaster,
                ));
                // glass rim around it
                m.spawn((
                    Mesh3d(meshes.add(Cylinder::new(0.135, 0.34))),
                    MeshMaterial3d(glass.clone()),
                    Transform::from_xyz(0.0, 0.01, 0.0),
                    NotShadowCaster,
                ));
                // foam head
                m.spawn((
                    Mesh3d(meshes.add(Cylinder::new(0.135, 0.06))),
                    MeshMaterial3d(foam.clone()),
                    Transform::from_xyz(0.0, 0.18, 0.0),
                    NotShadowCaster,
                ));
                // handle
                m.spawn((
                    Mesh3d(meshes.add(Torus {
                        minor_radius: 0.022,
                        major_radius: 0.085,
                    })),
                    MeshMaterial3d(glass.clone()),
                    Transform::from_xyz(0.155, 0.0, 0.0)
                        .with_rotation(Quat::from_rotation_y(std::f32::consts::FRAC_PI_2)),
                    NotShadowCaster,
                ));
            });
    }

    // --- penthouse environment (hidden until you "Enter the Penthouse"): a
    // floor-to-ceiling city window that sits in front of the bar (occluding it),
    // a dark glossy floor over the carpet, and a cool city glow. ---
    {
        let win_z = -9.5_f32;
        let win_w = 30.0_f32;
        let win_h = 10.0_f32; // floor (-0.5) to ceiling (9.5)
        let win_cy = floor_y + win_h / 2.0;
        // The city skyline itself: a big emissive plane so the lit windows glow.
        commands.spawn((
            Mesh3d(meshes.add(Rectangle::new(win_w, win_h))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::WHITE,
                base_color_texture: Some(asset_server.load("city.png")),
                emissive: LinearRgba::rgb(0.6, 0.6, 0.8),
                emissive_texture: Some(asset_server.load("city.png")),
                unlit: true,
                ..default()
            })),
            Transform::from_xyz(0.0, win_cy, win_z - 0.3),
            Visibility::Hidden,
            NotShadowCaster,
            EnvPenthouse,
        ));
        // Dark window frame / mullions in front of the glass (gridded steel).
        let mullion = materials.add(StandardMaterial {
            base_color: Color::srgb_u8(18, 20, 26),
            metallic: 0.7,
            perceptual_roughness: 0.4,
            ..default()
        });
        // verticals
        for i in 0..7 {
            let mx = -win_w / 2.0 + win_w * i as f32 / 6.0;
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(0.16, win_h, 0.16))),
                MeshMaterial3d(mullion.clone()),
                Transform::from_xyz(mx, win_cy, win_z),
                Visibility::Hidden,
                EnvPenthouse,
            ));
        }
        // horizontals (incl. floor & ceiling rails)
        for j in 0..4 {
            let my = floor_y + win_h * j as f32 / 3.0;
            commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(win_w, 0.18, 0.16))),
                MeshMaterial3d(mullion.clone()),
                Transform::from_xyz(0.0, my, win_z),
                Visibility::Hidden,
                EnvPenthouse,
            ));
        }
        // Glossy dark penthouse floor laid over the carpet.
        commands.spawn((
            Mesh3d(meshes.add(Rectangle::new(60.0, 60.0))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgb_u8(20, 18, 24),
                perceptual_roughness: 0.12,
                metallic: 0.3,
                reflectance: 0.6,
                ..default()
            })),
            Transform::from_xyz(0.0, floor_y + 0.03, 0.0)
                .with_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2)),
            Visibility::Hidden,
            EnvPenthouse,
        ));
        // Cool city glow spilling in from the window.
        commands.spawn((
            PointLight {
                color: Color::srgb(0.6, 0.7, 1.0),
                intensity: 700_000.0,
                range: 28.0,
                shadows_enabled: false,
                ..default()
            },
            Transform::from_xyz(0.0, floor_y + 4.0, win_z + 2.5),
            Visibility::Hidden,
            EnvPenthouse,
        ));
    }

    // --- background music: discover the songs in assets/music ---
    {
        let paths = scan_music();
        let songs: Vec<Handle<AudioSource>> =
            paths.iter().map(|p| asset_server.load(p.clone())).collect();
        // Shuffle the play order right now with a fresh time seed, independent of
        // any other RNG, so the first song is genuinely random every launch.
        let mut seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64 | 1)
            .unwrap_or(1);
        let mut order: Vec<usize> = (0..songs.len()).collect();
        for i in (1..order.len()).rev() {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let j = (seed >> 33) as usize % (i + 1);
            order.swap(i, j);
        }
        commands.insert_resource(Music {
            songs,
            order,
            pos: usize::MAX, // sentinel: first play uses order[0] as-is
            vol: 0.25,
            on: true,
            last_street: Street::HandOver,
            fade: 1.0,
            fading: false,
        });
    }

    // --- title screen overlay (dismiss with click / Enter) ---
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(0.0),
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                bottom: Val::Px(0.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                row_gap: Val::Px(18.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.02, 0.05, 0.72)),
            Visibility::Hidden,
            GlobalZIndex(50),
            TitleScreen,
        ))
        .with_children(|t| {
            t.spawn((
                Text::new("POKERMON HOLD 'EM"),
                TextFont {
                    font_size: 64.0,
                    ..default()
                },
                TextColor(Color::srgb(1.0, 0.85, 0.4)),
            ));
            t.spawn((
                Text::new("click or press  Enter  to play"),
                TextFont {
                    font_size: 24.0,
                    ..default()
                },
                TextColor(Color::srgb(0.9, 0.9, 0.85)),
            ));
        });

    // --- pause menu overlay (toggle with Enter); audio options inside ---
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(0.0),
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                bottom: Val::Px(0.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
            Visibility::Hidden,
            GlobalZIndex(60),
            PauseMenu,
        ))
        .with_children(|m| {
            m.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(20.0),
                    padding: UiRect::axes(Val::Px(40.0), Val::Px(30.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.07, 0.08, 0.10, 0.96)),
                BorderColor::all(Color::srgb(0.85, 0.72, 0.35)),
            ))
            .with_children(|panel| {
                panel.spawn((
                    Text::new("PAUSED"),
                    TextFont {
                        font_size: 42.0,
                        ..default()
                    },
                    TextColor(Color::srgb(1.0, 0.85, 0.4)),
                ));
                spawn_volume_row(panel, "Sound FX", VolKind::Sfx);
                spawn_volume_row(panel, "Music", VolKind::Music);
                panel.spawn((
                    Text::new("press  Enter  to resume"),
                    TextFont {
                        font_size: 17.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.7, 0.7, 0.66)),
                ));
            });
        });

    // --- end-of-game overlay (busted / won): a framed panel near the bottom so
    // the table stays visible behind it (you watch the AI play on after busting)
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                top: Val::Px(0.0),
                left: Val::Px(0.0),
                right: Val::Px(0.0),
                bottom: Val::Px(0.0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::FlexEnd,
                padding: UiRect::bottom(Val::Px(40.0)),
                ..default()
            },
            Visibility::Hidden,
            GlobalZIndex(55),
            GameOverRoot,
        ))
        .with_children(|o| {
            o.spawn((
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: Val::Px(10.0),
                    padding: UiRect::axes(Val::Px(34.0), Val::Px(20.0)),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.06, 0.07, 0.09, 0.92)),
                BorderColor::all(Color::srgb(0.85, 0.72, 0.35)),
            ))
            .with_children(|panel| {
                panel.spawn((
                    Text::new(""),
                    TextFont {
                        font_size: 40.0,
                        ..default()
                    },
                    TextColor(Color::srgb(1.0, 0.85, 0.4)),
                    GameOverTitle,
                ));
                panel.spawn((
                    Text::new(""),
                    TextFont {
                        font_size: 20.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.9, 0.9, 0.85)),
                    GameOverSub,
                ));
                panel
                    .spawn(Node {
                        flex_direction: FlexDirection::Row,
                        column_gap: Val::Px(18.0),
                        margin: UiRect::top(Val::Px(8.0)),
                        ..default()
                    })
                    .with_children(|row| {
                        for (btn, label) in [
                            (GameOverBtn::Penthouse, "Enter the Penthouse"),
                            (GameOverBtn::NewGame, "New Game?"),
                        ] {
                            row.spawn((
                                Button,
                                Node {
                                    padding: UiRect::axes(Val::Px(22.0), Val::Px(12.0)),
                                    border: UiRect::all(Val::Px(2.0)),
                                    ..default()
                                },
                                BackgroundColor(Color::srgb(0.2, 0.42, 0.26)),
                                BorderColor::all(Color::srgb(0.85, 0.72, 0.35)),
                                btn,
                            ))
                            .with_children(|b| {
                                b.spawn((
                                    Text::new(label),
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
    // Two cigarettes parked on the ashtray edge, each genuinely *resting* on the
    // rim: the filter end sits on the felt outside, the body touches the top of
    // the rim at its crossing point, and the lit end angles up over the dish —
    // so nothing passes through the rim or the dish.
    for (theta, tilt) in [(2.35_f32, 0.30_f32), (-0.65, 0.34)] {
        // Contact point: on top of the rim (torus top + cig radius).
        let contact = Vec3::new(
            ash_x + theta.cos() * 0.4,
            felt_top + 0.10 + 0.028,
            ash_z + theta.sin() * 0.4,
        );
        // Long axis: horizontally inward over the dish, tilted up.
        let dir = Vec3::new(
            -theta.cos() * tilt.cos(),
            tilt.sin(),
            -theta.sin() * tilt.cos(),
        );
        let rot = Quat::from_rotation_arc(Vec3::Y, dir);
        // Centre sits a little up-axis of the contact, so most of the paper +
        // the filter hang outside, dropping to the felt.
        let base = contact + dir * 0.12;
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
        let origin = ember + Vec3::Y * 0.03;
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
                    phase: p as f32 * 0.2 + theta.abs(),
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

        // Bounce: a little squash-and-stretch hop when the character speaks.
        const BOUNCE_DUR: f32 = 0.42;
        let (mut sx, mut sy) = (1.0, 1.0);
        if s.bounce > 0.0 {
            s.bounce = (s.bounce - dt).max(0.0);
            let bp = 1.0 - s.bounce / BOUNCE_DUR; // 0..1 over the hop
            let hop = (bp * std::f32::consts::PI).sin(); // up then down
            pos.y += hop * 0.45;
            // squash on take-off/landing, stretch tall at the apex
            let stretch = (bp * std::f32::consts::PI * 2.0).sin();
            sy = 1.0 + 0.16 * hop - 0.10 * (-stretch).max(0.0);
            sx = 1.0 - 0.12 * hop + 0.08 * (-stretch).max(0.0);
        }

        // Subtle squash-stretch pulse on sprite-frame switches (set by char_anim).
        const FRAME_SQUASH: f32 = 0.22;
        if s.squash > 0.0 {
            s.squash = (s.squash - dt).max(0.0);
            let p = (((FRAME_SQUASH - s.squash) / FRAME_SQUASH) * std::f32::consts::PI).sin();
            sx *= 1.0 + p * 0.06;
            sy *= 1.0 - p * 0.07;
        }

        // Nudge the standee toward the camera so its (camera-facing, flat) plane
        // sits in front of the fixed chair from any angle — it then cleanly
        // occludes the chair instead of the billboard intersecting it. Fades out
        // as they walk away to the bar.
        let to_cam = (Vec3::new(cam_pos.x, pos.y, cam_pos.z) - pos).normalize_or_zero();
        pos += to_cam * 0.45 * (1.0 - s.walk);

        t.translation = pos;
        let target = Vec3::new(cam_pos.x, pos.y, cam_pos.z);
        t.look_at(target, Vec3::Y);

        // A tiny lean + scale pulse on top of the facing rotation, plus bounce.
        t.rotate_local_y(s.yaw_offset);
        t.rotate_local_z(nlean * 0.005);
        t.scale = s.base_scale * (1.0 + nsc * 0.004) * Vec3::new(sx, sy, 1.0);
    }
}

/// Drive each character's sprite frames: random default-frame switches on the
/// same stepped cadence as the wiggle, a random talk frame while their voice
/// plays, the "back" sprite while walking to the bar after busting, and "sit"
/// once they're on the stool — with a subtle squash-stretch on every switch.
fn char_anim(
    time: Res<Time>,
    mut rng: ResMut<SfxRng>,
    mut q: Query<(
        &mut Standee,
        &mut CharAnim,
        &mut MeshMaterial3d<StandardMaterial>,
    )>,
) {
    let now = time.elapsed_secs();
    let step = (now * 4.0).floor();
    for (mut s, mut a, mut mat) in &mut q {
        let fallback = a.default_mats[a.cur % a.default_mats.len()].clone();
        let want: Handle<StandardMaterial> = if s.walk >= 0.995 {
            a.sit_mat.clone().unwrap_or(fallback)
        } else if s.walk > 0.0 {
            a.back_mat.clone().unwrap_or(fallback)
        } else if now < a.talk_until && !a.talk_mats.is_empty() {
            a.talk_mats[a.talk_idx % a.talk_mats.len()].clone()
        } else {
            // Idle: hop to a different random default frame at random intervals,
            // landing on the same time steps the wiggle is quantized to.
            if a.default_mats.len() > 1 && step >= a.next_switch {
                let mut idx = rng.next_u32() as usize % a.default_mats.len();
                if idx == a.cur {
                    idx = (idx + 1) % a.default_mats.len();
                }
                a.cur = idx;
                // Next switch in ~3–10s (the wiggle steps 4x per second).
                a.next_switch = step + 12.0 + rng.unit() * 28.0;
            }
            a.default_mats[a.cur].clone()
        };
        if mat.0 != want {
            mat.0 = want;
            s.squash = 0.22; // the little pop on every frame change
        }
    }
}

/// Show the human's two hole cards as a 2D overlay. When a hand is dealt they
/// slide from the table (where they were dealt) up to their resting spot in the
/// bottom-left corner.
fn human_cards_ui(
    time: Res<Time>,
    poker: Res<Poker>,
    ui: Res<AppUi>,
    asset_server: Res<AssetServer>,
    camera: Query<(&Camera, &GlobalTransform)>,
    windows: Query<&Window>,
    mut q: Query<(&mut HoleCardUi, &mut Node, &mut ImageNode, &mut Visibility)>,
) {
    const CW: f32 = 112.0;
    const CH: f32 = 156.0;
    let human = &poker.game.players[0];
    let show = human.in_hand() && !poker.dealing && !ui.on_title;
    let cam = camera.single().ok();
    let win_h = windows.single().map(|w| w.height()).unwrap_or(760.0);

    for (mut hc, mut node, mut img, mut vis) in &mut q {
        if !show {
            *vis = Visibility::Hidden;
            hc.t = 0.0; // re-arm the slide for the next hand
            continue;
        }
        *vis = Visibility::Visible;
        img.image = asset_server.load(format!("cards/{}.png", human.hole[hc.slot].code()));
        hc.t = (hc.t + time.delta_secs() * 3.5).min(1.0);
        let e = hc.t * hc.t * (3.0 - 2.0 * hc.t); // smoothstep

        // Resting spot in the bottom-left.
        let rest_left = 22.0 + hc.slot as f32 * 86.0;
        let rest_bottom = 20.0;

        // Start spot: where the card was dealt on the table, projected to screen.
        let off = (hc.slot as f32 - 0.5) * 0.46;
        let world = Vec3::new(-off, poker.felt_top + 0.02, poker.rz * 0.72);
        let (start_left, start_bottom) = match cam {
            Some((c, ct)) => match c.world_to_viewport(ct, world) {
                Ok(p) => (p.x - CW / 2.0, win_h - p.y - CH / 2.0),
                Err(_) => (rest_left, rest_bottom),
            },
            None => (rest_left, rest_bottom),
        };

        node.left = Val::Px(start_left + (rest_left - start_left) * e);
        node.bottom = Val::Px(start_bottom + (rest_bottom - start_bottom) * e);
    }
}

/// Update the bottom-right money counter (your stack, plus your current bet).
fn my_money(
    poker: Res<Poker>,
    ui: Res<AppUi>,
    mut q: Query<(&mut Text, &mut Visibility), With<MyMoney>>,
) {
    let g = &poker.game;
    let me = &g.players[0];
    // Hidden on the start screen (no game in progress yet).
    if ui.on_title {
        for (_, mut vis) in &mut q {
            *vis = Visibility::Hidden;
        }
        return;
    }
    let mut s = if me.bet > 0 {
        format!("${}  (bet ${})", me.stack, me.bet)
    } else {
        format!("${}", me.stack)
    };
    // At showdown, show your made hand next to your stack.
    if g.street == Street::HandOver && me.in_hand() {
        if let Some(hv) = g.hand_value(0) {
            s = format!("${}  —  {}", me.stack, hv.category.name());
        }
    }
    for (mut t, mut vis) in &mut q {
        *t = Text::new(s.clone());
        *vis = Visibility::Visible;
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

/// Recursively list sound files under `assets/sfx`, returning asset-relative
/// paths like "sfx/chip1.wav" or "sfx/characters/jaack/jaack1.wav".
fn scan_sfx() -> Vec<String> {
    let roots = [env::var("BEVY_ASSET_ROOT").ok(), Some("assets".to_string())];
    for root in roots.into_iter().flatten() {
        let base = std::path::Path::new(&root).join("sfx");
        if !base.is_dir() {
            continue;
        }
        let mut out = Vec::new();
        let mut stack = vec![base];
        while let Some(dir) = stack.pop() {
            let Ok(rd) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in rd.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    stack.push(p);
                } else if matches!(
                    p.extension().and_then(|e| e.to_str()),
                    Some("wav") | Some("ogg")
                ) {
                    if let Ok(rel) = p.strip_prefix(&root) {
                        out.push(rel.to_string_lossy().replace('\\', "/"));
                    }
                }
            }
        }
        if !out.is_empty() {
            out.sort();
            return out;
        }
    }
    Vec::new()
}

/// List the song files under `assets/music`, sorted, as asset-relative paths.
fn scan_music() -> Vec<String> {
    let roots = [env::var("BEVY_ASSET_ROOT").ok(), Some("assets".to_string())];
    for root in roots.into_iter().flatten() {
        let base = std::path::Path::new(&root).join("music");
        if !base.is_dir() {
            continue;
        }
        let mut out = Vec::new();
        if let Ok(rd) = std::fs::read_dir(&base) {
            for entry in rd.flatten() {
                let p = entry.path();
                if matches!(
                    p.extension()
                        .and_then(|e| e.to_str())
                        .map(|s| s.to_lowercase())
                        .as_deref(),
                    Some("ogg") | Some("wav") | Some("mp3") | Some("flac")
                ) {
                    if let Ok(rel) = p.strip_prefix(&root) {
                        out.push(rel.to_string_lossy().replace('\\', "/"));
                    }
                }
            }
        }
        if !out.is_empty() {
            out.sort();
            return out;
        }
    }
    Vec::new()
}

/// Width (px) of the options-menu volume bars.
const VOL_W: f32 = 200.0;

/// Spawn one labelled audio row in the pause menu: a name, an on/off checkbox,
/// and a draggable volume bar, all tagged with the channel `kind`.
fn spawn_volume_row(
    parent: &mut bevy::ecs::hierarchy::ChildSpawnerCommands,
    label: &str,
    kind: VolKind,
) {
    parent
        .spawn(Node {
            flex_direction: FlexDirection::Row,
            align_items: AlignItems::Center,
            column_gap: Val::Px(14.0),
            ..default()
        })
        .with_children(|row| {
            row.spawn((
                Text::new(label),
                TextFont {
                    font_size: 22.0,
                    ..default()
                },
                TextColor(Color::srgb(0.92, 0.92, 0.88)),
                Node {
                    width: Val::Px(110.0),
                    ..default()
                },
            ));
            // on/off checkbox
            row.spawn((
                Button,
                Node {
                    width: Val::Px(28.0),
                    height: Val::Px(28.0),
                    border: UiRect::all(Val::Px(2.0)),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.4)),
                BorderColor::all(Color::srgb(0.8, 0.7, 0.35)),
                VolToggle(kind),
            ))
            .with_children(|c| {
                c.spawn((
                    Text::new("X"),
                    TextFont {
                        font_size: 20.0,
                        ..default()
                    },
                    TextColor(Color::srgb(0.45, 0.92, 0.55)),
                    VolToggleMark(kind),
                ));
            });
            // draggable volume bar (track + green fill)
            row.spawn((
                Node {
                    width: Val::Px(VOL_W),
                    height: Val::Px(22.0),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.12, 0.13, 0.14)),
                RelativeCursorPosition::default(),
                VolSlider(kind),
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
                    BackgroundColor(Color::srgb(0.24, 0.62, 0.34)),
                    VolFill(kind),
                ));
            });
        });
}

/// Discover a character's animation frames from `characters/<id>/` (any case):
/// returns (default frames, talk frames, back, sit) as asset-relative paths,
/// keyed by filename keywords. All empty/None when the folder doesn't exist.
fn char_frame_files(id: &str) -> (Vec<String>, Vec<String>, Option<String>, Option<String>) {
    let mut defaults = Vec::new();
    let mut talks = Vec::new();
    let mut back = None;
    let mut sit = None;
    let roots = [env::var("BEVY_ASSET_ROOT").ok(), Some("assets".to_string())];
    for root in roots.into_iter().flatten() {
        let base = std::path::Path::new(&root).join("characters");
        let Ok(rd) = std::fs::read_dir(&base) else {
            continue;
        };
        for entry in rd.flatten() {
            let dir = entry.path();
            let dname = entry.file_name().to_string_lossy().to_string();
            if !dir.is_dir() || dname.to_lowercase() != id.to_lowercase() {
                continue;
            }
            let Ok(files) = std::fs::read_dir(&dir) else {
                continue;
            };
            for f in files.flatten() {
                let fname = f.file_name().to_string_lossy().to_string();
                let lower = fname.to_lowercase();
                if !lower.ends_with(".png") {
                    continue;
                }
                let rel = format!("characters/{dname}/{fname}");
                if lower.contains("default") {
                    defaults.push(rel);
                } else if lower.contains("talk") {
                    talks.push(rel);
                } else if lower.contains("back") {
                    back = Some(rel);
                } else if lower.contains("sit") {
                    sit = Some(rel);
                }
            }
        }
    }
    defaults.sort();
    talks.sort();
    (defaults, talks, back, sit)
}

/// Find Bubble's directional sprites: looks for a `characters/bubble/` folder
/// (any case) containing PNGs named with front/back/left/right, and falls back
/// to the single `characters/Bubble.png` for any facing that's missing.
/// Returns asset-relative paths ordered [front, back, left, right].
fn bubble_sprites() -> [String; 4] {
    let fallback = "characters/Bubble.png".to_string();
    let mut out = [
        fallback.clone(),
        fallback.clone(),
        fallback.clone(),
        fallback,
    ];
    let roots = [env::var("BEVY_ASSET_ROOT").ok(), Some("assets".to_string())];
    for root in roots.into_iter().flatten() {
        let base = std::path::Path::new(&root).join("characters");
        let Ok(rd) = std::fs::read_dir(&base) else {
            continue;
        };
        for entry in rd.flatten() {
            let dir = entry.path();
            let dname = entry.file_name().to_string_lossy().to_string();
            if !dir.is_dir() || dname.to_lowercase() != "bubble" {
                continue;
            }
            let Ok(files) = std::fs::read_dir(&dir) else {
                continue;
            };
            for f in files.flatten() {
                let fname = f.file_name().to_string_lossy().to_string();
                let lower = fname.to_lowercase();
                if !lower.ends_with(".png") {
                    continue;
                }
                let rel = format!("characters/{dname}/{fname}");
                for (key, slot) in [("front", 0), ("back", 1), ("left", 2), ("right", 3)] {
                    if lower.contains(key) {
                        out[slot] = rel.clone();
                    }
                }
            }
        }
    }
    out
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
#[allow(clippy::too_many_arguments)]
fn do_action(
    poker: &mut Poker,
    action: Action,
    needs: &mut NeedsRedraw,
    commands: &mut Commands,
    assets: &PokerAssets,
    sfx: &Sfx,
    rng: &mut SfxRng,
    now: f32,
) {
    let seat = poker.game.to_act;
    let name = poker.game.players[seat].name.clone();
    let desc = describe_action(&poker.game, seat, action);
    let call_amt = poker.game.call_amount(seat);
    let pre_community = poker.game.community.len();
    let pre_stack = poker.game.players[seat].stack;
    poker.game.apply(action);

    // Chips the player just pushed in are thrown into the pot, coloured by
    // denomination and tumbling through the air with a little fake physics.
    let thrown = pre_stack.saturating_sub(poker.game.players[seat].stack);
    if thrown > 0 {
        let a = poker.seat_angles[seat];
        let ft = poker.felt_top;
        let from = Vec3::new(a.cos() * poker.rx * 0.78, ft + 0.06, a.sin() * poker.rz * 0.78);
        throw_chips_to_pot(commands, assets, from, ft, thrown, now, rng);
    }

    let chip = rng.pick(&sfx.chips).cloned().unwrap_or_default();
    match action {
        Action::Fold => play_sfx(commands, &sfx.card, rng),
        Action::Check => play_sfx(commands, &sfx.knock, rng),
        Action::Call if call_amt > 0 => play_sfx(commands, &chip, rng),
        Action::Call => play_sfx(commands, &sfx.knock, rng),
        Action::Raise(_) => play_sfx(commands, &chip, rng),
    }
    if poker.game.community.len() > pre_community {
        play_sfx(commands, &sfx.card, rng);
    }
    if poker.game.street == Street::HandOver {
        // Pick a win sting scaled to the size of the pot just won.
        let won: u32 = poker.game.last_payouts.iter().map(|p| p.amount).sum();
        let list = if won >= 600 {
            &sfx.win_big
        } else if won >= 180 {
            &sfx.win_medium
        } else {
            &sfx.win_small
        };
        if let Some(h) = rng.pick(list) {
            play_sfx(commands, &h.clone(), rng);
        }
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
    assets: Res<PokerAssets>,
    sfx: Res<Sfx>,
    mut sfx_rng: ResMut<SfxRng>,
) {
    if poker.paused {
        return;
    }
    // Wait while a hand or street is being dealt (handled by deal_system).
    if poker.dealing || poker.pending_deal || poker.comm_anim {
        return;
    }
    let dt = time.delta();
    // At showdown we hold the result on screen until the player hits "Next
    // Hand" (handled by `next_round`).
    if poker.game.street == Street::HandOver {
        return;
    }

    // The human (seat 0) decides via the betting UI — don't auto-act for them.
    if poker.game.to_act == 0 {
        return;
    }

    if poker.act_timer.tick(dt).just_finished() {
        let seat = poker.game.to_act;
        let action = ai_decide(&mut poker.game, seat);
        let now = time.elapsed_secs();
        do_action(
            &mut poker, action, &mut needs, &mut commands, &assets, &sfx, &mut sfx_rng, now,
        );
    }
}

/// Play a character's voice when it becomes their turn (random clip, jittered)
/// and make that character do a little squash-and-stretch hop.
fn turn_sounds(
    time: Res<Time>,
    mut poker: ResMut<Poker>,
    mut commands: Commands,
    sfx: Res<Sfx>,
    mut rng: ResMut<SfxRng>,
    mut standees: Query<(&mut Standee, &mut CharAnim)>,
) {
    if poker.paused || poker.dealing || poker.pending_deal || poker.comm_anim {
        return;
    }
    if poker.game.street == Street::HandOver {
        poker.last_actor = usize::MAX;
        return;
    }
    let seat = poker.game.to_act;
    if seat == poker.last_actor {
        return;
    }
    poker.last_actor = seat;
    let talk_pick = rng.next_u32() as usize;
    for (mut st, mut anim) in &mut standees {
        if st.seat == seat {
            st.bounce = 0.42;
            // Show a random talk frame for about as long as the voice line.
            anim.talk_until = time.elapsed_secs() + 1.0;
            anim.talk_idx = talk_pick;
        }
    }
    if let Some(list) = sfx.chars.get(seat) {
        if let Some(h) = rng.pick(list) {
            play_sfx(&mut commands, &h.clone(), &mut rng);
        }
    }
}

/// Kick off and animate the deal: a quick deck shuffle, then face-down cards
/// slide out from the deck to each seat, one at a time.
fn deal_system(
    time: Res<Time>,
    mut poker: ResMut<Poker>,
    mut needs: ResMut<NeedsRedraw>,
    mut commands: Commands,
    assets: Res<PokerAssets>,
    sfx: Res<Sfx>,
    mut rng: ResMut<SfxRng>,
    mut faces: ResMut<CardFaces>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
    mut q_deal: Query<(Entity, &mut DealingCard, &mut Transform), Without<DeckCard>>,
    mut q_deck: Query<(&DeckCard, &mut Transform), Without<DealingCard>>,
    q_pot: Query<Entity, With<PotChip>>,
    mut last_deal: Local<usize>,
) {
    let now = time.elapsed_secs();
    let dt = time.delta_secs();
    let flat = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
    let deck_pos = Vec3::new(3.8, poker.felt_top + 0.21, 1.7);

    // Ramp the showdown card-raise (snap instantly when paused for screenshots).
    let target = if poker.game.street == Street::HandOver {
        1.0
    } else {
        0.0
    };
    let speed = if poker.paused { 100.0 } else { 4.5 };
    let nr = poker.showdown_raise + (target - poker.showdown_raise).clamp(-dt * speed, dt * speed);
    if (nr - poker.showdown_raise).abs() > 0.001 {
        poker.showdown_raise = nr;
        needs.0 = true;
    }

    if poker.paused {
        return;
    }

    // --- kick off the hole-card deal for a new hand ---
    if poker.pending_deal && !poker.dealing {
        poker.pending_deal = false;
        poker.comm_shown = 0;
        // Sweep last hand's pot chips off the table before the new deal.
        for e in &q_pot {
            commands.entity(e).despawn();
        }
        let n = poker.game.players.len();
        let mut order = Vec::new();
        for i in 1..=n {
            let s = (poker.game.button + i) % n;
            if poker.game.players[s].in_hand() {
                order.push(s);
            }
        }
        let shuffle = 0.45_f32;
        let step = 0.09_f32;
        let dur = 0.22_f32;
        let mut last = now + shuffle;
        for pass in 0..2 {
            for (idx, &s) in order.iter().enumerate() {
                let a = poker.seat_angles[s];
                let (cosv, sinv) = (a.cos(), a.sin());
                let off = (pass as f32 - 0.5) * 0.46;
                // Second pass lands slightly higher so the cards overlap
                // cleanly instead of intersecting (matches redraw_table).
                let to = Vec3::new(
                    cosv * poker.rx * 0.72 + (-sinv) * off,
                    poker.felt_top + 0.022 + pass as f32 * 0.016,
                    sinv * poker.rz * 0.72 + cosv * off,
                );
                let start = now + shuffle + (pass * order.len() + idx) as f32 * step;
                last = last.max(start + dur);
                spawn_dealing_card(
                    &mut commands,
                    &assets,
                    assets.card_back.clone(),
                    deck_pos,
                    to,
                    start,
                    dur,
                    flat,
                    0.85,
                );
            }
        }
        poker.dealing = true;
        poker.deal_start = now + shuffle;
        poker.deal_end = last + 0.05;
        needs.0 = true;
        // Random deal sound, but never the same one twice in a row — true
        // randomness repeats often enough that it *feels* broken.
        if !sfx.deal.is_empty() {
            let mut i = rng.next_u32() as usize % sfx.deal.len();
            if sfx.deal.len() > 1 && i == *last_deal {
                i = (i + 1 + rng.next_u32() as usize % (sfx.deal.len() - 1)) % sfx.deal.len();
            }
            *last_deal = i;
            let deal = sfx.deal[i].clone();
            play_sfx(&mut commands, &deal, &mut rng);
        }

        // The blinds (anything already committed) fly into the pot as the deal
        // finishes, so the pot pile starts off showing them.
        let ft = poker.felt_top;
        for s in 0..n {
            let committed = poker.game.players[s].committed;
            if committed > 0 {
                let a = poker.seat_angles[s];
                let from =
                    Vec3::new(a.cos() * poker.rx * 0.78, ft + 0.06, a.sin() * poker.rz * 0.78);
                throw_chips_to_pot(&mut commands, &assets, from, ft, committed, last, &mut rng);
            }
        }
    }

    // --- kick off a community-card deal-in (flop/turn/river) ---
    if !poker.dealing && !poker.comm_anim && poker.comm_shown < poker.game.community.len() {
        let total = poker.game.community.len();
        let step = 0.13_f32;
        let dur = 0.24_f32;
        let mut last = now;
        for i in poker.comm_shown..total {
            let to = Vec3::new(
                community_x(i, total as f32),
                poker.felt_top + 0.026,
                COMMUNITY_Z,
            );
            let start = now + (i - poker.comm_shown) as f32 * step;
            last = last.max(start + dur);
            let code = poker.game.community[i].code();
            let mat = face_material(&mut faces, &mut materials, &asset_server, &code);
            spawn_dealing_card(&mut commands, &assets, mat, deck_pos, to, start, dur, flat, 1.25);
        }
        poker.comm_anim = true;
        poker.comm_end = last + 0.05;
        needs.0 = true;
        play_sfx(&mut commands, &sfx.card, &mut rng);
    }

    // --- deck shuffle jiggle during the hole deal ---
    if poker.dealing {
        let shuffling = now < poker.deal_start;
        for (deck, mut tr) in &mut q_deck {
            if shuffling {
                let j = hash11(deck.base.y * 53.0 + now * 30.0) - 0.5;
                let k = hash11(deck.base.y * 91.0 + now * 27.0) - 0.5;
                tr.translation = deck.base + Vec3::new(j * 0.12, 0.0, k * 0.12);
            } else {
                tr.translation = deck.base;
            }
        }
    }

    // --- slide every flying card from the deck to its target, growing as it
    // goes. Landed cards rest at the target (they're cleared only when the real
    // cards are drawn) so there's no flicker gap. ---
    for (_, mut dc, mut tr) in &mut q_deal {
        if now >= dc.start && !dc.played {
            dc.played = true;
            play_sfx(&mut commands, &sfx.card, &mut rng);
        }
        let p = ((now - dc.start) / dc.dur).clamp(0.0, 1.0);
        let e2 = p * p * (3.0 - 2.0 * p); // smoothstep
        let mut pos = dc.from.lerp(dc.to, e2);
        pos.y += (e2 * std::f32::consts::PI).sin() * 0.35; // little arc
        tr.translation = pos;
        tr.scale = Vec3::splat(dc.scale * (0.55 + 0.45 * e2)); // smoothly scale up
    }

    // --- finish the animations: real cards appear and flying ones are cleared
    // in the same frame (no gap). ---
    if poker.dealing && now >= poker.deal_end {
        poker.dealing = false;
        for (deck, mut tr) in &mut q_deck {
            tr.translation = deck.base;
        }
        for (e, _, _) in &q_deal {
            commands.entity(e).despawn();
        }
        needs.0 = true;
    }
    if poker.comm_anim && now >= poker.comm_end {
        poker.comm_anim = false;
        poker.comm_shown = poker.game.community.len();
        for (e, _, _) in &q_deal {
            commands.entity(e).despawn();
        }
        needs.0 = true;
    }
}

/// Spawn one card flying from the deck to a target, for the deal animations.
#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_arguments)]
fn spawn_dealing_card(
    commands: &mut Commands,
    assets: &PokerAssets,
    mat: Handle<StandardMaterial>,
    from: Vec3,
    to: Vec3,
    start: f32,
    dur: f32,
    flat: Quat,
    scale: f32,
) {
    commands.spawn((
        Mesh3d(assets.card_quad.clone()),
        MeshMaterial3d(mat),
        Transform::from_translation(from)
            .with_rotation(flat)
            .with_scale(Vec3::splat(scale * 0.55)),
        DealingCard {
            from,
            to,
            start,
            dur,
            scale,
            played: false,
        },
    ));
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
///
/// Uses Bevy's `RelativeCursorPosition`, which is computed correctly for any
/// display DPI. `cursor_over` tells us a press landed on the (padded) track —
/// including right on the coin, which sits inside the track — so we latch into a
/// drag and follow the cursor until release, even if it slides off the bar.
/// Note: `normalized` is centre-relative, −0.5 (left) .. +0.5 (right).
fn slider_system(
    mouse: Res<ButtonInput<MouseButton>>,
    poker: Res<Poker>,
    mut slider: ResMut<BetSlider>,
    track: Query<&RelativeCursorPosition, With<SliderTrack>>,
    mut fill: Query<&mut Node, (With<SliderFill>, Without<SliderHandle>)>,
    mut handle: Query<&mut Node, (With<SliderHandle>, Without<SliderFill>)>,
    mut dragging: Local<bool>,
) {
    // Don't drag the bet slider while a menu/title overlay is up.
    if poker.paused {
        *dragging = false;
        return;
    }
    if !mouse.pressed(MouseButton::Left) {
        *dragging = false;
    }
    if let Ok(rel) = track.single() {
        if mouse.just_pressed(MouseButton::Left) && rel.cursor_over {
            *dragging = true;
        }
        if *dragging {
            if let Some(n) = rel.normalized {
                // Centre-relative x → pixels from the track's left edge → the
                // coin's travel range (inset by its half-width each side).
                let local_x = (n.x + 0.5) * SLIDER_TRACK_W;
                slider.frac = ((local_x - SLIDER_PAD) / SLIDER_W).clamp(0.0, 1.0);
            }
        }
    }
    if let Ok(mut f) = fill.single_mut() {
        f.width = Val::Px(slider.frac * SLIDER_W);
    }
    if let Ok(mut h) = handle.single_mut() {
        // Coin's left edge so its centre lands on the chosen fraction.
        h.left = Val::Px(slider.frac * SLIDER_W);
    }
}

/// Show the human's action UI on their turn and apply the chosen action.
/// CheckFold = "Check" when checking is free (never a free fold) else "Fold".
/// Keys: C = check/call, F = fold (only when facing a bet), R = raise (slider),
/// A = all-in.
/// At showdown, show the "Next Hand" button and hold the result on screen until
/// the player clicks it (or presses Space/Enter), then deal the next hand.
fn next_round(
    mut poker: ResMut<Poker>,
    mode: Res<GameMode>,
    mut needs: ResMut<NeedsRedraw>,
    keys: Res<ButtonInput<KeyCode>>,
    mut bar: Query<&mut Visibility, With<NextRoundBar>>,
    mut btn: Query<(&Interaction, &mut BackgroundColor), With<NextButton>>,
) {
    // Only once the showdown is settled (cards raised, nothing animating), and
    // only while you're still in the game (spectating auto-advances instead).
    let show = *mode == GameMode::Playing
        && poker.game.street == Street::HandOver
        && !poker.dealing
        && !poker.comm_anim
        && poker.showdown_raise > 0.9;
    for mut v in &mut bar {
        *v = if show {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    if poker.paused || !show {
        return;
    }

    // Space (or the button) deals the next hand; Enter is reserved for pause.
    let mut go = keys.just_pressed(KeyCode::Space);
    for (interaction, mut bg) in &mut btn {
        match *interaction {
            Interaction::Pressed => {
                *bg = BackgroundColor(Color::srgb(0.16, 0.55, 0.28));
                go = true;
            }
            Interaction::Hovered => *bg = BackgroundColor(Color::srgb(0.26, 0.5, 0.32)),
            Interaction::None => *bg = BackgroundColor(Color::srgb(0.2, 0.42, 0.26)),
        }
    }
    if go {
        poker.game.start_hand();
        poker.pending_deal = true;
        poker.showdown_raise = 0.0;
        poker.log = "New hand".to_string();
        needs.0 = true;
    }
}

fn betting_ui(
    time: Res<Time>,
    mut poker: ResMut<Poker>,
    mut needs: ResMut<NeedsRedraw>,
    mut commands: Commands,
    assets: Res<PokerAssets>,
    sfx: Res<Sfx>,
    mut sfx_rng: ResMut<SfxRng>,
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
    let my_turn = !poker.paused
        && !poker.dealing
        && !poker.comm_anim
        && g.street != Street::HandOver
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
        let now = time.elapsed_secs();
        do_action(
            &mut poker, action, &mut needs, &mut commands, &assets, &sfx, &mut sfx_rng, now,
        );
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

/// Fixed width of the floating per-player money labels (so they centre cleanly).
const MONEY_LABEL_W: f32 = 150.0;

/// Bet slider: `SLIDER_W` is the coin's travel range; the grabbable track is
/// padded by the coin's half-width on each side so the coin (and your click on
/// it) is always inside the hitbox, even at the ends.
const SLIDER_W: f32 = 360.0;
const SLIDER_COIN: f32 = 44.0;
const SLIDER_PAD: f32 = SLIDER_COIN / 2.0;
const SLIDER_TRACK_W: f32 = SLIDER_W + SLIDER_COIN;
const SLIDER_TRACK_H: f32 = 56.0;

/// Where the community cards sit (z toward the player) and their x layout.
const COMMUNITY_Z: f32 = 1.95;
fn community_x(i: usize, count: f32) -> f32 {
    // Wide enough that even a slightly-spun card never overlaps its neighbour
    // (card is ~0.98 wide at scale 1.25), so every rank stays readable.
    (i as f32 - (count - 1.0) / 2.0) * 1.34
}

/// Rebuild all engine-driven props (cards, chips, dealer button) from the
/// current game state whenever something changed.
fn redraw_table(
    mut commands: Commands,
    mut needs: ResMut<NeedsRedraw>,
    poker: Res<Poker>,
    ui: Res<AppUi>,
    time: Res<Time>,
    mut intro: ResMut<IntroDrop>,
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
    // The start screen shows an empty, ready table — no dealt hand, chips, or pot.
    if ui.on_title {
        return;
    }

    let g = &poker.game;
    let ft = poker.felt_top;
    // Lie flat, face up. Top edge points away from the camera (-Z) so the ranks
    // read right-side-up for the player sitting at the near (+Z) edge.
    let flat = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);

    // At showdown the cards flip up (animated) so the hands read clearly.
    let showdown = g.street == Street::HandOver;
    let card_tilt = poker.showdown_raise * 0.95;

    // Community cards: in the open space toward the player, bigger and centred.
    // While dealing in, the still-flying ones aren't drawn here.
    let shown = if poker.paused {
        g.community.len()
    } else {
        poker.comm_shown.min(g.community.len())
    };
    let c = g.community.len() as f32;
    for (i, card) in g.community.iter().take(shown).enumerate() {
        let x = community_x(i, c);
        // Only a whisper of spin — enough to look hand-placed, not so much that
        // a card's corner swings over its neighbour and hides a rank.
        let yaw = (hash11(i as f32 * 12.9 + 3.0) - 0.5) * 0.07;
        let mat = face_material(&mut faces, &mut materials, &asset_server, &card.code());
        spawn_table_card(
            &mut commands,
            &assets,
            mat,
            flat,
            x,
            COMMUNITY_Z,
            ft,
            1.25,
            card_tilt,
            yaw,
        );
    }

    // In live play the pot is shown by the actual chips thrown into the middle
    // (they land and accumulate as `PotChip`s). When paused for a screenshot the
    // game is fast-forwarded with no throw animations, so draw a static pile then
    // so the pot is still visible.
    if poker.paused && g.pot() > 0 {
        spawn_chips(&mut commands, &assets, POT_CENTER.x, POT_CENTER.z, ft, g.pot(), Vec2::new(1.0, 0.0), None);
    }

    // Stacks rain down once at the start of a game (the intro flourish).
    let drop = if intro.0 { Some(time.elapsed_secs()) } else { None };
    intro.0 = false;

    // Per-seat: stacks, hole cards, dealer button.
    for (s, p) in g.players.iter().enumerate() {
        let a = poker.seat_angles[s];
        let (cosv, sinv) = (a.cos(), a.sin());

        // Each player's remaining stack, in front of their seat. The piles
        // spread along the seat's tangent so they sit in a tidy row on the rail.
        if p.stack > 0 {
            let bx = cosv * poker.rx * 0.92 + (-sinv) * 0.55;
            let bz = sinv * poker.rz * 0.92 + cosv * 0.55;
            spawn_chips(&mut commands, &assets, bx, bz, ft, p.stack, Vec2::new(-sinv, cosv), drop);
        }

        // While the deal animation plays, the sliding cards stand in for the
        // hole cards; don't draw the real ones yet.
        if p.in_hand() && s != 0 && !poker.dealing {
            let hx = cosv * poker.rx * 0.72;
            let hz = sinv * poker.rz * 0.72;
            let (tx, tz) = (-sinv, cosv); // tangent, to lay the two cards side by side
            for (k, card) in p.hole.iter().enumerate() {
                let off = (k as f32 - 0.5) * 0.46;
                // Gentle spin only, and lay the second card clearly *on top of*
                // the first (raised a touch) so they overlap like real cards
                // instead of intersecting through each other.
                let yaw = (hash11(s as f32 * 7.0 + k as f32 * 3.1) - 0.5) * 0.06;
                let mat = if showdown {
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
                    ft + k as f32 * 0.016,
                    0.85,
                    card_tilt,
                    yaw,
                );
            }
        }
    }

    // (The human's own hole cards are drawn as a flat 2D UI overlay in the
    // bottom-left — see `human_cards_ui`.)

    // Dealer button, off to the side of the button seat's cards (not on them).
    let a = poker.seat_angles[g.button];
    let (bc, bs) = (a.cos(), a.sin());
    commands.spawn((
        Mesh3d(assets.chip_mesh.clone()),
        MeshMaterial3d(assets.button_mat.clone()),
        Transform::from_xyz(
            bc * poker.rx * 0.72 + (-bs) * 1.0,
            ft + 0.02,
            bs * poker.rz * 0.72 + bc * 1.0,
        )
        .with_scale(Vec3::new(0.32, 0.04, 0.32)),
        TableProp,
    ));

    // ("Action on you" arrow is a single persistent entity — see `turn_arrow`.)

    // At showdown, a warm glow behind each winning player.
    if showdown {
        let cam = Vec3::new(0.0, 4.7, 10.4);
        for pay in &g.last_payouts {
            if pay.seat == 0 {
                continue; // the human has no standee
            }
            let a = poker.seat_angles[pay.seat];
            let pos = Vec3::new(a.cos() * poker.prx, 2.3, a.sin() * poker.prz);
            let to_cam = (cam - pos).normalize_or_zero();
            commands.spawn((
                Mesh3d(assets.card_quad.clone()),
                MeshMaterial3d(assets.glow_mat.clone()),
                Transform::from_translation(pos - to_cam * 0.5)
                    .looking_at(cam, Vec3::Y)
                    .with_scale(Vec3::splat(5.0)),
                TableProp,
                WinGlow { anchor: pos },
            ));
        }
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
        // Stencil-cutout the rounded corners: the art is transparent outside the
        // card, so mask those pixels out instead of rendering them black.
        alpha_mode: AlphaMode::Mask(0.5),
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
    tilt: f32,
    yaw: f32,
) {
    // Optionally stand the card up toward the camera (used at showdown so the
    // hands are clearly readable); `tilt == 0` lies flat on the felt. `yaw` is a
    // small random spin so the cards don't look too perfectly aligned.
    let spin = Quat::from_rotation_y(yaw);
    let (rot, lift) = if tilt > 0.0 {
        (
            spin * Quat::from_rotation_x(tilt) * flat,
            1.08 * scale / 2.0 * tilt.sin(),
        )
    } else {
        (spin * flat, 0.0)
    };

    // Soft shadow on the felt beneath the card.
    commands.spawn((
        Mesh3d(assets.card_quad.clone()),
        MeshMaterial3d(assets.shadow_mat.clone()),
        Transform::from_xyz(x + 0.05, felt_top + 0.012, z - 0.08)
            .with_rotation(spin * flat)
            .with_scale(Vec3::new(scale * 1.12, 1.0, scale * 1.12)),
        TableProp,
    ));
    commands.spawn((
        Mesh3d(assets.card_quad.clone()),
        MeshMaterial3d(mat),
        Transform::from_xyz(x, felt_top + 0.026 + lift, z)
            .with_rotation(rot)
            .with_scale(Vec3::splat(scale)),
        TableProp,
    ));
}

/// Chip denominations, largest first, each paired with its colour index in
/// `PokerAssets::chip_mats`: $100 purple, $50 orange, $20 green, $10 blue, $5 red.
const CHIP_DENOMS: [(u32, usize); 5] = [(100, 4), (50, 3), (20, 2), (10, 1), (5, 0)];

/// Break an amount into physical chips that always add up to the value, but in a
/// *balanced* mix so a stack shows a bunch of every colour rather than one tall
/// tower of the biggest denomination. Returns `(colour_index, count)` per
/// denomination, largest first. A pure greedy split would give e.g. $990 as
/// nine $100s and one each of the rest; instead we give each denomination an
/// equal base count first, then spread the small remainder greedily.
fn chip_breakdown(amount: u32) -> Vec<(usize, u32)> {
    // Sum of one chip of every denomination ($5+$10+$20+$50+$100).
    const SET_VALUE: u32 = 185;
    let base = amount / SET_VALUE; // equal count of each denomination
    let mut counts = [0u32; 5]; // indexed by colour: [0]=$5 .. [4]=$100
    for c in counts.iter_mut() {
        *c = base;
    }
    let mut rem = amount - base * SET_VALUE;
    // Spread what's left across denominations, largest first.
    for (value, idx) in CHIP_DENOMS {
        let c = rem / value;
        counts[idx] += c;
        rem -= c * value;
    }
    if rem > 0 {
        counts[0] += 1; // odd chip under $5
    }
    // Emit piles largest-denomination first, skipping any that are empty.
    CHIP_DENOMS
        .iter()
        .filter(|&&(_, idx)| counts[idx] > 0)
        .map(|&(_, idx)| (idx, counts[idx]))
        .collect()
}

/// Lay `amount` worth of chips as separate same-colour piles (one per
/// denomination) around `(cx, cz)`, spreading the piles along `dir` (a unit
/// vector in the table plane). Each pile is a neat colour-sorted stack.
fn spawn_chips(
    commands: &mut Commands,
    assets: &PokerAssets,
    cx: f32,
    cz: f32,
    felt_top: f32,
    amount: u32,
    dir: Vec2,
    drop: Option<f32>,
) {
    if amount == 0 {
        return;
    }
    let mut chip_i = 0usize;
    let piles = chip_breakdown(amount);
    let n = piles.len();
    // Lay the piles out in a compact grid (up to 3 across), spreading width along
    // `dir` and depth along its perpendicular. Spacing is wider than a chip so
    // neighbouring piles never overlap. Chip radius 0.18 → diameter 0.36.
    let perp = Vec2::new(-dir.y, dir.x);
    let spacing = 0.42;
    let per_row = 3usize;
    let rows = n.div_ceil(per_row);
    for (pi, &(color, count)) in piles.iter().enumerate() {
        let row = pi / per_row;
        let col = pi % per_row;
        let row_len = (n - row * per_row).min(per_row);
        let woff = (col as f32 - (row_len as f32 - 1.0) / 2.0) * spacing;
        let doff = (row as f32 - (rows as f32 - 1.0) / 2.0) * spacing;
        let px = cx + dir.x * woff + perp.x * doff;
        let pz = cz + dir.y * woff + perp.y * doff;
        let height = count.min(18);
        for k in 0..height {
            // Per-chip randomness so a stack looks hand-made, not machined: spin
            // each chip a random amount around its axis (so the white edge spots
            // never line up), nudge it slightly off-centre, and give it a faint
            // lean. Seeded from its position so it's stable across redraws.
            let seed = px * 13.1 + pz * 7.7 + k as f32 * 3.37;
            let yaw = hash11(seed) * std::f32::consts::TAU;
            let jx = (hash11(seed + 1.7) - 0.5) * 0.05;
            let jz = (hash11(seed + 9.3) - 0.5) * 0.05;
            let lean_x = (hash11(seed + 4.2) - 0.5) * 0.05;
            let lean_z = (hash11(seed + 2.1) - 0.5) * 0.05;
            let rot = Quat::from_rotation_y(yaw)
                * Quat::from_rotation_x(lean_x)
                * Quat::from_rotation_z(lean_z);
            let rest_y = felt_top + 0.022 + k as f32 * 0.043;
            // Intro flourish: the whole stack rains down from above with a small
            // per-chip stagger, then settles at rest_y.
            let (start_y, anim) = match drop {
                Some(now) => (
                    rest_y + 5.0,
                    Some(ChipDrop {
                        rest_y,
                        start: now + chip_i as f32 * 0.012,
                        dur: 0.5,
                    }),
                ),
                None => (rest_y, None),
            };
            let mut e = commands.spawn((
                Mesh3d(assets.chip_mesh.clone()),
                MeshMaterial3d(assets.chip_mats[color].clone()),
                Transform {
                    translation: Vec3::new(px + jx, start_y, pz + jz),
                    rotation: rot,
                    scale: Vec3::new(0.18, 0.04, 0.18),
                },
                TableProp,
            ));
            if let Some(a) = anim {
                e.insert(a);
            }
            chip_i += 1;
        }
    }
}

/// Update the HUD text every frame from the game state.
fn hud(
    poker: Res<Poker>,
    mut q: Query<&mut Text, (With<HudText>, Without<WinBanner>)>,
    mut banner: Query<&mut Text, (With<WinBanner>, Without<HudText>)>,
    mut banner_root: Query<&mut Visibility, With<WinBannerRoot>>,
) {
    let g = &poker.game;
    let mut s = format!(
        "{}    Pot ${}\nBlinds {}/{}\n\n",
        street_name(g.street),
        g.pot(),
        g.small_blind,
        g.big_blind
    );
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
        // Only a real showdown (more than one player still holding cards) reveals
        // a hand — if everyone else folded, the winner never showed theirs.
        let showdown = g.live_count() > 1;
        g.last_payouts
            .iter()
            .map(|pay| {
                let p = &g.players[pay.seat];
                let verb = if p.is_human { "win" } else { "wins" };
                let hand = if showdown {
                    g.hand_value(pay.seat)
                        .map(|v| format!(" with {}", v.describe()))
                        .unwrap_or_default()
                } else {
                    String::new()
                };
                format!("{} {verb} ${}{}", p.name, pay.amount, hand)
            })
            .collect::<Vec<_>>()
            .join("   ·   ")
    } else {
        String::new()
    };
    for mut text in &mut banner {
        *text = Text::new(banner_text.clone());
    }
    let show_banner = g.street == Street::HandOver && !banner_text.is_empty();
    for mut vis in &mut banner_root {
        *vis = if show_banner {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

/// Bubble roams this stretch of the bar (counter runs x -11..11).
const BUBBLE_WALK_MIN: f32 = -9.5;
const BUBBLE_WALK_MAX: f32 = -2.5;
const BUBBLE_WALK_SPEED: f32 = 2.6; // units/sec — a brisk, visible stroll
const BUBBLE_SQUASH_T: f32 = 0.26; // squash-stretch pulse length (secs)

/// One step of Bubble's behaviour state machine, pulled out so it's unit-testable
/// without Bevy. Given the current state/position, two random rolls and the
/// clock, returns the next state, when to re-decide, and (for a walk) the
/// from/to x of the stroll. Walking is the most common choice so it's actually
/// visible. Returns `(next, until, walk_from, walk_to)`.
fn bubble_next(
    state: BarState,
    base_x: f32,
    r: f32,
    walk_target: f32,
    now: f32,
) -> (BarState, f32, f32, f32) {
    let walk_to = || BUBBLE_WALK_MIN + walk_target * (BUBBLE_WALK_MAX - BUBBLE_WALK_MIN);
    match state {
        // After a walk or tending the bar, mostly settle for a while.
        BarState::WalkLeft | BarState::WalkRight | BarState::Bar => {
            if r < 0.18 {
                let to = walk_to();
                let dur = ((to - base_x).abs() / BUBBLE_WALK_SPEED).max(0.5);
                let dir = if to < base_x {
                    BarState::WalkLeft
                } else {
                    BarState::WalkRight
                };
                (dir, now + dur, base_x, to)
            } else if r < 0.62 {
                (BarState::Front, now + 3.5 + walk_target * 4.0, base_x, base_x)
            } else {
                (BarState::Bar, now + 3.0 + walk_target * 4.0, base_x, base_x)
            }
        }
        // While idling out front: usually keep idling or tend the bar; the
        // occasional stroll.
        BarState::Front => {
            if r < 0.25 {
                let to = walk_to();
                let dur = ((to - base_x).abs() / BUBBLE_WALK_SPEED).max(0.5);
                let dir = if to < base_x {
                    BarState::WalkLeft
                } else {
                    BarState::WalkRight
                };
                (dir, now + dur, base_x, to)
            } else if r < 0.55 {
                (BarState::Bar, now + 3.0 + walk_target * 4.0, base_x, base_x)
            } else {
                (BarState::Front, now + 3.5 + walk_target * 4.0, base_x, base_x)
            }
        }
    }
}

/// Bubble the bartender's little life: he idles facing the room, turns around
/// to fiddle with the bar, and strolls left/right behind the counter — swapping
/// between his front/back/left/right sprites, with a quick squash-and-stretch
/// pulse on every state change and a bob while walking.
fn bar_standee(
    time: Res<Time>,
    mut rng: ResMut<SfxRng>,
    camera: Query<&Transform, (With<Camera3d>, Without<BarStandee>)>,
    mut q: Query<(
        &mut BarStandee,
        &mut Transform,
        &mut MeshMaterial3d<StandardMaterial>,
    )>,
) {
    let Some(cam) = camera.iter().next() else {
        return;
    };
    let cam_pos = cam.translation;
    let now = time.elapsed_secs();
    let dt = time.delta_secs();
    let step = (now * 4.0).floor();

    for (mut b, mut t, mut mat) in &mut q {
        // --- state machine: pick something new to do when the timer runs out ---
        if now >= b.state_until {
            let r = rng.unit();
            let target = rng.unit();
            let (next, until, from, to) = bubble_next(b.state, b.base.x, r, target, now);
            if next != b.state {
                b.squash = BUBBLE_SQUASH_T; // pop a squash-stretch on every switch
            }
            b.state = next;
            b.state_until = until;
            b.walk_from = from;
            b.walk_to = to;
            b.walk_start = now;
        }

        // --- sprite facing ---
        // The billboard quad is rotated to face the camera, which mirrors
        // world-x relative to the texture — so walking left needs the *right*
        // sprite and vice versa for him to look where he's going.
        let idx = match b.state {
            BarState::Front => 0,
            BarState::Bar => 1,
            BarState::WalkLeft => 3,
            BarState::WalkRight => 2,
        };
        if mat.0 != b.mats[idx] {
            mat.0 = b.mats[idx].clone();
        }

        // --- position: stroll or idle-wobble ---
        let mut pos = b.base;
        if matches!(b.state, BarState::WalkLeft | BarState::WalkRight) {
            let span = (b.state_until - b.walk_start).max(0.001);
            let p = ((now - b.walk_start) / span).clamp(0.0, 1.0);
            pos.x = b.walk_from + (b.walk_to - b.walk_from) * p;
            b.base.x = pos.x;
            // A springy step-bob while he walks.
            pos.y += ((now * 9.0).sin()).abs() * 0.12;
        } else {
            let nx = hash11(b.seed * 1.3 + step * 0.0137) * 2.0 - 1.0;
            let ny = hash11(b.seed * 2.1 + step * 0.0211) * 2.0 - 1.0;
            pos += Vec3::new(nx * 0.02, ny * 0.02, 0.0);
        }
        t.translation = pos;
        t.look_at(Vec3::new(cam_pos.x, pos.y, cam_pos.z), Vec3::Y);
        let nlean = hash11(b.seed * 3.7 + step * 0.009) * 2.0 - 1.0;
        t.rotate_local_z(nlean * 0.01);

        // --- squash & stretch pulse on state switches ---
        if b.squash > 0.0 {
            b.squash = (b.squash - dt).max(0.0);
        }
        let pulse = if b.squash > 0.0 {
            (((BUBBLE_SQUASH_T - b.squash) / BUBBLE_SQUASH_T) * std::f32::consts::PI).sin()
        } else {
            0.0
        };
        t.scale = Vec3::new(1.0 + pulse * 0.10, 1.0 - pulse * 0.12, 1.0);
    }
}

/// Bob the single turn-arrow above whoever's turn it is (hidden otherwise).
fn turn_arrow(
    time: Res<Time>,
    poker: Res<Poker>,
    mut q: Query<(&mut Transform, &mut Visibility), With<TurnArrow>>,
) {
    let g = &poker.game;
    // Only point at the AI seats (your own turn is obvious from the buttons).
    let active = g.street != Street::HandOver
        && g.to_act != 0
        && !poker.dealing
        && !poker.comm_anim
        && !poker.pending_deal;
    for (mut tr, mut vis) in &mut q {
        if !active {
            *vis = Visibility::Hidden;
            continue;
        }
        *vis = Visibility::Visible;
        let a = poker.seat_angles[g.to_act];
        let bob = (time.elapsed_secs() * 3.2).sin() * 0.12;
        tr.translation = Vec3::new(a.cos() * poker.prx, 3.95 + bob, a.sin() * poker.prz);
    }
}

/// Move chips flying into the pot; despawn them when they land.
/// Where the pot sits, in the middle of the table (toward the far side a touch).
const POT_CENTER: Vec3 = Vec3::new(0.0, 0.0, -0.55);

/// Throw `amount` worth of chips from `from` into the pot at the table centre.
/// They arc, tumble, and (in `chip_fly`) land and stay as the growing pot pile.
/// Shared by player bets and the blinds at the start of a hand.
fn throw_chips_to_pot(
    commands: &mut Commands,
    assets: &PokerAssets,
    from: Vec3,
    felt_top: f32,
    amount: u32,
    now: f32,
    rng: &mut SfxRng,
) {
    let to_center = Vec3::new(POT_CENTER.x, felt_top + 0.06, POT_CENTER.z);
    // Tumble axis is perpendicular to the throw direction, in the table plane.
    let flat_dir = Vec3::new(to_center.x - from.x, 0.0, to_center.z - from.z).normalize_or_zero();
    let axis = Vec3::new(-flat_dir.z, 0.0, flat_dir.x);

    // One flying chip per physical chip in the denomination breakdown, capped so
    // a big bet doesn't spray dozens of chips.
    let mut colors: Vec<usize> = Vec::new();
    for (color, count) in chip_breakdown(amount) {
        for _ in 0..count.min(4) {
            colors.push(color);
        }
    }
    if colors.len() > 10 {
        colors.truncate(10);
    }
    for (k, color) in colors.iter().enumerate() {
        let jitter = Vec3::new((rng.unit() - 0.5) * 0.6, 0.0, (rng.unit() - 0.5) * 0.6);
        let turns = if k % 2 == 0 { 2.0 } else { 3.0 }; // whole turns → lands flat
        commands.spawn((
            Mesh3d(assets.chip_mesh.clone()),
            MeshMaterial3d(assets.chip_mats[*color].clone()),
            Transform::from_translation(from).with_scale(Vec3::new(0.18, 0.04, 0.18)),
            FlyingChip {
                from,
                to: to_center + jitter,
                start: now + k as f32 * 0.06,
                dur: 0.42,
                arc: 0.7 + rng.unit() * 0.3,
                spin: turns * std::f32::consts::TAU,
                axis,
            },
        ));
    }
}

/// Animate the intro chip-rain: each chip falls from above to its rest height
/// with an ease-out, then the component is removed and it sits still.
fn chip_drop(
    time: Res<Time>,
    mut commands: Commands,
    mut q: Query<(Entity, &ChipDrop, &mut Transform)>,
) {
    let now = time.elapsed_secs();
    for (e, d, mut t) in &mut q {
        let p = ((now - d.start) / d.dur).clamp(0.0, 1.0);
        let eased = 1.0 - (1.0 - p) * (1.0 - p); // ease-out (fast then settle)
        t.translation.y = d.rest_y + 5.0 * (1.0 - eased);
        if p >= 1.0 {
            t.translation.y = d.rest_y;
            commands.entity(e).remove::<ChipDrop>();
        }
    }
}

fn chip_fly(
    time: Res<Time>,
    mut commands: Commands,
    mut q: Query<(Entity, &FlyingChip, &mut Transform)>,
) {
    let now = time.elapsed_secs();
    for (e, fc, mut tr) in &mut q {
        if now >= fc.start + fc.dur {
            // Landed: settle into the pot and stay there (becomes a PotChip so a
            // table redraw won't wipe it). Rest flat with a random spin and a
            // little height variation so the pile looks naturally tossed.
            let ft = fc.to.y - 0.06;
            let hh = hash11(fc.to.x * 31.7 + fc.to.z * 17.3);
            *tr = Transform {
                translation: Vec3::new(fc.to.x, ft + 0.024 + hh * 0.05, fc.to.z),
                rotation: Quat::from_rotation_y(hh * std::f32::consts::TAU),
                scale: Vec3::new(0.18, 0.04, 0.18),
            };
            commands.entity(e).remove::<FlyingChip>().insert(PotChip);
            continue;
        }
        if now < fc.start {
            continue; // staggered launch; wait at the player's stack
        }
        let p = ((now - fc.start) / fc.dur).clamp(0.0, 1.0);
        let mut pos = fc.from.lerp(fc.to, p);
        // Vertical: a main throw arc for the first 80% of the flight, then a
        // small settle-bounce as it lands in the pot.
        let h = if p < 0.8 {
            let q = p / 0.8;
            fc.arc * 4.0 * q * (1.0 - q)
        } else {
            let q = (p - 0.8) / 0.2;
            fc.arc * 0.16 * 4.0 * q * (1.0 - q)
        };
        pos.y += h;
        tr.translation = pos;
        // Tumble through the air, easing to a stop so it lands flat (whole turns).
        let ease = 1.0 - (1.0 - p) * (1.0 - p);
        tr.rotation = Quat::from_axis_angle(fc.axis, fc.spin * ease);
    }
}

fn win_celebrate(
    time: Res<Time>,
    mut poker: ResMut<Poker>,
    mut commands: Commands,
    sfx: Res<Sfx>,
    mut rng: ResMut<SfxRng>,
    mut standees: Query<(&mut Standee, &mut CharAnim)>,
) {
    if poker.paused {
        return;
    }
    if poker.game.street != Street::HandOver {
        poker.celebrated = false;
        return;
    }
    // Wait until the cards have finished rising, then celebrate once.
    if poker.celebrated || poker.showdown_raise < 0.9 {
        return;
    }
    poker.celebrated = true;
    let winners: Vec<usize> = poker.game.last_payouts.iter().map(|p| p.seat).collect();
    for seat in winners {
        let talk_pick = rng.next_u32() as usize;
        for (mut st, mut anim) in &mut standees {
            if st.seat == seat {
                st.bounce = 0.6;
                anim.talk_until = time.elapsed_secs() + 1.2;
                anim.talk_idx = talk_pick;
            }
        }
        if let Some(list) = sfx.chars.get(seat) {
            if let Some(h) = rng.pick(list) {
                play_sfx(&mut commands, &h.clone(), &mut rng);
            }
        }
    }
}

/// Float each seated player's stack above their head (screen-space, projected
/// from the world each frame).
fn money_labels(
    poker: Res<Poker>,
    ui: Res<AppUi>,
    camera: Query<(&Camera, &GlobalTransform)>,
    mut labels: Query<(&MoneyLabel, &mut Node, &mut Text, &mut Visibility)>,
) {
    let Ok((cam, cam_t)) = camera.single() else {
        return;
    };
    if ui.on_title {
        for (_, _, _, mut vis) in &mut labels {
            *vis = Visibility::Hidden;
        }
        return;
    }
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
                // Centre the fixed-width label on the head position.
                node.left = Val::Px(p.x - MONEY_LABEL_W / 2.0);
                node.top = Val::Px(p.y);
                // Stack on the first line; any status/hand on a second line so it
                // stays centred and readable instead of running off sideways.
                let second = if g.street == Street::HandOver {
                    if player.folded {
                        "folded".to_string()
                    } else {
                        g.hand_value(s).map(|hv| hv.category.name().to_string()).unwrap_or_default()
                    }
                } else if player.all_in {
                    "all-in".to_string()
                } else if player.folded {
                    "folded".to_string()
                } else {
                    String::new()
                };
                let content = if second.is_empty() {
                    format!("${}", player.stack)
                } else {
                    format!("${}\n{}", player.stack, second)
                };
                *text = Text::new(content);
                *vis = Visibility::Visible;
            }
            Err(_) => *vis = Visibility::Hidden,
        }
    }
}

/// Fold the title/pause/screenshot overlays into the master freeze flag the
/// game-driving systems already respect.
fn sync_pause(ui: Res<AppUi>, mode: Res<GameMode>, mut poker: ResMut<Poker>) {
    // Title, pause menu, screenshots, and the win screen all freeze the table.
    // (Busting out does NOT freeze it — the AI play on while you spectate.)
    let frozen = ui.screenshot || ui.on_title || ui.paused || *mode == GameMode::Won;
    if poker.paused != frozen {
        poker.paused = frozen;
    }
}

/// Dismiss the title (click / Enter / Space) and toggle the pause menu (Enter).
fn ui_input(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut ui: ResMut<AppUi>,
) {
    if ui.screenshot {
        return;
    }
    if ui.on_title {
        if keys.just_pressed(KeyCode::Enter)
            || keys.just_pressed(KeyCode::Space)
            || mouse.just_pressed(MouseButton::Left)
        {
            ui.on_title = false;
        }
        return;
    }
    if keys.just_pressed(KeyCode::Enter) {
        ui.paused = !ui.paused;
    }
}

/// Show/hide the title and pause overlays from the UI state.
fn ui_overlays(
    ui: Res<AppUi>,
    mut title: Query<&mut Visibility, (With<TitleScreen>, Without<PauseMenu>)>,
    mut pause: Query<&mut Visibility, (With<PauseMenu>, Without<TitleScreen>)>,
) {
    let tv = if ui.on_title {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    for mut v in &mut title {
        *v = tv;
    }
    let pv = if ui.paused {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    for mut v in &mut pause {
        *v = pv;
    }
}

/// Build a fresh random play order over all songs (Fisher–Yates), resetting the
/// position to the start. Avoids opening the new cycle with the song that just
/// finished the previous one, so there's never a back-to-back repeat.
fn reshuffle_songs(music: &mut Music, rng: &mut SfxRng) {
    let n = music.songs.len();
    let prev = music.order.get(music.pos).copied();
    let mut order: Vec<usize> = (0..n).collect();
    for i in (1..n).rev() {
        let j = rng.next_u32() as usize % (i + 1);
        order.swap(i, j);
    }
    if n > 1 {
        if let Some(p) = prev {
            if order[0] == p {
                order.swap(0, n - 1);
            }
        }
    }
    music.order = order;
    music.pos = 0;
}

/// Background music: a fresh looping song at the start of each round, faded out
/// when the round ends, with live volume/mute from the options menu.
fn music_system(
    time: Res<Time>,
    ui: Res<AppUi>,
    poker: Res<Poker>,
    mut music: ResMut<Music>,
    mut rng: ResMut<SfxRng>,
    mut commands: Commands,
    all_tracks: Query<Entity, With<MusicTrack>>,
    mut sinks: Query<&mut AudioSink, With<MusicTrack>>,
) {
    if music.songs.is_empty() || ui.on_title {
        return;
    }
    let street = poker.game.street;

    // Round start (entering Preflop): advance the shuffled play order to the
    // next song (reshuffling once we've played them all) and loop it.
    if street == Street::Preflop && music.last_street != Street::Preflop {
        for e in &all_tracks {
            commands.entity(e).despawn();
        }
        let n = music.songs.len();
        if music.order.is_empty() {
            reshuffle_songs(&mut music, &mut rng);
        } else if music.pos == usize::MAX {
            // First song of the game: use the order shuffled at startup as-is.
            music.pos = 0;
        } else {
            music.pos += 1;
            if music.pos >= music.order.len() {
                reshuffle_songs(&mut music, &mut rng);
            }
        }
        let idx = music.order.get(music.pos).copied().unwrap_or(0) % n;
        music.fade = 1.0;
        music.fading = false;
        let vol = if music.on { music.vol } else { 0.0 };
        commands.spawn((
            AudioPlayer(music.songs[idx].clone()),
            PlaybackSettings {
                mode: PlaybackMode::Loop,
                volume: Volume::Linear(vol),
                ..default()
            },
            MusicTrack,
        ));
    }
    // Round end (entering HandOver): fade out.
    if street == Street::HandOver && music.last_street != Street::HandOver {
        music.fading = true;
    }
    music.last_street = street;

    // Drive the fade and apply live volume/mute to the playing track.
    if music.fading {
        music.fade = (music.fade - time.delta_secs() / 1.4).max(0.0);
    }
    let target = if music.on { music.vol * music.fade } else { 0.0 };
    for mut sink in &mut sinks {
        sink.set_volume(Volume::Linear(target));
    }
    if music.fading && music.fade <= 0.0 {
        for e in &all_tracks {
            commands.entity(e).despawn();
        }
        music.fading = false;
    }
}

/// Drag the options-menu volume bars and reflect the live values in the fills
/// and the on/off check-marks.
fn volume_controls(
    ui: Res<AppUi>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut sfx: ResMut<SfxRng>,
    mut music: ResMut<Music>,
    tracks: Query<(&VolSlider, &RelativeCursorPosition)>,
    mut fills: Query<(&VolFill, &mut Node)>,
    mut marks: Query<(&VolToggleMark, &mut Visibility)>,
    mut drag: Local<Option<VolKind>>,
) {
    if !ui.paused {
        *drag = None;
        return;
    }
    if !mouse.pressed(MouseButton::Left) {
        *drag = None;
    }
    for (slider, rel) in &tracks {
        if mouse.just_pressed(MouseButton::Left) && rel.cursor_over {
            *drag = Some(slider.0);
        }
        if *drag == Some(slider.0) {
            if let Some(n) = rel.normalized {
                let v = (n.x + 0.5).clamp(0.0, 1.0);
                match slider.0 {
                    VolKind::Sfx => sfx.sfx_vol = v,
                    VolKind::Music => music.vol = v,
                }
            }
        }
    }
    for (fill, mut node) in &mut fills {
        let v = match fill.0 {
            VolKind::Sfx => sfx.sfx_vol,
            VolKind::Music => music.vol,
        };
        node.width = Val::Percent(v * 100.0);
    }
    for (mark, mut vis) in &mut marks {
        let on = match mark.0 {
            VolKind::Sfx => sfx.sfx_on,
            VolKind::Music => music.on,
        };
        // Inherited (not Visible): Visible would force the check-marks to render
        // even while the pause menu itself is hidden — green X's floating over
        // the table.
        *vis = if on {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

/// Toggle a channel on/off when its checkbox is clicked.
fn volume_toggle(
    ui: Res<AppUi>,
    q: Query<(&VolToggle, &Interaction), Changed<Interaction>>,
    mut sfx: ResMut<SfxRng>,
    mut music: ResMut<Music>,
) {
    if !ui.paused {
        return;
    }
    for (tog, interaction) in &q {
        if *interaction == Interaction::Pressed {
            match tog.0 {
                VolKind::Sfx => sfx.sfx_on = !sfx.sfx_on,
                VolKind::Music => music.on = !music.on,
            }
        }
    }
}

/// Reset every stack to the buy-in and deal a fresh hand (a brand-new game).
fn reset_game(poker: &mut Poker) {
    for p in &mut poker.game.players {
        p.stack = 1000;
        p.folded = false;
        p.all_in = false;
        p.bet = 0;
        p.committed = 0;
    }
    poker.game.small_blind = BLIND_LEVELS[0].0;
    poker.game.big_blind = BLIND_LEVELS[0].1;
    poker.game.start_hand();
    poker.pending_deal = true;
    poker.dealing = false;
    poker.comm_anim = false;
    poker.comm_shown = 0;
    poker.showdown_raise = 0.0;
    poker.celebrated = false;
    poker.log = "New game".to_string();
}

/// Once a hand has settled, decide whether the human is out (busted) or has
/// cleaned everyone else out (won).
fn game_over_detect(mut mode: ResMut<GameMode>, poker: Res<Poker>) {
    if *mode != GameMode::Playing {
        return;
    }
    if poker.game.street != Street::HandOver
        || poker.dealing
        || poker.pending_deal
        || poker.comm_anim
        || poker.showdown_raise < 0.9
    {
        return;
    }
    let human_out = poker.game.players[0].stack == 0;
    let others_out = poker.game.players.iter().skip(1).all(|p| p.stack == 0);
    if others_out && !human_out {
        *mode = GameMode::Won;
    } else if human_out {
        *mode = GameMode::Busted;
    }
}

/// While the human spectates from the bar, deal the AI's hands automatically.
fn spectate_advance(
    mode: Res<GameMode>,
    time: Res<Time>,
    mut poker: ResMut<Poker>,
    mut needs: ResMut<NeedsRedraw>,
    mut timer: Local<f32>,
) {
    if *mode != GameMode::Busted {
        *timer = 0.0;
        return;
    }
    let settled = poker.game.street == Street::HandOver
        && !poker.dealing
        && !poker.comm_anim
        && !poker.pending_deal
        && poker.showdown_raise > 0.9;
    if settled {
        *timer += time.delta_secs();
        if *timer > 2.5 {
            *timer = 0.0;
            poker.game.start_hand();
            poker.pending_deal = true;
            poker.showdown_raise = 0.0;
            poker.log = "New hand".to_string();
            needs.0 = true;
        }
    } else {
        *timer = 0.0;
    }
}

/// Smoothly move the camera to a spectator spot when the human busts. Mostly it
/// watches the table from afar, but every so often it turns to look over at the
/// bar where the other knocked-out players are nursing their drinks.
fn camera_rig(
    mode: Res<GameMode>,
    time: Res<Time>,
    mut cam: Query<&mut Transform, With<MainCamera>>,
    mut phase: Local<f32>,
    mut at_bar: Local<bool>,
) {
    let Ok(mut t) = cam.single_mut() else {
        return;
    };
    let (target, look) = if *mode == GameMode::Busted {
        // Cycle: ~10s watching the table, then ~5s glancing over at the bar.
        *phase += time.delta_secs();
        let dwell = if *at_bar { 5.0 } else { 10.0 };
        if *phase > dwell {
            *phase = 0.0;
            *at_bar = !*at_bar;
        }
        if *at_bar {
            // Seated at the bar end, looking down the line of stools.
            (Vec3::new(-10.5, 3.2, -9.5), Vec3::new(3.0, 2.2, -12.0))
        } else {
            // Well back by the bar, taking in the whole table from afar.
            (Vec3::new(-10.5, 4.6, -9.0), Vec3::new(1.2, 0.9, 1.6))
        }
    } else {
        *phase = 0.0;
        *at_bar = false;
        (Vec3::new(0.0, 4.7, 10.4), Vec3::new(0.0, 1.8, -2.0))
    };
    let k = (time.delta_secs() * 1.6).min(1.0);
    let pos = t.translation.lerp(target, k);
    let cur_look = t.rotation * Vec3::NEG_Z; // smoothly turn the gaze too
    let new_look = pos + cur_look.lerp((look - pos).normalize_or_zero(), k);
    *t = Transform::from_translation(pos).looking_at(new_look, Vec3::Y);
}

/// Drive the end-of-game overlay: headline, the New Game / Penthouse buttons,
/// and their clicks (restart, or buy into the penthouse).
fn game_over_ui(
    mut mode: ResMut<GameMode>,
    mut env: ResMut<Environment>,
    mut poker: ResMut<Poker>,
    mut clock: ResMut<BlindClock>,
    mut intro: ResMut<IntroDrop>,
    mut needs: ResMut<NeedsRedraw>,
    mut root: Query<&mut Visibility, (With<GameOverRoot>, Without<GameOverBtn>)>,
    mut title: Query<&mut Text, (With<GameOverTitle>, Without<GameOverSub>)>,
    mut sub: Query<&mut Text, (With<GameOverSub>, Without<GameOverTitle>)>,
    mut buttons: Query<
        (
            &GameOverBtn,
            &Interaction,
            &mut Visibility,
            &mut BackgroundColor,
        ),
        (With<Button>, Without<GameOverRoot>),
    >,
) {
    let m = *mode;
    let show = m != GameMode::Playing;
    for mut v in &mut root {
        *v = if show {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
    for mut t in &mut title {
        *t = Text::new(match m {
            GameMode::Busted => "BUSTED OUT",
            GameMode::Won => "YOU CLEANED THEM OUT!",
            GameMode::Playing => "",
        });
    }
    for mut t in &mut sub {
        *t = Text::new(match m {
            GameMode::Busted => "Nursing a drink at the bar while the game plays on...",
            GameMode::Won => "Cash out and buy into the high-roller game uptown?",
            GameMode::Playing => "",
        });
    }
    let mut clicked: Option<GameOverBtn> = None;
    for (btn, interaction, mut vis, mut bg) in &mut buttons {
        // The penthouse offer only appears on a win.
        let visible = show && (*btn == GameOverBtn::NewGame || m == GameMode::Won);
        *vis = if visible {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if !visible {
            continue;
        }
        match *interaction {
            Interaction::Pressed => {
                *bg = BackgroundColor(Color::srgb(0.16, 0.55, 0.28));
                clicked = Some(*btn);
            }
            Interaction::Hovered => *bg = BackgroundColor(Color::srgb(0.26, 0.5, 0.32)),
            Interaction::None => *bg = BackgroundColor(Color::srgb(0.2, 0.42, 0.26)),
        }
    }
    if let Some(b) = clicked {
        *env = match b {
            GameOverBtn::Penthouse => Environment::Penthouse,
            GameOverBtn::NewGame => Environment::Bar,
        };
        reset_game(&mut poker);
        clock.active = 0.0;
        clock.level = 0;
        intro.0 = true; // rain the chips in for the new game
        *mode = GameMode::Playing;
        needs.0 = true;
    }
}

/// Hold Tab to peek at the HUD detail panel.
fn hud_panel_toggle(
    keys: Res<ButtonInput<KeyCode>>,
    mut q: Query<&mut Visibility, With<HudPanel>>,
) {
    let show = keys.pressed(KeyCode::Tab);
    for mut v in &mut q {
        *v = if show {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
    }
}

/// Keep the "Pot $X" label sitting just under the pot chips in the middle.
fn pot_label(
    poker: Res<Poker>,
    ui: Res<AppUi>,
    camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    mut q: Query<(&mut Node, &mut Text, &mut Visibility), With<PotLabel>>,
) {
    let Ok((cam, cam_t)) = camera.single() else {
        return;
    };
    let pot = if ui.on_title { 0 } else { poker.game.pot() };
    // Just toward the camera from the pot pile, on the felt.
    let anchor = Vec3::new(0.0, poker.felt_top, -0.05);
    for (mut node, mut text, mut vis) in &mut q {
        if pot == 0 {
            *vis = Visibility::Hidden;
            continue;
        }
        match cam.world_to_viewport(cam_t, anchor) {
            Ok(p) => {
                node.left = Val::Px(p.x - MONEY_LABEL_W / 2.0);
                node.top = Val::Px(p.y);
                *text = Text::new(format!("Pot ${pot}"));
                *vis = Visibility::Visible;
            }
            Err(_) => *vis = Visibility::Hidden,
        }
    }
}

/// Show the last action ("Hoodguy folds") as a transient line at the top of the
/// screen: fade in, drift upward, fade out.
fn action_ticker(
    time: Res<Time>,
    poker: Res<Poker>,
    ui: Res<AppUi>,
    mut q: Query<(&mut Node, &mut Text, &mut TextColor), With<ActionTicker>>,
    mut last: Local<String>,
    mut shown_at: Local<f32>,
) {
    let now = time.elapsed_secs();
    if ui.on_title {
        *last = poker.log.clone(); // swallow startup log so it doesn't pop in
        for (_, _, mut color) in &mut q {
            *color = TextColor(Color::srgba(0.96, 0.95, 0.88, 0.0));
        }
        return;
    }
    if poker.log != *last {
        *last = poker.log.clone();
        *shown_at = now;
        for (_, mut text, _) in &mut q {
            *text = Text::new(poker.log.clone());
        }
    }
    let age = now - *shown_at;
    // Fade in over 0.25s, hold, fade out 2.0..3.0s, drifting up the whole time.
    let alpha = if age < 0.25 {
        age / 0.25
    } else if age < 2.0 {
        1.0
    } else if age < 3.0 {
        1.0 - (age - 2.0)
    } else {
        0.0
    };
    for (mut node, _, mut color) in &mut q {
        node.top = Val::Px(74.0 - (age.min(3.0)) * 6.0);
        *color = TextColor(Color::srgba(0.96, 0.95, 0.88, alpha));
    }
}

/// Swivel each chair around its standee so it stays directly behind the
/// billboard as seen from the live camera — the standee then never clips it.
fn chair_rig(mut q: Query<(&ChairPart, &mut Transform)>) {
    for (part, mut t) in &mut q {
        // Fixed: a chair behind each seat, set radially outward from the table
        // centre and facing inward, so it never swings over the table (the old
        // camera-relative placement pushed it onto the felt from oblique views).
        let rad = Vec2::new(part.anchor.x, part.anchor.z).normalize_or_zero();
        let cyaw = (-rad.x).atan2(-rad.y); // backrest faces the table centre
        let crot = Quat::from_rotation_y(cyaw);
        let back = Vec3::new(
            part.anchor.x + rad.x * 0.5,
            part.anchor.y + 2.0,
            part.anchor.z + rad.y * 0.5,
        );
        let pos = if part.is_post {
            let cright = crot * Vec3::X;
            Vec3::new(back.x, part.anchor.y + 1.3, back.z) + cright * (0.84 * part.post)
        } else {
            back
        };
        *t = Transform::from_translation(pos).with_rotation(crot);
    }
}

/// Pin the spectate beer in front of the camera (lower-right, upright) while
/// you're busted, and hide it otherwise.
fn spectate_beer(
    mode: Res<GameMode>,
    camera: Query<&Transform, (With<MainCamera>, Without<SpectateBeer>)>,
    mut q: Query<(&mut Transform, &mut Visibility), With<SpectateBeer>>,
) {
    let busted = *mode == GameMode::Busted;
    let Ok(cam) = camera.single() else {
        return;
    };
    for (mut t, mut vis) in &mut q {
        *vis = if busted {
            Visibility::Visible
        } else {
            Visibility::Hidden
        };
        if busted {
            let pos = cam.translation
                + *cam.forward() * 1.5
                + *cam.right() * 0.6
                + *cam.up() * -0.62;
            // Keep it upright in world space (don't tilt with the camera).
            t.translation = pos;
            t.rotation = Quat::IDENTITY;
        }
    }
}

/// Keep the winner glow billboard facing the live camera (and tucked just behind
/// the winner from its view), so the halo reads from any angle.
fn win_glow(
    camera: Query<&Transform, (With<MainCamera>, Without<WinGlow>)>,
    mut q: Query<(&WinGlow, &mut Transform)>,
) {
    let Ok(cam) = camera.single() else {
        return;
    };
    let cam_pos = cam.translation;
    for (g, mut t) in &mut q {
        let to_cam = (cam_pos - g.anchor).normalize_or_zero();
        *t = Transform::from_translation(g.anchor - to_cam * 0.6)
            .looking_at(cam_pos, Vec3::Y)
            .with_scale(Vec3::splat(5.0));
    }
}

/// Keep the name plates facing the live camera (text side toward it).
fn name_plates(
    camera: Query<&Transform, (With<MainCamera>, Without<NamePlate>)>,
    mut q: Query<&mut Transform, With<NamePlate>>,
) {
    let Ok(cam) = camera.single() else {
        return;
    };
    let cam_pos = cam.translation;
    for mut t in &mut q {
        let pos = t.translation;
        let away = pos + (pos - Vec3::new(cam_pos.x, pos.y, cam_pos.z));
        t.look_at(away, Vec3::Y);
    }
}

/// Advance the blind clock and bump the blinds every 5 minutes of active play.
/// Stalls while the game is paused or the player has been idle for over 30s.
fn blind_clock(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    mut cursor: MessageReader<bevy::window::CursorMoved>,
    mut poker: ResMut<Poker>,
    mut clock: ResMut<BlindClock>,
) {
    let now = time.elapsed_secs();
    let input = keys.get_just_pressed().next().is_some()
        || mouse.get_just_pressed().next().is_some()
        || cursor.read().next().is_some();
    if input {
        clock.last_input = now;
    }
    let idle = now - clock.last_input;
    if !poker.paused && idle < 30.0 {
        clock.active += time.delta_secs();
    }
    let target = ((clock.active / BLIND_INTERVAL) as usize).min(BLIND_LEVELS.len() - 1);
    if target > clock.level {
        clock.level = target;
        let (sb, bb) = BLIND_LEVELS[target];
        poker.game.small_blind = sb;
        poker.game.big_blind = bb;
        poker.log = format!("Blinds up — {sb}/{bb}");
    }
}

/// Show the penthouse-only props when that room is active, hide them otherwise.
fn environment_visibility(env: Res<Environment>, mut q: Query<&mut Visibility, With<EnvPenthouse>>) {
    let want = if *env == Environment::Penthouse {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };
    for mut v in &mut q {
        if *v != want {
            *v = want;
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

#[cfg(test)]
mod sfx_tests {
    use super::*;

    /// The folder scan must find every deal*.wav, and random picks must hit all
    /// of them — guards the "is it really switching between my sounds?" path.
    #[test]
    fn deal_sounds_are_all_discovered_and_picked() {
        let files = scan_sfx();
        let deals: Vec<&String> = files
            .iter()
            .filter(|f| {
                f.rsplit('/')
                    .next()
                    .unwrap_or(f)
                    .to_lowercase()
                    .starts_with("deal")
            })
            .collect();
        assert!(
            deals.len() >= 10,
            "expected at least 10 deal sounds, found {}: {:?}",
            deals.len(),
            deals
        );

        // Picking 500 times must select every file (uniform RNG over 10 items).
        let mut rng = SfxRng::new(12345);
        let mut hits = vec![0u32; deals.len()];
        for _ in 0..500 {
            let p = rng.pick(&deals).unwrap();
            let i = deals.iter().position(|d| d == p).unwrap();
            hits[i] += 1;
        }
        assert!(
            hits.iter().all(|&h| h > 0),
            "some deal sounds were never picked: {hits:?}"
        );
    }
}

#[cfg(test)]
mod bubble_tests {
    use super::*;

    /// Bubble's behaviour must actually produce walks (and valid ones): a stroll
    /// should head to a target inside his roaming range with the correct facing.
    #[test]
    fn bubble_walks_often_and_within_bounds() {
        let mut walks = 0;
        let n = 1000;
        for i in 0..n {
            let r = i as f32 / n as f32; // sweep the full 0..1 roll
            let target = ((i * 37) % n) as f32 / n as f32;
            let (next, until, from, to) = bubble_next(BarState::Front, -7.5, r, target, 10.0);
            assert!(until > 10.0, "next decision must be in the future");
            if matches!(next, BarState::WalkLeft | BarState::WalkRight) {
                walks += 1;
                assert!(
                    (BUBBLE_WALK_MIN..=BUBBLE_WALK_MAX).contains(&to),
                    "walk target {to} out of range"
                );
                assert_eq!(from, -7.5, "walk should start from current position");
                let correct_dir = if to < -7.5 {
                    BarState::WalkLeft
                } else {
                    BarState::WalkRight
                };
                assert_eq!(next, correct_dir, "walk facing must match direction");
            }
        }
        // He strolls now and then (~25% from Front) but mostly idles.
        assert!(
            walks > n / 8 && walks < n / 2,
            "walk frequency out of band: {walks}/{n}"
        );
    }

    /// All four directional sprites must be discovered from the Bubble folder
    /// (not the single-image fallback) now that the art is in the repo.
    #[test]
    fn bubble_directional_sprites_are_discovered() {
        let s = bubble_sprites();
        for (i, facing) in ["front", "back", "left", "right"].iter().enumerate() {
            assert!(
                s[i].to_lowercase().contains(facing),
                "facing {facing} resolved to fallback: {}",
                s[i]
            );
        }
    }
}

#[cfg(test)]
mod char_frame_tests {
    use super::*;

    /// Frame discovery keys files by name (default/talk/back/sit) from a
    /// character's folder, and returns nothing for characters without one.
    #[test]
    fn char_frames_discovered_by_keyword() {
        let dir = std::path::Path::new("assets/characters/__testchar");
        std::fs::create_dir_all(dir).unwrap();
        for f in [
            "Test_default1.png",
            "Test_default2.png",
            "Test_talk1.png",
            "Test_back.png",
            "Test_sit.png",
            "notes.txt", // ignored: not a png
        ] {
            std::fs::write(dir.join(f), b"x").unwrap();
        }
        let (d, t, b, s) = char_frame_files("__testchar");
        std::fs::remove_dir_all(dir).unwrap();
        assert_eq!(d.len(), 2, "two default frames: {d:?}");
        assert_eq!(t.len(), 1, "one talk frame: {t:?}");
        assert!(b.unwrap().ends_with("Test_back.png"));
        assert!(s.unwrap().ends_with("Test_sit.png"));

        let (d, t, b, s) = char_frame_files("no_such_character");
        assert!(d.is_empty() && t.is_empty() && b.is_none() && s.is_none());
    }
}
