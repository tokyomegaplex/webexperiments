# Pokermon Hold 'em

A 3D cartoon Texas Hold 'em game, built from scratch in Rust with
[Bevy](https://bevy.org/). You play seat 0 against five characters around a
felt table in a dim bar — with chip stacks, card art, music, voices, and a
smoky ashtray you can actually pick up.

## Run it

You need Rust — install from [rustup.rs](https://rustup.rs). Run these from this
folder (the repo root once split out; `bevy-holdem/` inside webexperiments).

```sh
cargo run --release
```

The first build compiles Bevy and takes a few minutes; after that it's quick.
Use `--release` — a debug build renders the 3D scene sluggishly.

## Play

- **Bet** with the on-screen buttons (Fold / Check / Call / Raise) and the
  raise slider.
- **Hand Guide** — bottom-right button, opens a hand-rankings reference with
  example cards for every hand.
- **Tutorial** — bottom-right toggle. Walks you through the game one screen at
  a time (click **Next** to advance); the table stays frozen while a screen is
  up, then it coaches you contextually as the hand plays out.
- **Cigarette** — click one resting on the ashtray to raise it for a drag, then
  it settles back onto the rim.
- **Enter** pauses and opens the audio options.

## Build a macOS .app

```sh
./bundle-mac.sh
```

Produces `Pokermon Hold 'em.app` — double-clickable, with the bundled assets
downscaled so it isn't enormous. Drop a `pokermon icon.png` into `assets/` and
it gets embedded as the app icon. Zip it to share. It's unsigned, so the first
launch needs right-click → Open, and an Apple-Silicon build won't run on Intel
Macs (or vice versa).

## Tests

The poker engine (deck, hand evaluation, betting state machine, side pots, AI)
is pure Rust with no graphics, so the rules are verifiable without a window:

```sh
cargo test
```

Covers hand-category ordering, tiebreaks, the wheel straight, side pots, split
pots, and full AI hands for chip conservation and termination.

## Layout

| Path | What's in it |
| --- | --- |
| `src/main.rs` | Bevy app: scene, rendering, animation, UI, input |
| `src/poker.rs` | The engine — cards, hand evaluation, betting, AI |
| `src/poker/tests.rs` | Engine unit tests |
| `assets/` | Character art, card faces, chips, music, sound effects |
| `bundle-mac.sh` | macOS `.app` packaging |

## Art

Character PNGs live in `assets/characters/<Name>/`, discovered by keyword in
the filename (`_sit`, `_talk1`, `_default1`, `_back`, …). Missing art falls
back to a flat colored quad, so the game still runs.

## Linux notes

Building needs `libasound2-dev libudev-dev libxkbcommon-dev` (plus
`libwayland-dev` on Wayland) and a working Vulkan/GL driver. macOS and Windows
need nothing extra.
