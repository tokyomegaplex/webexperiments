// ---------------------------------------------------------------------------
// handEvaluator.js — rank any 5–7 card hand into a comparable score.
//
// evaluateHand(cards) returns:
//   {
//     category: 0..8,        // see CATEGORY below (higher = better)
//     score: [c, t1, t2...], // array compared lexicographically; bigger wins
//     name: 'Full House',
//     cards: [ ...best5 ],   // the 5 cards that make the hand
//   }
//
// Two hands are compared with compareScore(a.score, b.score):
//   >0 if a wins, <0 if b wins, 0 if tie (split pot).
//
// The evaluator is deliberately self-contained and rules-agnostic so a custom
// game can reuse it, replace it, or wrap it (e.g. a "wild card" power could
// pre-process the cards before calling in here).
// ---------------------------------------------------------------------------

export const CATEGORY = {
  HIGH_CARD: 0,
  PAIR: 1,
  TWO_PAIR: 2,
  TRIPS: 3,
  STRAIGHT: 4,
  FLUSH: 5,
  FULL_HOUSE: 6,
  QUADS: 7,
  STRAIGHT_FLUSH: 8,
}

export const CATEGORY_NAMES = {
  0: 'High Card',
  1: 'Pair',
  2: 'Two Pair',
  3: 'Three of a Kind',
  4: 'Straight',
  5: 'Flush',
  6: 'Full House',
  7: 'Four of a Kind',
  8: 'Straight Flush',
}

// Lexicographic comparison of two score arrays.
export function compareScore(a, b) {
  const n = Math.max(a.length, b.length)
  for (let i = 0; i < n; i++) {
    const x = a[i] ?? 0
    const y = b[i] ?? 0
    if (x !== y) return x - y
  }
  return 0
}

// Score exactly five cards.
function scoreFive(five) {
  const ranks = five.map((c) => c.rank).sort((a, b) => b - a)
  const suits = five.map((c) => c.suit)

  const isFlush = suits.every((s) => s === suits[0])

  // Build a descending list of distinct ranks for straight detection.
  const distinct = [...new Set(ranks)].sort((a, b) => b - a)
  let straightHigh = 0
  if (distinct.length === 5) {
    if (distinct[0] - distinct[4] === 4) {
      straightHigh = distinct[0]
    } else if (
      // Wheel: A-2-3-4-5, where the Ace plays low and 5 is the high card.
      distinct[0] === 14 &&
      distinct[1] === 5 &&
      distinct[2] === 4 &&
      distinct[3] === 3 &&
      distinct[4] === 2
    ) {
      straightHigh = 5
    }
  }

  // Count occurrences of each rank.
  const counts = {}
  for (const r of ranks) counts[r] = (counts[r] || 0) + 1
  // Sort ranks by (count desc, rank desc) — the canonical tiebreak ordering.
  const grouped = Object.keys(counts)
    .map(Number)
    .sort((a, b) => counts[b] - counts[a] || b - a)
  const countShape = grouped.map((r) => counts[r]).join('') // e.g. '32', '22 1'

  if (straightHigh && isFlush) {
    return { category: CATEGORY.STRAIGHT_FLUSH, score: [8, straightHigh] }
  }
  if (countShape.startsWith('4')) {
    return { category: CATEGORY.QUADS, score: [7, ...grouped] }
  }
  if (countShape.startsWith('32')) {
    return { category: CATEGORY.FULL_HOUSE, score: [6, ...grouped] }
  }
  if (isFlush) {
    return { category: CATEGORY.FLUSH, score: [5, ...ranks] }
  }
  if (straightHigh) {
    return { category: CATEGORY.STRAIGHT, score: [4, straightHigh] }
  }
  if (countShape.startsWith('3')) {
    return { category: CATEGORY.TRIPS, score: [3, ...grouped] }
  }
  if (countShape.startsWith('22')) {
    return { category: CATEGORY.TWO_PAIR, score: [2, ...grouped] }
  }
  if (countShape.startsWith('2')) {
    return { category: CATEGORY.PAIR, score: [1, ...grouped] }
  }
  return { category: CATEGORY.HIGH_CARD, score: [0, ...ranks] }
}

// All k-combinations of an array (indices), used to pick the best 5 of 7.
function combinations(arr, k) {
  const result = []
  const combo = []
  function backtrack(start) {
    if (combo.length === k) {
      result.push(combo.slice())
      return
    }
    for (let i = start; i < arr.length; i++) {
      combo.push(arr[i])
      backtrack(i + 1)
      combo.pop()
    }
  }
  backtrack(0)
  return result
}

export function evaluateHand(cards) {
  if (cards.length < 5) {
    throw new Error(`evaluateHand needs at least 5 cards, got ${cards.length}`)
  }
  let best = null
  for (const combo of combinations(cards, 5)) {
    const scored = scoreFive(combo)
    if (!best || compareScore(scored.score, best.score) > 0) {
      best = { ...scored, cards: combo }
    }
  }
  best.name = CATEGORY_NAMES[best.category]
  return best
}
