// ---------------------------------------------------------------------------
// pokerGame.js — the Texas Hold'em state machine.
//
// This class is pure logic: no React, no canvas, no timers. The UI reads
// `game.state` and calls methods (startHand, act, ...); after each call it
// re-reads the state to redraw. Keeping it framework-free is what lets you
// reuse the engine, unit-test it, or drop it into a different shell later.
//
// Mutation seam: a PowerRegistry (powers.js) is threaded through and pinged at
// every important moment. The base game passes an empty registry, so the hooks
// are no-ops — but the call sites already exist for your custom abilities.
// ---------------------------------------------------------------------------

import { createDeck, shuffle, makeRng } from './cards.js'
import { evaluateHand, compareScore } from './handEvaluator.js'
import { noPowers } from './powers.js'

export const STREETS = ['preflop', 'flop', 'turn', 'river']

let _uid = 0

export class PokerGame {
  constructor({
    players,            // [{ id, name, isHuman, character, avatar, color }]
    smallBlind = 10,
    bigBlind = 20,
    startingChips = 1000,
    powers = noPowers(),
    seed,               // optional: deterministic shuffles for tests
  }) {
    this.powers = powers
    this.rng = seed === undefined ? Math.random : makeRng(seed)

    this.state = {
      players: players.map((p, i) => ({
        seat: i,
        id: p.id,
        name: p.name,
        isHuman: !!p.isHuman,
        character: p.character ?? null,
        avatar: p.avatar ?? '🙂',
        color: p.color ?? '#888',
        chips: startingChips,
        hole: [],
        folded: false,
        allIn: false,
        streetBet: 0,   // committed on the current street
        totalBet: 0,    // committed across the whole hand (for side pots)
        hasActed: false,
        inHand: false,  // dealt into the current hand
        lastAction: null,
        won: 0,
      })),
      smallBlind,
      bigBlind,
      startingChips,
      deck: [],
      community: [],
      street: 'idle',
      buttonIndex: -1,
      currentBet: 0,
      minRaise: bigBlind,
      actingIndex: -1,
      lastAggressor: -1,
      handNumber: 0,
      results: null,    // populated at showdown / hand end
      log: [],
    }
  }

  // ---- small helpers ------------------------------------------------------

  get s() {
    return this.state
  }

  log(msg) {
    this.state.log.push({ id: ++_uid, text: msg })
    if (this.state.log.length > 200) this.state.log.shift()
  }

  pot() {
    return this.state.players.reduce((sum, p) => sum + p.totalBet, 0)
  }

  // Players seated with chips (or already in this hand).
  seatedPlayers() {
    return this.state.players.filter((p) => p.chips > 0 || p.inHand)
  }

  inHandPlayers() {
    return this.state.players.filter((p) => p.inHand && !p.folded)
  }

  // Players who can still make a betting decision (not folded, not all-in).
  actablePlayers() {
    return this.state.players.filter(
      (p) => p.inHand && !p.folded && !p.allIn,
    )
  }

  player(seat) {
    return this.state.players[seat]
  }

  // Next seat (cyclic) after `from` satisfying pred.
  nextSeat(from, pred) {
    const n = this.state.players.length
    for (let step = 1; step <= n; step++) {
      const seat = (from + step) % n
      if (pred(this.state.players[seat])) return seat
    }
    return -1
  }

  // ---- starting a hand ----------------------------------------------------

