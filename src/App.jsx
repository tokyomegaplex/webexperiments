// ---------------------------------------------------------------------------
// App.jsx — top-level shell: a setup screen, then the table.
//
// The setup screen lets you pick how many cartoon opponents to face (1–5) and
// seats them with your human player at the bottom. Everything below this line
// is presentation — the game itself lives in /engine.
// ---------------------------------------------------------------------------

import { useState } from 'react'
import { useGame } from './ui/useGame.js'
import Table from './ui/Table.jsx'
import Controls from './ui/Controls.jsx'
import { pickOpponents } from './engine/characters.js'

export default function App() {
  const [config, setConfig] = useState(null)

  if (!config) return <Setup onStart={setConfig} />
  return <GameScreen config={config} onQuit={() => setConfig(null)} />
}

function Setup({ onStart }) {
  const [count, setCount] = useState(3)
  const [name, setName] = useState('You')

  function start() {
    const opponents = pickOpponents(count)
    const players = [
      { id: 'human', name: name || 'You', isHuman: true, avatar: '😎', color: '#2c8fd6' },
      ...opponents.map((c) => ({
        id: c.id,
        name: c.name,
        isHuman: false,
        avatar: c.avatar,
        color: c.color,
        character: c,
      })),
    ]
    onStart({ players, smallBlind: 10, bigBlind: 20, startingChips: 1000 })
  }

  return (
    <div className="setup">
      <h1 className="title">Cartoon Hold'em</h1>
      <p className="subtitle">
        A 52-card Texas Hold'em foundation — built to be mutated into your own game.
      </p>

      <label className="field">
        <span>Your name</span>
        <input value={name} onChange={(e) => setName(e.target.value)} maxLength={16} />
      </label>

      <div className="field">
        <span>Opponents: {count}</span>
        <div className="opp-buttons">
          {[1, 2, 3, 4].map((c) => (
            <button
              key={c}
              className={c === count ? 'opp opp--on' : 'opp'}
              onClick={() => setCount(c)}
            >
              {c}
            </button>
          ))}
        </div>
      </div>

      <button className="btn btn--deal" onClick={start}>
        Sit Down & Deal
      </button>

      <p className="hint">
        Smart AI opponents · 1,000 starting chips · blinds 10/20
      </p>
    </div>
  )
}

function GameScreen({ config, onQuit }) {
  const { game, state, legal, perform, nextHand, bubbles, dealerSay } =
    useGame(config)

  const handover = state.street === 'handover'
  const gameover = state.street === 'gameover'
  const pot = state.players.reduce((s, p) => s + p.totalBet, 0)

  return (
    <div className="game">
      <div className="topbar">
        <span className="brand">Cartoon Hold'em</span>
        <span className="hand-no">Hand #{state.handNumber}</span>
        <button className="btn btn--ghost" onClick={onQuit}>
          Leave table
        </button>
      </div>

      <Table
        state={state}
        bubbles={bubbles}
        dealerSay={dealerSay}
        revealAll={handover && state.results?.reveal}
      />

      <div className="bottombar">
        {gameover ? (
          <div className="controls controls--end">
            <strong>
              {state.results?.winner
                ? `${state.results.winner.name} wins it all!`
                : 'Game over.'}
            </strong>
            <button className="btn btn--deal" onClick={onQuit}>
              New game
            </button>
          </div>
        ) : handover ? (
          <div className="controls controls--end">
            <strong>{handResult(state)}</strong>
            <button className="btn btn--deal" onClick={nextHand}>
              Deal next hand
            </button>
          </div>
        ) : (
          <Controls legal={legal} onAction={perform} pot={pot} />
        )}
      </div>

      <GameLog log={state.log} />
    </div>
  )
}

function handResult(state) {
  const res = state.results
  if (!res) return ''
  const winners = [...new Set((res.pots || []).flatMap((p) => p.winners))]
    .map((seat) => state.players[seat])
  if (!winners.length) return 'Hand complete.'
  if (res.uncontested) return `${winners[0].name} wins ${winners[0].won} uncontested.`
  const names = winners.map((w) => `${w.name} (+${w.won})`).join(', ')
  return `Showdown — ${names}`
}

function GameLog({ log }) {
  return (
    <details className="log">
      <summary>Hand log</summary>
      <div className="log-body">
        {log.slice(-30).reverse().map((entry) => (
          <div key={entry.id} className="log-line">{entry.text}</div>
        ))}
      </div>
    </details>
  )
}
