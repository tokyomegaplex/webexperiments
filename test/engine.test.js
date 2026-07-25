import { test } from 'node:test'
import assert from 'node:assert/strict'

import { createDeck } from '../src/engine/cards.js'
import { evaluateHand, compareScore, CATEGORY } from '../src/engine/handEvaluator.js'
import { PokerGame, buildSidePots } from '../src/engine/pokerGame.js'
import { decideAction } from '../src/engine/ai.js'

const C = (rank, suit) => ({ rank, suit })

test('deck has 52 unique cards', () => {
  const deck = createDeck()
  assert.equal(deck.length, 52)
  assert.equal(new Set(deck.map((c) => `${c.rank}${c.suit}`)).size, 52)
})

test('recognizes a royal/straight flush', () => {
  const h = evaluateHand([C(14, 's'), C(13, 's'), C(12, 's'), C(11, 's'), C(10, 's'), C(2, 'h'), C(3, 'd')])
  assert.equal(h.category, CATEGORY.STRAIGHT_FLUSH)
})

test('recognizes the wheel (A-2-3-4-5) straight', () => {
  const h = evaluateHand([C(14, 's'), C(2, 'h'), C(3, 'd'), C(4, 'c'), C(5, 's'), C(9, 'h'), C(13, 'd')])
  assert.equal(h.category, CATEGORY.STRAIGHT)
  assert.equal(h.score[1], 5) // five-high straight
})

test('full house beats flush', () => {
  const fullHouse = evaluateHand([C(8, 's'), C(8, 'h'), C(8, 'd'), C(4, 'c'), C(4, 's'), C(2, 'h'), C(7, 'd')])
  const flush = evaluateHand([C(2, 's'), C(5, 's'), C(7, 's'), C(9, 's'), C(11, 's'), C(3, 'h'), C(4, 'd')])
  assert.equal(fullHouse.category, CATEGORY.FULL_HOUSE)
  assert.equal(flush.category, CATEGORY.FLUSH)
  assert.ok(compareScore(fullHouse.score, flush.score) > 0)
})

test('kicker decides between equal pairs', () => {
  const a = evaluateHand([C(10, 's'), C(10, 'h'), C(14, 'd'), C(5, 'c'), C(2, 's'), C(3, 'h'), C(4, 'd')])
  const b = evaluateHand([C(10, 'c'), C(10, 'd'), C(13, 'd'), C(5, 'h'), C(2, 'c'), C(3, 's'), C(4, 'c')])
  assert.ok(compareScore(a.score, b.score) > 0) // ace kicker beats king kicker
})

test('side pots split correctly with an all-in', () => {
  // A all-in 100, B and C each put 300. A can only win the 300 main pot.
  const players = [
    { seat: 0, totalBet: 100, folded: false },
    { seat: 1, totalBet: 300, folded: false },
    { seat: 2, totalBet: 300, folded: false },
  ]
  const pots = buildSidePots(players)
  const total = pots.reduce((s, p) => s + p.amount, 0)
  assert.equal(total, 700)
  // Main pot 300 eligible to all three; side pot 400 only to B & C.
  assert.equal(pots[0].amount, 300)
  assert.deepEqual(pots[0].eligible.sort(), [0, 1, 2])
  assert.equal(pots[1].amount, 400)
  assert.deepEqual(pots[1].eligible.sort(), [1, 2])
})

test('a full hand plays to completion with AI on both sides', () => {
  const game = new PokerGame({
    players: [
      { id: 'a', name: 'A', isHuman: false },
      { id: 'b', name: 'B', isHuman: false },
      { id: 'c', name: 'C', isHuman: false },
    ],
    seed: 12345,
  })

  for (let hand = 0; hand < 40; hand++) {
    game.startHand()
    if (game.state.street === 'gameover') break
    let guard = 0
    while (game.state.actingIndex !== -1) {
      const action = decideAction(game, game.state.actingIndex)
      game.act(action)
      if (++guard > 500) throw new Error('hand did not terminate')
    }
    // Chips are conserved across the table.
    const total = game.state.players.reduce((s, p) => s + p.chips, 0)
    assert.equal(total, 3000, `chip total drifted on hand ${hand}`)
    assert.ok(game.state.street === 'handover' || game.state.street === 'gameover')
  }
})
