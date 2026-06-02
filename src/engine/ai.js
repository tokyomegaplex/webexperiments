// ---------------------------------------------------------------------------
// ai.js — opponent brains ("smarter heuristics").
//
// decideAction(game, seat) returns an action object the engine understands:
//   { type:'fold'|'check'|'call'|'raise', total? }
//
// How it "thinks":
//   1. Estimate win equity with a quick Monte Carlo rollout — deal out random
//      opponent hole cards + remaining board many times and count how often
//      this hand wins/ties. This is real equity, not a lookup table, so it
//      handles draws, multiway pots, and any board.
//   2. Compare equity to the pot odds it's being offered.
//   3. Layer the character's PERSONALITY on top: aggression (raise more),
//      tightness (need more equity to continue), bluff (sometimes bet air).
//
// It's deliberately readable and tweakable — the personality numbers live in
// characters.js, and a future "power" can override the whole decision via the
// filterAiDecision hook.
// ---------------------------------------------------------------------------

import { createDeck } from './cards.js'
import { evaluateHand, compareScore } from './handEvaluator.js'

const DEFAULT_PROFILE = { aggression: 0.5, tightness: 0.5, bluff: 0.2, tilt: 0.2 }

// Estimate equity of `hole` against `opponents` random hands on `board`.
function estimateEquity(hole, board, opponents, iterations) {
  if (opponents <= 0) return 1

  const seen = new Set(
    [...hole, ...board].map((c) => c.rank * 4 + suitIndex(c.suit)),
  )
  const stub = createDeck().filter(
    (c) => !seen.has(c.rank * 4 + suitIndex(c.suit)),
  )

  let win = 0
  let tie = 0
  for (let it = 0; it < iterations; it++) {
    // Partial Fisher-Yates: only shuffle as many cards as we need to draw.
    const need = opponents * 2 + (5 - board.length)
    const drawn = drawN(stub, need)

    let k = 0
    const oppHoles = []
    for (let o = 0; o < opponents; o++) {
      oppHoles.push([drawn[k++], drawn[k++]])
    }
    const fullBoard = board.concat(drawn.slice(k))

    const myScore = evaluateHand(hole.concat(fullBoard)).score
    let best = 0 // 0 = I'm best so far, 1 = an opp beats me, 0.5 = tie
    let tied = false
    for (const oh of oppHoles) {
      const cmp = compareScore(evaluateHand(oh.concat(fullBoard)).score, myScore)
      if (cmp > 0) {
        best = 1
        break
      } else if (cmp === 0) {
        tied = true
      }
    }
    if (best === 1) continue
    if (tied) tie++
    else win++
  }
  return (win + tie * 0.5) / iterations
}

// Draw N distinct random cards from `stub` without mutating it.
function drawN(stub, n) {
  const pool = stub.slice()
  const out = []
  for (let i = 0; i < n; i++) {
    const j = (Math.random() * (pool.length - i)) | 0
    const pick = pool[j]
    pool[j] = pool[pool.length - 1 - i]
    out.push(pick)
  }
  return out
}

function suitIndex(s) {
  return { s: 0, h: 1, d: 2, c: 3 }[s]
}

export function decideAction(game, seat) {
  const st = game.state
  const p = st.players[seat]
  const legal = game.legalActions(seat)
  if (!legal) return { type: 'check' }

  // Powers may completely override the AI's choice. They run last (see below)
  // so they can react to what the heuristic decided.
  const finalize = (action) =>
    game.powers.filter('filterAiDecision', action, { game, player: p, legal })

  const profile = { ...DEFAULT_PROFILE, ...(p.character?.profile || {}) }

  const opponents = game.inHandPlayers().filter((o) => o.seat !== seat).length
  // More iterations preflop (cheap eval) and fewer multiway (expensive eval).
  const iterations = st.community.length === 0 ? 240 : Math.max(120, 360 - opponents * 50)
  let equity = estimateEquity(p.hole, st.community, opponents, iterations)

  // --- pot odds -----------------------------------------------------------
  const toCall = legal.call ?? 0
  const pot = game.pot()
  const potOdds = toCall > 0 ? toCall / (pot + toCall) : 0

  // Tightness raises the bar for continuing; tilt loosens it after a bad beat.
  const requirement = potOdds + (profile.tightness - 0.5) * 0.18

  // --- decide -------------------------------------------------------------
  const r = Math.random()

  // Strong hand: value-bet/raise often.
  const wantsRaise =
    legal.raise &&
    (equity > 0.62 + (0.12 * (1 - profile.aggression)) ||
      (equity > 0.5 && r < profile.aggression * 0.5))

  // Occasional pure bluff with weak equity.
  const wantsBluff =
    legal.raise &&
    equity < 0.35 &&
    r < profile.bluff * (st.community.length >= 3 ? 1 : 0.4)

  if (wantsRaise || wantsBluff) {
    return finalize({ type: 'raise', total: chooseRaiseSize(game, legal, equity, profile, wantsBluff) })
  }

  // No bet to face: check (free card) unless we wanted to raise (handled above).
  if (toCall <= 0) {
    return finalize({ type: legal.check ? 'check' : 'call' })
  }

  // Facing a bet: call if equity clears the (personality-adjusted) bar.
  if (equity >= requirement) {
    return finalize({ type: 'call' })
  }

  // Small mercy: call tiny bets with marginal equity (don't fold for cheap).
  if (potOdds < 0.12 && equity > 0.2 && r < 0.5) {
    return finalize({ type: 'call' })
  }

  return finalize({ type: 'fold' })
}

function chooseRaiseSize(game, legal, equity, profile, isBluff) {
  const st = game.state
  const pot = game.pot()
  const { minTotal, maxTotal } = legal.raise

  // Base sizing as a fraction of the pot, scaled by aggression and equity.
  let frac = 0.5 + profile.aggression * 0.5 + (equity - 0.5) * 0.6
  if (isBluff) frac = 0.6 + profile.aggression * 0.4
  frac = Math.max(0.33, Math.min(frac, 1.3))

  let target = st.currentBet + Math.round(pot * frac)

  // Shove with the nuts-ish hands now and then.
  if (equity > 0.85 && Math.random() < profile.aggression) target = maxTotal

  return Math.max(minTotal, Math.min(target, maxTotal))
}
