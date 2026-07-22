# TITMOUSE × Y2K GPU BOX ART — Design Brief

A creative direction pack + copy bank for a hoodie graphic inspired by late-'90s / early-'00s
3D graphics-card box art — the "*Overclocked*" era. Think **NVIDIA GeForce, 3dfx Voodoo, ATI Radeon,
Matrox, S3, PowerVR** retail boxes: an airbrushed chrome fantasy creature bursting out of a dark void,
smothered in chrome type, starbursts, and spec badges shouting about features nobody understood.

The companion file `index.html` is a **live custom renderer** that generates art in exactly this style,
starring a chrome Titmouse-bird mascot. Use it to pose/screenshot the hero; use this doc for the words,
type, and scene ideas.

---

## 1. The aesthetic, decoded

What actually makes something read as "Y2K GPU box art":

| Ingredient | Why it matters | How to get it |
|---|---|---|
| **Chrome / liquid-metal hero** | The card can render reflective metal, so the box proves it | Mirror-finish creature, environment reflections, hot rim light |
| **Impossible creature** | Shows off "organic" shading the hardware can supposedly do | Dragon, mermaid, alien, cyber-animal — here: a **cyber-titmouse** |
| **Feature it can't obviously do** | Fur, water, translucency, iridescence = "look, per-pixel shaders!" | Fur tufts, splashing water, oil-slick iridescence, glass |
| **Nuclear rim lighting** | Cinematic, dramatic, "next-gen" | Cyan + magenta back-rims, warm key, lens flare |
| **Dark void + starfield** | Makes the chrome and neon pop | Deep navy→black gradient, floating particles |
| **Chrome bevelled type** | The logo has to look injection-molded | Extended italic, metallic gradient fill, hard bevel + drop shadow |
| **Spec badges & starbursts** | Marketing noise; the more the better | Rounded gel pills, sunburst "SUPERCHARGED!" seal |
| **A fake model number** | GeForce2 GTS, Voodoo5 5500… | Invent one: *GX-9000 ULTRA* |

Adjacent references to raid: **demoscene** intros, **Winamp** skins, **Y2K frost/chrome** web
graphics, **Lisa Frank meets Terminator 2**, Vegas-90s trade-show booths, holographic Pog slammers,
`.mod` tracker UIs, and the airbrushed van/T-shirt art the box painters actually came from.

---

## 2. Copy bank — the words on the box

Mix a **big chrome headline**, a **fake model line**, **6–8 spec badges**, **one starburst seal**,
and a strip of **fine-print jargon**. (All of these are editable in the `COPY` object at the top of
`index.html`.)

### Big chrome headlines / hero words
- **ADVANCED GRAPHIX** · **SUPERCHARGED** · **MAXIMUM OVERDRIVE** · **FULLY ACCELERATED**
- **TITMOUSE VISUAL PROCESSING UNIT** · **REALTIME EVERYTHING** · **BEYOND REAL-TIME**
- **HYPER-RENDER** · **INFINITE DETAIL ENGINE** · **PIXELS UNLEASHED** · **RENDER GOD MODE**
- **NEXT-GEN GRAPHIX** · **EXTREME VISUAL POWER** · **THE FUTURE OF FRAMES**

### Fake model / product names (the "GeForce2 Ultra" slot)
- **GX-9000 ULTRA** · **TITMOUSE FURY MX** · **BIRDCORE 4600 Ti** · **VOODOO-TIT 5**
- **RADEON-EST** · **CHIRP-X 1900XTX** · **NESTFORCE 256** · **PLUMEON GTS**
- **TALON HD 3D** · **MEGABIRD 12000** · **FEATHERMAX PRO** · **AVI-ONE TERAFLOP EDITION**

### Spec / feature badges (short, punchy — gel pills)
`PHYS-X` · `512-BIT` · `128MB DDR` · `256MB` · `REAL-TIME FUR` · `HDR LIGHTING` · `4× AGP`
`T&L ENGINE` · `ANTI-ALIAS` · `16× ANISOTROPIC` · `PER-PIXEL SHADING` · `BUMP MAPPED`
`ENVIRONMENT MAPPING` · `TRUE COLOR` · `FSAA` · `TRILINEAR` · `HARDWARE T&L` · `VERTEX SHADER`
`64-BIT COLOR` · `2.1 GIGATEXEL/S` · `FEATHER-MAPPING™` · `WATER FX` · `IRIDESCENCE ENGINE`
`SUB-PIXEL PRECISION` · `DIRECT-BEAK 9` · `OPEN-WING 2.0` · `MIP-MAPPED` · `Z-BUFFERED`

