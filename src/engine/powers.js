// ---------------------------------------------------------------------------
// powers.js — the extension seam for turning base Hold'em into YOUR game.
//
// The base game ships with ZERO powers. Everything here is plumbing so that
// later you can give each cartoon character a special ability without forking
// the engine. The engine emits named "hooks" at every meaningful moment; a
// power is just a bundle of hook handlers.
//
// A Power looks like:
//   {
//     id: 'sneaky-peek',
//     name: 'Sneaky Peek',
//     description: 'Once per hand, glimpse the next community card.',
//     hooks: {
//       onHandStart(ctx) { ... },
//       onShowdown(ctx)  { ... },
//       // ...any hook name below
//     },
//   }
//
// Hooks receive a ctx object: { game, player, event, ... } and may mutate
// game state directly (the engine owns no secrets from powers — they are
// trusted house rules). Some hooks are "filters": whatever you return replaces
// the value. See HOOKS for the contract of each.
// ---------------------------------------------------------------------------

// The full catalog of moments the engine broadcasts. Documented here so future
// powers (and you) know exactly what is interceptable.
export const HOOKS = {
  onHandStart: 'A new hand begins (after dealer rotation, before blinds).',
  onDeal: 'Cards have just been dealt to players (hole cards).',
  onStreet: 'A new community street is revealed (flop/turn/river). ctx.street.',
  beforeAction: 'A player is about to act. ctx.legal = allowed actions.',
  afterAction: 'A player has acted. ctx.action = what they did.',
  filterEvaluation:
    'FILTER: return a replacement hand-evaluation for a player at showdown.',
  filterAiDecision:
    'FILTER: return a replacement action for an AI decision, or null to pass.',
  onShowdown: 'Hands are revealed; winners not yet decided.',
  onHandEnd: 'Pot(s) awarded; hand fully resolved.',
}

export class PowerRegistry {
  constructor(powers = []) {
    this.powers = []
    for (const p of powers) this.register(p)
  }

  register(power) {
    if (!power || !power.id) throw new Error('Power needs an id')
    this.powers.push(power)
    return this
  }

  // Fire-and-forget broadcast: every power with this hook runs, in order.
  emit(hookName, ctx) {
    for (const power of this.powers) {
      const fn = power.hooks?.[hookName]
      if (fn) fn({ ...ctx, power })
    }
  }

  // Filter chain: each handler may return a replacement value, threaded along.
  // Returns the final value (or the provided initial value if nobody handled).
  filter(hookName, value, ctx) {
    let current = value
    for (const power of this.powers) {
      const fn = power.hooks?.[hookName]
      if (fn) {
        const next = fn({ ...ctx, power, value: current })
        if (next !== undefined) current = next
      }
    }
    return current
  }
}

// A registry the whole engine shares. The base game leaves it empty.
export const noPowers = () => new PowerRegistry([])
