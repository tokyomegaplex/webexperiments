// ---------------------------------------------------------------------------
// Table.jsx — thin React wrapper that paints the engine state to a <canvas>.
// All the actual drawing lives in tableRender.js (framework-free) so it can be
// reused/snapshotted outside the browser.
// ---------------------------------------------------------------------------

import { useEffect, useRef } from 'react'
import { draw, W, H } from './tableRender.js'

export default function Table({ state, bubbles, dealerSay, revealAll }) {
  const canvasRef = useRef(null)

  useEffect(() => {
    const ctx = canvasRef.current.getContext('2d')
    draw(ctx, state, bubbles, dealerSay, revealAll)
  })

  return <canvas ref={canvasRef} width={W} height={H} className="table-canvas" />
}
