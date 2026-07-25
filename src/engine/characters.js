// ---------------------------------------------------------------------------
// characters.js — the cartoon cast.
//
// Each character bundles:
//   - cosmetic identity (name, color, emoji avatar placeholder)
//   - a personality `profile` that drives the AI (see ai.js)
//   - `lines`: barks shown in speech bubbles to give them personality
//   - `power`: null in the base game — your hook into powers.js later.
//
// These are original characters (not from any existing game). Swap the emoji
// avatars and colors for your own art when you're ready; the engine only cares
// about `id`, `profile`, and (later) `power`.
//
// profile fields (all roughly 0..1 unless noted):
//   aggression — how often it raises vs. just calls
//   tightness  — how strong a hand it needs to stay in (higher = folds more)
//   bluff      — how often it bets weak hands as a bluff
//   tilt       — how much losing rattles it (raises variance after a bad beat)
// ---------------------------------------------------------------------------

// The cast. Each `image` points at a PNG under public/ (see public/characters).
// Until you drop the file in, a colored stand-in is drawn so the game still
// runs. Save your art with these exact filenames and they load automatically.
export const CHARACTERS = [
  {
    id: 'buck',
    name: 'Buck',
    avatar: '😬',
    image: 'characters/buck.png',
    color: '#c79a1e',
    blurb: 'A big-toothed bundle of nerves. Grinds it out, rarely bluffs, tilts hard.',
    profile: { aggression: 0.4, tightness: 0.7, bluff: 0.12, tilt: 0.55 },
    lines: {
      raise: ['O-okay... raise.', 'Gotta push here, right? Right.', '*gulp* Raise.'],
      call: ['I-I\'ll call.', 'Just a call. Just a call.'],
      fold: ['Nope. Folding.', 'Too rich for me!', '*nervous laugh* Out.'],
      win: ['I... I won?', 'Phew! Okay!'],
      bigWin: ['HA! Did you SEE that?!'],
      lose: ['Knew it. Knew it.'],
      bluffCaught: ['How did you— never mind.'],
    },
    power: null,
  },
  {
    id: 'gnash',
    name: 'Gnash',
    avatar: '😈',
    image: 'characters/gnash.png',
    color: '#9a7fc0',
    blurb: 'A manic purple gremlin who is all teeth and chaos. Bluffs constantly.',
    profile: { aggression: 0.9, tightness: 0.24, bluff: 0.58, tilt: 0.5 },
    lines: {
      raise: ['RAISE! Hehehe!', 'More! MORE!', 'Chomp chomp — raise!'],
      call: ['Eh, I\'ll bite.', 'Sure sure sure, call.'],
      fold: ['Boooring. Fold.', 'Pfft. Take it.'],
      win: ['MINE! All mine!', 'Nyahaha!'],
      bigWin: ['FEAST! A glorious FEAST!'],
      lose: ['Grrrr! Rigged!'],
      bluffCaught: ['Ack! You peeked!'],
    },
    power: null,
  },
  {
    id: 'grin',
    name: 'Grin',
    avatar: '😁',
    image: 'characters/grin.png',
    color: '#d4a017',
    blurb: 'A cheerful blob who never met a hand it didn\'t like. Calls everything.',
    profile: { aggression: 0.48, tightness: 0.32, bluff: 0.22, tilt: 0.12 },
    lines: {
      raise: ['Why not! Raise!', 'Feelin\' good — raise!'],
      call: ['Sure, I\'m in!', 'Call! This is fun!', 'Ooh, let\'s see more!'],
      fold: ['Aw. Maybe next one!', 'Okay okay, fold.'],
      win: ['Yaaay!', 'We did it!'],
      bigWin: ['BEST. POT. EVER!'],
      lose: ['Heehee, oh well!'],
      bluffCaught: ['Worth a shot!'],
    },
    power: null,
  },
  {
    id: 'frost',
    name: 'Frost',
    avatar: '😐',
    image: 'characters/frost.png',
    color: '#9aa3ae',
    blurb: 'A stone-faced figure in a red cap. Patient, calculating, ice-cold tells.',
    profile: { aggression: 0.5, tightness: 0.66, bluff: 0.1, tilt: 0.05 },
    lines: {
      raise: ['Raise.', 'The math says raise.'],
      call: ['Call.', 'I\'ll see it.'],
      fold: ['No.', 'Fold.'],
      win: ['Naturally.', 'Expected.'],
      bigWin: ['...good hand.'],
      lose: ['Hm.'],
      bluffCaught: ['...noted.'],
    },
    power: null,
  },
]

// The dealer narrates the table. Not a player; never holds cards in the base
// game. Give it a power later and it could become a wild force at the table.
export const DEALER = {
  id: 'dealer',
  name: 'Dottie the Dealer',
  avatar: '🎴',
  color: '#1f2d3d',
  blurb: 'The unflappable host. Keeps the game moving with a wink and a quip.',
  lines: {
    handStart: ['New hand, fresh fortunes!', 'Shuffle up and deal!', 'Here we go, folks.'],
    flop: ['And the flop comes down...', 'Three on the felt!'],
    turn: ['The turn...', 'Fourth street, here it is.'],
    river: ['And the river!', 'Last card, make it count.'],
    showdown: ['Cards on the table!', 'Let\'s see what you\'ve got.'],
    win: (name) => [`${name} takes it down!`, `The pot goes to ${name}!`],
    fold: ['Everyone folded — easy pot!', 'Taken without a fight!'],
  },
  power: null,
}

export function pickOpponents(count) {
  // Deterministic-ish selection: take the first `count` from a shuffled copy
  // so each session feels varied but balanced.
  const pool = CHARACTERS.slice()
  for (let i = pool.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1))
    ;[pool[i], pool[j]] = [pool[j], pool[i]]
  }
  return pool.slice(0, Math.max(1, Math.min(count, CHARACTERS.length)))
}

export function randomLine(lines, key, ...args) {
  const entry = lines?.[key]
  if (!entry) return ''
  const arr = typeof entry === 'function' ? entry(...args) : entry
  return arr[Math.floor(Math.random() * arr.length)]
}