  startHand() {
    const st = this.state
    // Reset per-hand player fields; bust players sit out.
    for (const p of st.players) {
      p.hole = []
      p.folded = false
      p.allIn = false
      p.streetBet = 0
      p.totalBet = 0
      p.hasActed = false
      p.lastAction = null
      p.won = 0
      p.inHand = p.chips > 0
    }

    const contenders = st.players.filter((p) => p.inHand)
    if (contenders.length < 2) {
      st.street = 'gameover'
      st.results = { gameOver: true, winner: contenders[0] ?? null }
      this.log('Not enough players with chips — game over.')
      return
    }

    st.handNumber += 1
    st.deck = shuffle(createDeck(), this.rng)
    st.community = []
    st.currentBet = 0
    st.minRaise = st.bigBlind
    st.results = null
    st.street = 'preflop'

    // Move the button to the next seated player.
    st.buttonIndex = this.nextSeat(st.buttonIndex, (p) => p.inHand)

    this.powers.emit('onHandStart', { game: this })
    this.log(`— Hand #${st.handNumber} —`)

    // Post blinds. Heads-up: button is the small blind and acts first preflop.
    const headsUp = contenders.length === 2
    let sbSeat, bbSeat
    if (headsUp) {
      sbSeat = st.buttonIndex
      bbSeat = this.nextSeat(st.buttonIndex, (p) => p.inHand)
    } else {
      sbSeat = this.nextSeat(st.buttonIndex, (p) => p.inHand)
      bbSeat = this.nextSeat(sbSeat, (p) => p.inHand)
    }
    this._postBlind(sbSeat, st.smallBlind, 'small blind')
    this._postBlind(bbSeat, st.bigBlind, 'big blind')
    st.currentBet = st.bigBlind
    st.minRaise = st.bigBlind
    st.lastAggressor = bbSeat

    // Deal two hole cards to each contender, starting left of the button.
    for (let round = 0; round < 2; round++) {
      let seat = st.buttonIndex
      for (let i = 0; i < contenders.length; i++) {
        seat = this.nextSeat(seat, (p) => p.inHand)
        st.players[seat].hole.push(st.deck.pop())
      }
    }
    this.powers.emit('onDeal', { game: this })

    // First to act preflop: left of big blind (heads-up: the button/SB).
    st.actingIndex = headsUp
      ? sbSeat
      : this.nextSeat(bbSeat, (p) => p.inHand && !p.allIn && !p.folded)

    // Blinds count as "not yet acted" so they get their option.
    this._beforeActing()
  }

  _postBlind(seat, amount, label) {
    const p = this.state.players[seat]
    const pay = Math.min(amount, p.chips)
    p.chips -= pay
    p.streetBet += pay
    p.totalBet += pay
    if (p.chips === 0) p.allIn = true
    this.log(`${p.name} posts ${label} (${pay}).`)
  }

  // ---- legal actions ------------------------------------------------------

  // What can the player currently to act legally do?
  legalActions(seat = this.state.actingIndex) {
    const st = this.state
    const p = st.players[seat]
    if (!p || p.folded || p.allIn || !p.inHand) return null

    const toCall = st.currentBet - p.streetBet
    const actions = { fold: true }

    if (toCall <= 0) {
      actions.check = true
    } else {
      actions.call = Math.min(toCall, p.chips) // amount to put in to call
    }

    // Raising: you need chips beyond the call. The minimum *total* street bet
    // for a raise is currentBet + minRaise (capped by going all-in).
    const maxTotal = p.streetBet + p.chips // all-in total this street
    if (maxTotal > st.currentBet) {
      const minTotal = Math.min(st.currentBet + st.minRaise, maxTotal)
      actions.raise = {
        minTotal,                 // smallest legal total streetBet for a raise
        maxTotal,                 // shove
        toCall: Math.max(0, toCall),
      }
    }

    return this.powers.filter('beforeAction', actions, { game: this, player: p })
  }

  // ---- applying an action -------------------------------------------------

