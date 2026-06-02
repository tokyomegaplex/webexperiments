# Character art (Bevy version)

Bevy loads assets from this `assets/` tree. Drop the 5 character PNGs here,
named to match `src/characters.rs` (rename both sides to taste):

| File          | Character |
|---------------|-----------|
| `buck.png`    | Buck — yellow, big teeth, pink vest |
| `gnash.png`   | Gnash — purple toothy gremlin |
| `grin.png`    | Grin — yellow blob, arms wide |
| `frost.png`   | Frost — gray figure, red cap |
| `hoodguy.png` | Hoodguy — red hooded figure |

Transparent-background PNGs work best; each is drawn on an upright quad that
faces the camera. Until a file is present, the standee shows as a flat colored
quad (the character's `color`).
