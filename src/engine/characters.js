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

export const CHARACTERS = [
  {
    id: 'maximillian',
    name: 'Maximillian',
    avatar: '🤖',
    color: '#5b8def',
    blurb: 'A polite calculating android. Plays by the numbers, rarely bluffs.',
    profile: { aggression: 0.45, tightness: 0.62, bluff: 0.08, tilt: 0.05 },
    lines: {
      raise: ['Probability favors aggression.', 'Recalculating... raise.'],
      call: ['A reasonable call.', 'I will see this card.'],
      fold: ['Insufficient equity. I fold.', 'Logic dictates retreat.'],
      win: ['As computed.', 'Outcome: optimal.'],
      bigWin: ['Variance, resolved in my favor.'],
      lose: ['An acceptable loss.', 'Noted for next iteration.'],
      bluffCaught: ['A rare deviation. Curious.'],
    },
    power: null,
  },
  {
    id: 'rosie',
    name: 'Rosie Rattlesnake',
    avatar: '🐍',
    color: '#3fbf7f',
    blurb: 'A drawling desert gambler who bluffs like she breathes.',
    profile: { aggression: 0.78, tightness: 0.32, bluff: 0.42, tilt: 0.3 },
    lines: {
      raise: ['Let\'s make it interestin\', sugar.', 'Push them chips on in.'],
      call: ['I\'ll tag along.', 'Sure, why not.'],
      fold: ['Not this time, darlin\'.', 'I\'ll let y\'all have it.'],
      win: ['Ha! Told ya.', 'Easy money.'],
      bigWin: ['Whoo-ee! Rake it on over here!'],
      lose: ['Rattlesnakes bite back, remember that.'],
      bluffCaught: ['...how\'d you know?'],
    },
    power: null,
  },
  {
    id: 'mortimer',
    name: 'Mortimer',
    avatar: '👻',
    // A hooded figure rendered from a PNG standee. Drop the art at
    // public/characters/hoodguy.png and it loads automatically; until then a
    // hand-drawn hooded fallback stands in. (Replaces the old Baron seat.)
    image: 'characters/hoodguy.png',
    color: '#7a1f1f',
    blurb: 'A silent hooded figure with enormous eyes. Reads souls, raises big.',
    profile: { aggression: 0.85, tightness: 0.28, bluff: 0.5, tilt: 0.45 },
    lines: {
      raise: ['...', '(a long, knowing stare) ...raise.'],
      call: ['(the hood tilts)', '...call.'],
      fold: ['(the hood sinks lower)', '...not this one.'],
      win: ['(the eyes gleam)', '...mine.'],
      bigWin: ['(a low, rattling laugh)'],
      lose: ['(the hood droops)'],
      bluffCaught: ['(the eyes go very wide)'],
    },
    power: null,
  },
  {
    id: 'pip',
    name: 'Pip',
    avatar: '🐹',
    color: '#f5a623',
    blurb: 'A jittery rookie hamster. Tight, nervous, folds at the first scare.',
    profile: { aggression: 0.3, tightness: 0.78, bluff: 0.1, tilt: 0.6 },
    lines: {
      raise: ['Um... raise? Is that okay?', 'I-I think this one\'s good!'],
      call: ['Okay, okay, I\'ll call.', 'Just a little call...'],
      fold: ['Nope nope nope, I fold!', 'Too scary, I\'m out!'],
      win: ['I won?! I WON!', 'Yay! Wait, really?'],
      bigWin: ['THIS IS THE BEST DAY EVER!'],
      lose: ['Awww, peanuts...'],
      bluffCaught: ['Eep! Sorry!'],
    },
    power: null,
  },
  {
    id: 'gusto',
    name: 'Chef Gusto',
    avatar: '👨‍🍳',
    color: '#e74c3c',
    blurb: 'A boisterous chef who treats every pot like a simmering sauce.',
    profile: { aggression: 0.6, tightness: 0.5, bluff: 0.25, tilt: 0.2 },
    lines: {
      raise: ['Add a little spice — raise!', 'Turn up the heat!'],
      call: ['I\'ll have a taste. Call.', 'Mmm, I\'ll stay in the pot.'],
      fold: ['This dish is spoiled. Fold.', 'Send it back, I fold.'],
      win: ['Bellissimo! Delicious!', 'Chef\'s kiss!'],
      bigWin: ['A FEAST! The whole pot is mine!'],
      lose: ['Burnt it. Bah.'],
      bluffCaught: ['You caught me sampling early!'],
    },
    power: null,
  },
  {
    id: 'shade',
    name: 'Madame Shade',
    avatar: '🔮',
    color: '#8e44ad',
    blurb: 'A mysterious fortune teller. Patient, sharp, strikes when sure.',
    profile: { aggression: 0.55, tightness: 0.68, bluff: 0.2, tilt: 0.1 },
    lines: {
      raise: ['The cards foretold this. Raise.', 'Destiny... demands more.'],
      call: ['I have foreseen your move. Call.', 'The spirits say... stay.'],
      fold: ['This timeline is not mine.', 'I release this hand.'],
      win: ['It was written.', 'The crystal never lies.'],
      bigWin: ['The fates pour their fortune upon me.'],
      lose: ['A cloudy vision, this time.'],
      bluffCaught: ['Even seers misdirect.'],
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
