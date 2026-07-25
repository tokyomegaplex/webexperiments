// ---------------------------------------------------------------------------
// Controls.jsx — the human's action bar: Fold / Check / Call / Raise.
// Shown only when it's the human's turn; `legal` comes straight from the
// engine so the buttons can never offer an illegal move.
// ---------------------------------------------------------------------------

import { useEffect, useState } from 'react'

export default function Controls({ legal, onAction, pot }) {
  const raise = legal?.raise
  const [amount, setAmount] = useState(raise?.minTotal ?? 0)

  // Reset the slider whenever a fresh decision opens up.
  useEffect(() => {
    if (raise) setAmount(raise.minTotal)
  }, [raise?.minTotal, raise?.maxTotal])

  if (!legal) {
    return <div className="controls controls--waiting">Waiting for the table…</div>
  }

  const callLabel =
    legal.call != null ? `Call ${legal.call}` : legal.check ? 'Check' : null

  return (
    <div className="controls">
      <button className="btn btn--fold" onClick={() => onAction({ type: 'fold' })}>
        Fold
      </button>

      {legal.check && (
        <button className="btn btn--check" onClick={() => onAction({ type: 'check' })}>
          Check
        </button>
      )}
      {legal.call != null && (
        <button className="btn btn--call" onClick={() => onAction({ type: 'call' })}>
          {callLabel}
        </button>
      )}

      {raise && (
        <div className="raise-group">
          <div className="raise-row">
            <input
              type="range"
              min={raise.minTotal}
              max={raise.maxTotal}
              value={amount}
              step={1}
              onChange={(e) => setAmount(Number(e.target.value))}
            />
            <span className="raise-amount">{amount}</span>
          </div>
          <div className="raise-presets">
            <button onClick={() => setAmount(clamp(Math.round(potBet(pot, 0.5)), raise))}>½ pot</button>
            <button onClick={() => setAmount(clamp(Math.round(potBet(pot, 1)), raise))}>Pot</button>
            <button onClick={() => setAmount(raise.maxTotal)}>All-in</button>
          </div>
          <button
            className="btn btn--raise"
            onClick={() => onAction({ type: 'raise', total: amount })}
          >
            {amount >= raise.maxTotal ? 'Shove All-in' : `Raise to ${amount}`}
          </button>
        </div>
      )}
    </div>
  )
}

function potBet(pot, frac) {
  return pot * frac
}
function clamp(v, raise) {
  return Math.max(raise.minTotal, Math.min(v, raise.maxTotal))
}