  // action = { type: 'fold'|'check'|'call'|'raise', total?: number }
  // For 'raise', `total` is the desired total streetBet (chips committed this
  // street). The engine clamps it to the legal range.
  act(action) {
    const st = this.state
    const seat = st.actingIndex
    const p = st.players[seat]
    if (!p || seat < 0) throw new Error('No player to act.')
    const legal = this.legalActions(seat)
    if (!legal) throw new Error('Acting player cannot act.')

    let resolved = { type: action.type, amount: 0, total: p.streetBet }

    switch (action.type) {
      case 'fold': {
        p.folded = true
        resolved = { type: 'fold' }
        this.log(`${p.name} folds.`)
        break
      }
      case 'check': {
        if (!legal.check) throw new Error('Cannot check.')
        resolved = { type: 'check' }
        this.log(`${p.name} checks.`)
        break
      }
      case 'call': {
        const pay = Math.min(st.currentBet - p.streetBet, p.chips)
        this._commit(p, pay)
        resolved = { type: 'call', amount: pay, total: p.streetBet }
        this.log(`${p.name} calls ${pay}${p.allIn ? ' (all-in)' : ''}.`)
        break
      }
      case 'raise': {
        if (!legal.raise) throw new Error('Cannot raise.')
        let total = Math.round(action.total)
        total = Math.max(legal.raise.minTotal, Math.min(total, legal.raise.maxTotal))
        const pay = total - p.streetBet
        const raiseSize = total - st.currentBet
        this._commit(p, pay)
        // A full raise resets the minimum; a short all-in does not.
        if (raiseSize >= st.minRaise) st.minRaise = raiseSize
        st.currentBet = Math.max(st.currentBet, total)
        st.lastAggressor = seat
        // Everyone still in must respond to a raise.
        for (const other of st.players) {
          if (other !== p && other.inHand && !other.folded && !other.allIn) {
            other.hasActed = false
          }
        }
        resolved = { type: 'raise', amount: pay, total, allIn: p.allIn }
        this.log(`${p.name} ${p.allIn ? 'shoves' : 'raises'} to ${total}${p.allIn ? ' (all-in)' : ''}.`)
        break
      }
      default:
        throw new Error(`Unknown action ${action.type}`)
    }

    p.hasActed = true
    p.lastAction = resolved
    this.powers.emit('afterAction', { game: this, player: p, action: resolved })

    this._afterAction()
    return resolved
  }

  _commit(p, amount) {
    const pay = Math.min(amount, p.chips)
    p.chips -= pay
    p.streetBet += pay
    p.totalBet += pay
    if (p.chips === 0) p.allIn = true
  }

  // Does this player still owe an action this street?
  _needsToAct(p) {
    return (
      p.inHand &&
      !p.folded &&
      !p.allIn &&
      (!p.hasActed || p.streetBet < this.state.currentBet)
    )
  }

  _afterAction() {
    const st = this.state

    // Everyone but one folded → that player wins immediately.
    if (this.inHandPlayers().length === 1) {
      this._awardUncontested()
      return
    }

    // Find the next player who still needs to act this street.
    const next = this.nextSeat(st.actingIndex, (p) => this._needsToAct(p))
    if (next !== -1) {
      st.actingIndex = next
      this._beforeActing()
      return
    }

    // Betting round complete.
    this._endBettingRound()
  }

  _beforeActing() {
    // Hook point so a power could, say, peek or inject an extra option.
    const p = this.state.players[this.state.actingIndex]
    if (p) this.powers.emit('beforeAction', { game: this, player: p })
  }

  _endBettingRound() {
    const st = this.state
    // Reset street bets for the next street.
    for (const p of st.players) {
      p.streetBet = 0
      p.hasActed = false
    }
    st.currentBet = 0
    st.minRaise = st.bigBlind

    // If fewer than two players can still act, run the board out to showdown.
    const canAct = this.actablePlayers().length
    if (canAct < 2) {
      this._runOutBoard()
      this._showdown()
      return
    }

    // Otherwise advance to the next street, or showdown after the river.
    const idx = STREETS.indexOf(st.street)
    if (idx >= STREETS.length - 1) {
      this._showdown()
      return
    }
    this._dealStreet(STREETS[idx + 1])

    // First to act post-flop: first active player left of the button.
    st.actingIndex = this.nextSeat(st.buttonIndex, (p) => this._needsToAct(p))
    if (st.actingIndex === -1) {
      // Everyone left is all-in; keep dealing.
      this._runOutBoard()
      this._showdown()
    } else {
      this._beforeActing()
    }
  }

  _dealStreet(street) {
    const st = this.state
    st.deck.pop() // burn
    if (street === 'flop') {
      st.community.push(st.deck.pop(), st.deck.pop(), st.deck.pop())
    } else {
      st.community.push(st.deck.pop())
    }
    st.street = street
    this.powers.emit('onStreet', { game: this, street })
    this.log(`${cap(street)}: ${this._board()}`)
  }

  _board() {
    return this.state.community
      .map((c) => `${c.rank}${c.suit}`)
      .join(' ')
  }