### Starburst seal words (the sunburst sticker)
- **SUPER-CHARGED!** · **NEW!** · **NOW WITH FUR!** · **3D READY!** · **AS SEEN IN 3D!**
- **60 FPS+!** · **FEATHERS INCLUDED** · **FREE PHYS-X!** · **OVERCLOCKED!**

### Fine-print jargon strip (the tiny confident nonsense)
- `T&L ENGINE • ANISOTROPIC • ANTI-ALIAS • PER-PIXEL SHADING`
- `POWERED BY THE TITMOUSE VISUAL PROCESSING UNIT`
- `HARDWARE-ACCELERATED FUR · WATER · IRIDESCENCE · TRANSLUCENCY`
- `REQUIRES: IMAGINATION. NOT COMPATIBLE WITH REALITY.`
- `©20XX TITMOUSE INC. ALL PIXELS RESERVED. ACTUAL FRAMES MAY VARY.`

### Titmouse-flavored in-jokes (studio Easter eggs)
- **"DRAWS ITS OWN FRAMES"** · **"HAND-KEYED AT 24 FPS, RENDERED AT 9000"**
- **ANIMATION ACCELERATION UNIT** · **"NOW YOUR STORYBOARDS RENDER THEMSELVES"**
- **BIG MOUTH RENDERING PIPELINE** · **THE STUDIO THAT SHIPS PIXELS**

---

## 3. Type / font direction

Real box art leaned on **Eurostile Bold Extended, Bank Gothic, Handel Gothic, Microgramma,
Compacta,** and custom chrome logotypes. Free web-safe stand-ins that nail the vibe:

| Role | Font (free / Google Fonts) | Emulates | Treatment |
|---|---|---|---|
| **Hero chrome logo** | **Orbitron** (900), *italic-sheared* | custom chrome logotype | metallic gradient fill, hard bevel, cyan drop-glow |
| Alt hero | **Zen Dots**, **Audiowide**, **Wallpoet** | Compacta / display chrome | same chrome treatment |
| Model line / sub-brand | **Michroma**, **Saira Stencil One** | Eurostile Extended | wide tracking, single accent color |
| Spec badges | **Rajdhani** (700), **Chakra Petch** | Bank Gothic / Isonorm | ALL CAPS, tight, dark text on gel pill |
| Fine print / ticker | **Chakra Petch**, **Share Tech Mono** | technical mono | low-contrast, wide letterspacing |
| "Extreme" accents | **Bungee**, **Racing Sans One** | racing/airbrush script | sparingly, for a starburst word |

**Chrome text recipe** (used in the renderer, reproducible in any design app):
1. Set the word in a heavy extended face, **italic-shear ~-12°**.
2. Fill with a **vertical metal gradient**: `white → pale-blue → deep-blue at the 50% "horizon" → mid-blue → white`. The hard dark band across the middle is what sells "chrome."
3. **Bevel:** thin light stroke on the top-left edge, dark stroke on the bottom-right.
4. **Drop shadow / outer glow** in cyan or magenta, plus a solid dark offset shadow for depth.
5. Optional: a single white **specular streak** across the top third.

Pairing rule: **one loud chrome display face + one calm techy face.** Never more than two families.

---

## 4. Hero & scene ideas (the shader showcase)

Each classic box picked ONE creature and used it to brag about ONE marquee feature. Here's a menu,
all bendable toward the **Titmouse bird**:

