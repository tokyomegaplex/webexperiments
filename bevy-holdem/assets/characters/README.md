# Character art (Bevy version)

Bevy loads assets from this `assets/` tree. The 5 character PNGs (filenames are
case-sensitive, matched in `src/characters.rs`):

| File           | Character |
|----------------|-----------|
| `Jaack.png`    | Jaack — yellow, big teeth, pink vest |
| `Stinky.png`   | Stinky — purple toothy gremlin |
| `Mejdk.png`    | Mejdk — yellow blob, arms wide |
| `Funnyguy.png` | Funnyguy — gray figure, red cap |
| `Hoodguy.png`  | Hoodguy — red hooded figure |

Each is drawn on an upright quad that faces the camera. Until a file is present,
the standee shows as a flat colored quad (the character's `color`).

Note: these PNGs are added locally and are not committed to the repo. Commit
them if you want them on a fresh clone.