  // Deal whatever community cards remain (used when betting is closed because
  // everyone left is all-in). Reveals flop → turn → river as needed.
  _runOutBoard() {
    while (this.state.community.length < 5) {
      const have = this.state.community.length
      if (have === 0) this._dealStreet('flop')
      else if (have === 3) this._dealStreet('turn')
      else if (have === 4) this._dealStreet('river')
      else break // safety; shouldn't happen
    }
  }

  // ---- resolution ---------------------------------------------------------

  _awardUncontested() {
    const winner = this.inHandPlayers()[0]
    const total = this.pot()
    winner.chips += total
    winner.won = total
    this.state.street = 'handover'
    this.state.actingIndex = -1
    this.state.results = {
      uncontested: true,
      pots: [{ amount: total, winners: [winner.seat] }],
      reveal: false,
    }
    this.powers.emit('onHandEnd', { game: this })
    this.log(`${winner.name} wins ${total} uncontested.`)
  }

  _showdown() {
    const st = this.state
    st.street = 'showdown'
    st.actingIndex = -1
    this.powers.emit('onShowdown', { game: this })

    const contenders = this.inHandPlayers()

    // Evaluate every contender's best hand (powers may rewrite the eval).
    const evals = {}
    for (const p of contenders) {
      let result = evaluateHand([...p.hole, ...st.community])
      result = this.powers.filter('filterEvaluation', result, {
        game: this,
        player: p,
      })
      evals[p.seat] = result
    }

    // Build side pots from per-player total contributions.
    const pots = buildSidePots(st.players)

    const potResults = []
    for (const pot of pots) {
      const eligible = pot.eligible.filter((seat) => evals[seat]) // not folded
      if (eligible.length === 0) continue
      // Highest score wins; ties split.
      let best = null
      let winners = []
      for (const seat of eligible) {
        const sc = evals[seat].score
        if (!best || compareScore(sc, best) > 0) {
          best = sc
          winners = [seat]
        } else if (compareScore(sc, best) === 0) {
          winners.push(seat)
        }
      }
      const share = Math.floor(pot.amount / winners.length)
      let remainder = pot.amount - share * winners.length
      for (const seat of winners) {
        let add = share
        // Odd chip goes to the first winner left of the button.
        if (remainder > 0) {
          add += 1
          remainder -= 1
        }
        st.players[seat].chips += add
        st.players[seat].won += add
      }
      potResults.push({ amount: pot.amount, winners, best })
    }

    st.street = 'handover'
    st.results = {
      reveal: true,
      evals,
      pots: potResults,
    }
    this.powers.emit('onHandEnd', { game: this })

    const names = [...new Set(potResults.flatMap((p) => p.winners))]
      .map((seat) => st.players[seat].name)
      .join(', ')
    this.log(`Showdown — ${names} win${names.includes(',') ? '' : 's'} the pot.`)
  }
}

// Build side pots from players' totalBet. Folded players contribute chips but
// are not eligible to win. Returns [{ amount, eligible:[seat,...] }] ordered
// from main pot outward. (Uncalled excess naturally returns to its sole
// contributor as a one-eligible pot, which is correct.)
export function buildSidePots(players) {
  const contributors = players.filter((p) => p.totalBet > 0)
  const levels = [...new Set(contributors.map((p) => p.totalBet))].sort(
    (a, b) => a - b,
  )
  const pots = []
  let prev = 0
  for (const level of levels) {
    const band = level - prev
    const atLevel = contributors.filter((p) => p.totalBet >= level)
    const amount = band * atLevel.length
    if (amount > 0) {
      pots.push({
        amount,
        // Eligible to WIN = those still in the hand (not folded).
        eligible: atLevel.filter((p) => !p.folded).map((p) => p.seat),
      })
    }
    prev = level
  }
  // Merge adjacent pots with identical eligibility for cleaner display.
  const merged = []
  for (const pot of pots) {
    const key = pot.eligible.join(',')
    const last = merged[merged.length - 1]
    if (last && last.key === key) last.amount += pot.amount
    else merged.push({ ...pot, key })
  }
  return merged
}

function cap(s) {
  return s.charAt(0).toUpperCase() + s.slice(1)
}
