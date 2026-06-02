//! The cartoon cast. Each character maps to a PNG in `assets/characters/`
//! (named by `file`) plus a personality `profile` for the AI (added in the
//! engine port). Until a PNG exists, the standee shows as a flat colored quad
//! tinted with `color`.
//!
//! NOTE: filenames below are my best guess from the art you sent — rename them
//! (here and the PNGs) to match whatever you saved.

use bevy::prelude::*;

// `id`, `profile`, and `blurb` are consumed by the phase-2 engine/AI port.
#[allow(dead_code)]
#[derive(Clone)]
pub struct CharDef {
    pub id: &'static str,
    pub name: &'static str,
    pub file: &'static str,
    pub color: Color,
    /// aggression, tightness, bluff, tilt — used by the AI port.
    pub profile: [f32; 4],
    pub blurb: &'static str,
}

pub fn roster() -> Vec<CharDef> {
    vec![
        CharDef {
            id: "buck",
            name: "Buck",
            file: "characters/buck.png",
            color: Color::srgb_u8(199, 154, 30),
            profile: [0.40, 0.70, 0.12, 0.55],
            blurb: "Big-toothed bundle of nerves. Grinds it out, rarely bluffs.",
        },
        CharDef {
            id: "gnash",
            name: "Gnash",
            file: "characters/gnash.png",
            color: Color::srgb_u8(154, 127, 192),
            profile: [0.90, 0.24, 0.58, 0.50],
            blurb: "Manic purple gremlin — all teeth and chaos. Bluffs constantly.",
        },
        CharDef {
            id: "grin",
            name: "Grin",
            file: "characters/grin.png",
            color: Color::srgb_u8(212, 160, 23),
            profile: [0.48, 0.32, 0.22, 0.12],
            blurb: "Cheerful blob who never met a hand it didn't like. Calls everything.",
        },
        CharDef {
            id: "frost",
            name: "Frost",
            file: "characters/frost.png",
            color: Color::srgb_u8(154, 163, 174),
            profile: [0.50, 0.66, 0.10, 0.05],
            blurb: "Stone-faced figure in a red cap. Patient, calculating, ice-cold.",
        },
        CharDef {
            id: "hoodguy",
            name: "Hoodguy",
            file: "characters/hoodguy.png",
            color: Color::srgb_u8(122, 31, 31),
            profile: [0.85, 0.28, 0.50, 0.45],
            blurb: "Silent hooded figure with enormous eyes. Reads souls, raises big.",
        },
    ]
}
