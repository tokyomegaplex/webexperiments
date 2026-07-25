# Character art

Drop the four character PNGs here, named exactly as below. They load
automatically; until a file exists, a colored stand-in (with the character's
emoji face) is drawn instead.

| File         | Character | Vibe                                   |
|--------------|-----------|----------------------------------------|
| `buck.png`   | Buck      | big-toothed, nervous grinder           |
| `gnash.png`  | Gnash     | manic purple gremlin, all teeth        |
| `grin.png`   | Grin      | cheerful blob, calls everything        |
| `frost.png`  | Frost     | stone-faced figure in a red cap        |

After adding the files, shrink them for the web:

```
npm run art:optimize
```

Tips for standee art:
- Transparent background (PNG).
- Portrait-ish; the character is anchored by its base and drawn upright behind
  the table, so full-body or waist-up both work.
