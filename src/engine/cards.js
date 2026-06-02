// ---------------------------------------------------------------------------
// cards.js — the 52-card deck primitives.
//
// A Card is a plain object: { rank, suit }.
//   rank: integer 2..14  (11=J, 12=Q, 13=K, 14=A)
//   suit: 's' | 'h' | 'd' | 'c'
//
// Keeping cards as plain data (no classes) makes them trivial to serialize,
// clone, and — importantly for custom games later — to invent new ones.
// ---------------------------------------------------------------------------

export const SUITS = ['s', 'h', 'd', 'c']
export const RANKS = [2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14]

export const SUIT_SYMBOLS = { s: '♠', h: '♥', d: '♦', c: '♣' }
export const SUIT_COLORS = { s: '#222', c: '#222', h: '#d4264a', d: '#d4264a' }
export const RANK_LABELS = {
  2: '2', 3: '3', 4: '4', 5: '5', 6: '6', 7: '7', 8: '8', 9: '9', 10: '10',
  11: 'J', 12: 'Q', 13: 'K', 14: 'A',
}

export function createDeck() {
  const deck = []
  for (const suit of SUITS) {
    for (const rank of RANKS) {
      deck.push({ rank, suit })
    }
  }
  return deck
}

// Mulberry32 — a tiny, fast, *seedable* PRNG. Seeding lets us reproduce a
// shuffle for tests/debugging or "replay" a hand. Pass no seed for randomness.
export function makeRng(seed = (Math.random() * 2 ** 32) >>> 0) {
  let a = seed >>> 0
  return function rng() {
    a |= 0
    a = (a + 0x6d2b79f5) | 0
    let t = Math.imul(a ^ (a >>> 15), 1 | a)
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296
  }
}

// Fisher-Yates shuffle. Returns a NEW array; does not mutate the input.
export function shuffle(cards, rng = Math.random) {
  const a = cards.slice()
  for (let i = a.length - 1; i > 0; i--) {
    const j = Math.floor(rng() * (i + 1))
    ;[a[i], a[j]] = [a[j], a[i]]
  }
  return a
}

export function cardToString(card) {
  return RANK_LABELS[card.rank] + card.suit
}