### Titmouse-native heroes (recommended — on brand)
- **Chrome Cyber-Titmouse** *(the renderer's default)* — liquid-metal tufted titmouse, crest as
  energy blades, bursting up from water. Sells: **reflections + fur + water.**
- **The Titmouse-from-the-deep** — your "weird monster with fur coming out of a body of water" idea,
  reimagined as a **giant kaiju titmouse** rising from a chrome ocean, water sheeting off matted fur,
  glowing eyes. Sells: **fur shader + water + subsurface.**
- **Iridescent Oil-Slick Titmouse** — feathers as a thin-film/oil rainbow. Sells: **iridescence.**
- **Wireframe→Skin Titmouse** — split down the middle: one half glowing wireframe/vertex cage, one
  half fully skinned chrome. The classic "before/after the GPU" gag. Sells: **T&L / shading pipeline.**
- **Mecha-Titmouse** — chromed battle-mech bird with panel lines, vents, and a giant fan (GPU cooler)
  in its chest. Sells: **hardware / overclock.**

### Generic box-art heroes (swap the creature if you want variety)
- Chrome **dragon** mid-roar with molten breath (fire/particle FX)
- **Mermaid / fairy** with translucent wings (the NVIDIA "Dawn/Nalu" homage) — sub-surface & hair
- **Alien warrior** with slime/goo (translucency)
- **Cyber-wolf or panther** made of fur + chrome plates (fur + metal)
- **Crystal golem** refracting the scene (refraction/glass)
- Floating **eyeball / brain-in-a-jar** GPU core with cables (pure techno-camp)

### Scene-building blocks (layer 2–3 of these)
Reflective black water · alien nebula sky · a distant chrome city/grid horizon · floating debris &
sparks · volumetric god-rays · a giant lens flare from an off-screen sun · a HUD/targeting reticle
overlay · scanlines & CRT curvature · a wireframe planet · tessellated terrain in the background.

---

## 5. Color palettes

| Palette | Base | Rim / accents | Mood |
|---|---|---|---|
| **Void** *(default)* | deep navy → black | cyan + hot magenta | classic 3dfx night |
| **Sunset** | plum → orange → gold | warm gold + pink | airbrushed van / vaporwave |
| **Cyber** | teal-black → electric blue | acid cyan + magenta | Matrix / arcade |
| **Studio** | charcoal → slate | white softbox + ice-blue | clean product-shot chrome |

Rule of thumb: **one warm key, two cool/neon rims, near-black background.** Keep the creature
mostly desaturated chrome so the neon lighting and the gel badges carry the color.

---

## 6. Layout / composition (the box grid)

```
┌─────────────────────────────────────────────┐
│ CHROME WORDMARK            ▓ badge ▓ badge   │  ← logo top-left, badges top/right rail
│ model line                 ▓ badge ▓ badge   │
│                                              │
│              ★ HERO CREATURE ★               │  ← hero center, slightly right, bursting up
│              (bursting toward you)           │
│                                    ✷ STAR    │  ← sunburst seal mid-right
│                                     BURST    │
│  BIG CHROME HEADLINE                         │  ← headline lower-left
│  · fine-print jargon ticker ·                │
│ [ footer spec bar: brand · features ]        │  ← spec bar pinned to bottom
└─────────────────────────────────────────────┘
```

- **Diagonal energy:** italic type + a creature leaning/turning gives the whole thing motion.
- **Depth stack:** background void → particles → creature → foreground splash/flare → type on top.
- **Breathing room:** the hero needs a dark "halo" around it so the chrome edge separates.
- **One focal starburst,** not five. It's the loudest object; let it be alone.

---

## 7. Turning it into a hoodie (production notes)

- **Composition for apparel:** a **big front-and-center hero** with minimal type reads best across a
  chest; the full "box" layout is better for a **back print**. Consider a small chrome wordmark on the
  front, the full box art on the back.
- **Contrast:** on a **black/charcoal hoodie** the void background disappears into the garment — the
  chrome, neon, and badges float. This is the money look. Pick the *Void* or *Cyber* palette.
- **Export:** in `index.html`, hit **EXPORT PNG (4K)** for a 3840-wide composited PNG (art + type),
  or **Clean render** for the 3D only (add type in your design app). Pose first, then export.
- **Print method:** heavy neon + gradients + fine chrome bevels → **DTG or a screen-printed
  simulated-process / halftone** job. Ask the printer for a **simulated process** or **CMYK+white
  underbase** on dark garments so the neons stay bright. Keep smallest type ≥ ~6 mm tall.
- **Vector the type:** re-set the headline/wordmark as vectors for crisp print; the 3D render can stay
  raster at 300 DPI (≈ a 12" print from the 4K export).
- **Trap the neons:** add a thin dark keyline around gel badges so they don't halo on press.

---

## 8. Using the renderer (`index.html`)

Open it in any modern browser (it's fully self-contained — Three.js is vendored in `vendor/`, no
internet needed). Controls:

- **Body finish:** Chrome · Iridescent · Holo-Energy (three custom shader looks)
- **Backdrop:** Void · Sunset · Cyber · Studio
- **Bloom / Chroma-CRT / Auto-spin / Water level** sliders
- **Drag** to orbit, **scroll** to zoom, **`H`** hides the panel, **`E`** exports
- **EXPORT PNG (4K)** — composited art + type · **Clean render** — 3D only, no overlay
- Edit the **`COPY`** object at the top of the file to change every word on the art
- Tune materials/lights in the clearly-commented sections (`HERO`, `LIGHTS`, `CUSTOM SHADERS`,
  `POST PROCESSING`, `2D OVERLAY`)

Sample renders live in `samples/`.
