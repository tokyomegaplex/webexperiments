// ---------------------------------------------------------------------------
// useGame.js — React glue around the framework-free PokerGame.
//
// The engine mutates its own `state` object in place, so we keep one instance
// in a ref and force re-renders with a version counter. This hook also:
//   - drives AI turns on a timer (so you can watch the table think/talk),
//   - records speech-bubble barks from each character's `lines`,
//   - exposes a tiny action API for the human's controls.
// ---------------------------------------------------------------------------

import { useCallback, useEffect, useRef, useState } from 'react'
import { PokerGame } from '../engine/pokerGame.js'
import { decideAction } from '../engine/ai.js'
import { DEALER, randomLine } from '../engine/characters.js'

const AI_DELAY = 850 // ms between AI actions, for watchability

export function useGame(config) {
  const gameRef = useRef(null)
  if (gameRef.current === null) {
    gameRef.current = new PokerGame(config)
    gameRef.current.startHand()
  }
  const game = gameRef.current

  const [, setVersion] = useState(0)
  const bump = useCallback(() => setVersion((v) => v + 1), [])

  // seat -> { text, ts }  (the most recent bark to float over a player)
  const [bubbles, setBubbles] = useState({})
  const [dealerSay, setDealerSay] = useState(
    randomLine(DEALER.lines, 'handStart'),
  )

  const sayFor = useCallback((seat, key, ...args) => {
    const p = game.state.players[seat]
    const text = randomLine(p.character?.lines, key, ...args)
    if (text) setBubbles((b) => ({ ...b, [seat]: { text, ts: Date.now() } }))
  }, [game])

  // Apply an action (human or AI), emit a bark, then re-render.
  const perform = useCallback(
    (action) => {
      const seat = game.state.actingIndex
      if (seat < 0) return
      const resolved = game.act(action)
      const key =
        resolved.type === 'raise'
          ? 'raise'
          : resolved.type === 'fold'
          ? 'fold'
          : 'call'
      sayFor(seat, key)
      bump()
    },
    [game, sayFor, bump],
  )

  // Drive AI turns. Re-runs whenever the acting player changes.
  const acting = game.state.actingIndex
  const street = game.state.street
  useEffect(() => {
    if (acting < 0) return
    const p = game.state.players[acting]
    if (!p || p.isHuman) return
    const id = setTimeout(() => {
      // Re-check: state may have moved on (StrictMode / rapid renders).
      if (game.state.actingIndex !== acting) return
      const action = decideAction(game, acting)
      perform(action)
    }, AI_DELAY)
    return () => clearTimeout(id)
  }, [acting, street, game, perform])

  // Dealer narration as streets change.
  const community = game.state.community.length
  useEffect(() => {
    if (street === 'flop') setDealerSay(randomLine(DEALER.lines, 'flop'))
    else if (street === 'turn') setDealerSay(randomLine(DEALER.lines, 'turn'))
    else if (street === 'river') setDealerSay(randomLine(DEALER.lines, 'river'))
    else if (street === 'showdown') setDealerSay(randomLine(DEALER.lines, 'showdown'))
  }, [street, community])

  // Announce winners when a hand ends, and let characters react.
  useEffect(() => {
    const res = game.state.results
    if (street !== 'handover' || !res) return
    const winners = (res.pots || []).flatMap((p) => p.winners)
    const uniq = [...new Set(winners)]
    for (const seat of uniq) {
      const p = game.state.players[seat]
      sayFor(seat, p.won > game.state.bigBlind * 8 ? 'bigWin' : 'win')
    }
    if (res.uncontested) setDealerSay(randomLine(DEALER.lines, 'fold'))
    else if (uniq.length) {
      setDealerSay(randomLine(DEALER.lines, 'win', game.state.players[uniq[0]].name))
    }
  }, [street, game, sayFor])

  const nextHand = useCallback(() => {
    setBubbles({})
    game.startHand()
    setDealerSay(randomLine(DEALER.lines, 'handStart'))
    bump()
  }, [game, bump])

  const legal =
    acting >= 0 && game.state.players[acting]?.isHuman
      ? game.legalActions(acting)
      : null

  return { game, state: game.state, legal, perform, nextHand, bubbles, dealerSay }
}
