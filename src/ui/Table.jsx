// ---------------------------------------------------------------------------
// Table.jsx — React wrapper that paints engine state to a <canvas> using the
// perspective renderer (tableRender.js). It also preloads any character art
// referenced by `image` and redraws once the images load.
// ---------------------------------------------------------------------------

import { useEffect, useRef, useState } from 'react'
import { draw, W, H } from './tableRender.js'

// Resolve an image path (relative to /public) against Vite's base URL.
const assetUrl = (p) => `${import.meta.env.BASE_URL}${p}`

export default function Table({ state, bubbles, dealerSay, revealAll }) {
  const canvasRef = useRef(null)
  const [images, setImages] = useState({})

  // Preload character images once.
  useEffect(() => {
    const urls = new Set()
    for (const p of state.players) {
      if (p.character?.image) urls.add(p.character.image)
    }
    urls.forEach((rel) => {
      if (images[rel]) return
      const img = new Image()
      img.onload = () => setImages((prev) => ({ ...prev, [rel]: img }))
      img.onerror = () => {} // fall back to the drawn stand-in
      img.src = assetUrl(rel)
    })
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [state.players])

  useEffect(() => {
    const ctx = canvasRef.current.getContext('2d')
    draw(ctx, state, { bubbles, dealerSay, revealAll, images })
  })

  return <canvas ref={canvasRef} width={W} height={H} className="table-canvas" />
}
