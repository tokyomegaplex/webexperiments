// ---------------------------------------------------------------------------
// examplePowers.js — TEMPLATES, not wired into the base game.
//
// This file exists to show you, concretely, how to turn base Hold'em into your
// own game by writing powers against the hooks in powers.js. None of these are
// active until you register them. To use one:
//
//   import { PowerRegistry } from './powers.js'
//   import { extraHoleCard } from './examplePowers.js'
//   const powers = new PowerRegistry([extraHoleCard('gnash')])
//   const game = new PokerGame({ players, powers })
//
// Then (optionally) attach a power to a character by setting `character.power`
// in characters.js and registering it here. Mix, match, and invent your own.
// ---------------------------------------------------------------------------

// 1) DEAL-TIME power: give one character a third hole card ("Texas Hold'em,
//    but the Baron cheats"). Demonstrates the onDeal hook + reading game state.
export function extraHoleCard(ownerId) {
  return {
    id: `extra-card-${ownerId}`,
    name: 'Hidden Ace',
    description: 'This player is dealt a third hole card.',
    hooks: {
      onDeal({ game }) {
        const p = game.state.players.find((x) => x.id === ownerId)
        if (p && p.inHand) p.hole.push(game.state.deck.pop())
      },
    },
  }
}

// 2) EVALUATION power: let a character treat one suit as wild. Demonstrates the
//    filterEvaluation FILTER hook — return a replacement hand result. (Here we
//    just bump their category as a simple illustration; a real version would
//    re-run evaluateHand with substituted cards.)
export function luckySuit(ownerId, suit = 'h') {
  return {
    id: `lucky-suit-${ownerId}`,
    name: 'Lucky Suit',
    description: `Hearts give this player an edge at showdown.`,
    hooks: {
      filterEvaluation({ player, value }) {
        if (player.id !== ownerId) return value
        const hearts = player.hole.filter((c) => c.suit === suit).length
        if (!hearts) return value
        // Nudge the tiebreak score upward; replace with real wild-card logic.
        return { ...value, score: [...value.score, hearts] }
      },
    },
  }
}

// 3) DECISION power: a "tell reader" that makes one AI play better by peeking
//    at pot context. Demonstrates filterAiDecision (return an action to
//    override, or undefined/null to let the normal AI decide).
export function bluffCatcher(ownerId) {
  return {
    id: `bluff-catcher-${ownerId}`,
    name: 'Bluff Catcher',
    description: 'Never folds a made hand to a single raise.',
    hooks: {
      filterAiDecision({ game, player, value }) {
        if (player.id !== ownerId) return value
        // Example override: if facing only the big blind, always at least call.
        if (value?.type === 'fold' && game.state.currentBet <= game.state.bigBlind) {
          return { type: 'call' }
        }
        return value
      },
    },
  }
}

// 4) ROUND power: the dealer "burns the river" — a chaotic house rule that
//    swaps the final community card each hand. Demonstrates onStreet.
export function chaoticRiver() {
  return {
    id: 'chaotic-river',
    name: 'Chaotic River',
    description: 'The river card is re-drawn once for extra swing.',
    hooks: {
      onStreet({ game, street }) {
        if (street !== 'river') return
        const board = game.state.community
        game.state.deck.push(board.pop()) // put it back
        board.push(game.state.deck.shift()) // draw a different one
      },
    },
  }
}
