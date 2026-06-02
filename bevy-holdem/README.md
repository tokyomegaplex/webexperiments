# Cartoon Hold'em — Bevy (3D, native desktop)

A from-scratch rebuild of the poker game in [Bevy](https://bevy.org/), Rust's
ECS game engine, for a real 3D table (perspective camera, lighting) instead of
the 2.5D canvas. This lives alongside the original web/React version (in the
repo root) — neither replaces the other yet.

## Status

**Phase 1 — the 3D table scene** (this commit):
- Angled perspective camera + directional/ambient lighting.
- Felt table with a wooden rail, on a floor.
- The 5 characters seated around the back arc as upright billboard standees
  (PNG textures with a colored-quad fallback).
- Community-card slots + the human's hole cards as 3D meshes.

**Phase 2 — coming next:**
- Port the poker engine (deck, hand evaluation, betting state machine, side
  pots) and the Monte-Carlo + personality AI from the JS version into Rust.
- Betting UI (Bevy UI), turn flow, dealer/character speech.
- Card faces (textured quads) and chip stacks.

## Run it

You need Rust (https://rustup.rs). On macOS no extra system libs are needed.

```
cd bevy-holdem
cargo run
```

First build downloads + compiles Bevy (a few minutes); after that it's fast.
A window opens with the 3D table. Drag-to-orbit isn't wired yet — the camera is
a fixed angle for now.

## Art

Drop your 5 character PNGs in `assets/characters/` (see the README there for the
exact filenames). They load automatically; missing ones fall back to a flat
colored quad.

## Linux build note

On Linux you need `libasound2-dev libudev-dev libxkbcommon-dev` (and a Vulkan/GL
driver to run). macOS and Windows need nothing extra.
