// ---------------------------------------------------------------------------
// tableRender.js — the pure canvas drawing for the poker table.
//
// Framework-free on purpose: it only touches a 2D canvas context, so it can be
// driven by React (Table.jsx) in the browser OR by node-canvas in a script for
// offline snapshots/tests. No game logic lives here.
//
// Bring your own character art by editing drawAvatar() — replace the emoji
// circle with an image/sprite.
// ---------------------------------------------------------------------------

import { RANK_LABELS, SUIT_SYMBOLS, SUIT_COLORS } from '../engine/cards.js'

export const W = 960
export const H = 740

export function draw(ctx, state, bubbles = {}, dealerSay = '', revealAll = false) {
  ctx.clearRect(0, 0, W, H)

  // Backdrop
  const bg = ctx.createRadialGradient(W / 2, H / 2, 80, W / 2, H / 2, 620)
  bg.addColorStop(0, '#15241b')
  bg.addColorStop(1, '#0a130d')
  ctx.fillStyle = bg
  ctx.fillRect(0, 0, W, H)

  const cx = W / 2
  const cy = H / 2 - 6
  const rx = 335
  const ry = 168

  // Felt
  drawEllipse(ctx, cx, cy, rx, ry)
  const felt = ctx.createRadialGradient(cx, cy, 40, cx, cy, rx)
  felt.addColorStop(0, '#1f7a4d')
  felt.addColorStop(1, '#0f5234')
  ctx.fillStyle = felt
  ctx.fill()
  ctx.lineWidth = 14
  ctx.strokeStyle = '#3a2417'
  ctx.stroke()
  ctx.lineWidth = 3
  ctx.strokeStyle = 'rgba(255,255,255,0.12)'
  drawEllipse(ctx, cx, cy, rx - 22, ry - 22)
  ctx.stroke()

  const players = state.players
  const n = players.length
  const humanIndex = Math.max(0, players.findIndex((p) => p.isHuman))

  // Pot
  const pot = players.reduce((s, p) => s + p.totalBet, 0)
  ctx.fillStyle = 'rgba(0,0,0,0.35)'
  roundRect(ctx, cx - 70, cy - ry + 26, 140, 34, 17)
  ctx.fill()
  ctx.fillStyle = '#ffd86b'
  ctx.font = 'bold 20px system-ui, sans-serif'
  ctx.textAlign = 'center'
  ctx.textBaseline = 'middle'
  ctx.fillText(`POT ${pot}`, cx, cy - ry + 43)

  // Community cards
  const cardW = 62
  const cardH = 88
  const gap = 10
  const total = 5 * cardW + 4 * gap
  let bx = cx - total / 2
  for (let i = 0; i < 5; i++) {
    const card = state.community[i]
    const y = cy - cardH / 2 + 4
    if (card) drawCard(ctx, bx, y, cardW, cardH, card)
    else drawCardSlot(ctx, bx, y, cardW, cardH)
    bx += cardW + gap
  }

  // Dealer narration bubble (top-left host area)
  if (dealerSay) {
    drawDealer(ctx, 18, 18, dealerSay)
  }

  // Seats around the ellipse — human pinned to the bottom.
  for (let seat = 0; seat < n; seat++) {
    const offset = seat - humanIndex
    const angle = Math.PI / 2 + (offset / n) * Math.PI * 2
    const sx = cx + Math.cos(angle) * (rx + 58)
    // Clamp vertically so a seat's cards (above) and chip plate (below) always
    // stay on the canvas, even for the top/bottom seats.
    const rawSy = cy + Math.sin(angle) * (ry + 70)
    const sy = Math.max(104, Math.min(rawSy, H - 96))
    drawSeat(ctx, players[seat], sx, sy, {
      isActive: state.actingIndex === seat,
      isButton: state.buttonIndex === seat,
      reveal: revealAll || (state.results?.reveal && !players[seat].folded),
      bubble: bubbles[seat],
      results: state.results,
    })
  }
}

function drawSeat(ctx, p, x, y, opts) {
  const { isActive, isButton, reveal, bubble, results } = opts

  // Hole cards (above the avatar)
  const cw = 44
  const ch = 60
  const cardsY = y - 84
  if (p.inHand && p.hole.length) {
    const showFace = p.isHuman || reveal
    const x0 = x - cw - 3
    if (p.folded) ctx.globalAlpha = 0.32
    if (showFace) {
      drawCard(ctx, x0, cardsY, cw, ch, p.hole[0])
      drawCard(ctx, x0 + cw + 6, cardsY, cw, ch, p.hole[1])
    } else {
      drawCardBack(ctx, x0, cardsY, cw, ch)
      drawCardBack(ctx, x0 + cw + 6, cardsY, cw, ch)
    }
    ctx.globalAlpha = 1
  }

  // Active glow
  if (isActive) {
    ctx.save()
    ctx.shadowColor = '#ffd86b'
    ctx.shadowBlur = 28
    ctx.beginPath()
    ctx.arc(x, y, 38, 0, Math.PI * 2)
    ctx.strokeStyle = '#ffd86b'
    ctx.lineWidth = 4
    ctx.stroke()
    ctx.restore()
  }

  drawAvatar(ctx, p, x, y)

  // Name plate + chips
  const plateW = 150
  ctx.fillStyle = p.folded ? 'rgba(20,20,20,0.6)' : 'rgba(0,0,0,0.55)'
  roundRect(ctx, x - plateW / 2, y + 40, plateW, 42, 10)
  ctx.fill()
  ctx.textAlign = 'center'
  ctx.textBaseline = 'middle'
  ctx.fillStyle = p.folded ? '#888' : '#fff'
  ctx.font = 'bold 15px system-ui, sans-serif'
  ctx.fillText(trunc(p.name, 16), x, y + 53)
  ctx.fillStyle = p.chips > 0 ? '#7ee0a8' : '#e06f6f'
  ctx.font = '13px system-ui, sans-serif'
  ctx.fillText(`${p.chips} chips`, x, y + 70)

  // Current street bet (chips in front)
  if (p.streetBet > 0) {
    const by = y - 30
    ctx.fillStyle = '#caa45a'
    ctx.beginPath()
    ctx.arc(x + 44, by, 11, 0, Math.PI * 2)
    ctx.fill()
    ctx.fillStyle = '#1a1a1a'
    ctx.font = 'bold 12px system-ui, sans-serif'
    ctx.fillText(String(p.streetBet), x + 78, by)
  }

  // Dealer button
  if (isButton) {
    ctx.fillStyle = '#fff'
    ctx.beginPath()
    ctx.arc(x - 46, y + 18, 13, 0, Math.PI * 2)
    ctx.fill()
    ctx.fillStyle = '#1a1a1a'
    ctx.font = 'bold 13px system-ui, sans-serif'
    ctx.fillText('D', x - 46, y + 18)
  }

  // Win badge at showdown
  if (results && p.won > 0) {
    ctx.fillStyle = '#ffd86b'
    roundRect(ctx, x - 46, y - 12, 92, 26, 13)
    ctx.fill()
    ctx.fillStyle = '#1a1a1a'
    ctx.font = 'bold 14px system-ui, sans-serif'
    ctx.fillText(`+${p.won}`, x, y + 1)
  }

  // Last-action tag
  if (p.lastAction && p.inHand && !results) {
    const label = actionLabel(p.lastAction)
    if (label) {
      ctx.fillStyle = 'rgba(255,255,255,0.85)'
      ctx.font = '12px system-ui, sans-serif'
      ctx.fillText(label, x, y + 92)
    }
  }

  // Speech bubble (fades after a moment via fresh ts). Clamp it onto canvas
  // so bubbles over the top seats don't get cut off.
  if (bubble && Date.now() - bubble.ts < 4200) {
    drawBubble(ctx, x, Math.max(y - 118, 46), bubble.text)
  }
}

function drawAvatar(ctx, p, x, y) {
  ctx.beginPath()
  ctx.arc(x, y, 32, 0, Math.PI * 2)
  ctx.fillStyle = p.color || '#666'
  ctx.fill()
  ctx.lineWidth = 3
  ctx.strokeStyle = p.folded ? 'rgba(255,255,255,0.25)' : 'rgba(255,255,255,0.7)'
  ctx.stroke()
  // Emoji placeholder — swap for sprite art later.
  ctx.font = '34px serif'
  ctx.textAlign = 'center'
  ctx.textBaseline = 'middle'
  ctx.globalAlpha = p.folded ? 0.4 : 1
  ctx.fillText(p.avatar || '🙂', x, y + 2)
  ctx.globalAlpha = 1
}

function drawCard(ctx, x, y, w, h, card) {
  roundRect(ctx, x, y, w, h, 6)
  ctx.fillStyle = '#fdfdf6'
  ctx.fill()
  ctx.lineWidth = 1
  ctx.strokeStyle = 'rgba(0,0,0,0.25)'
  ctx.stroke()

  const color = SUIT_COLORS[card.suit]
  ctx.fillStyle = color
  ctx.textAlign = 'left'
  ctx.textBaseline = 'top'
  ctx.font = `bold ${Math.round(h * 0.26)}px system-ui, sans-serif`
  ctx.fillText(RANK_LABELS[card.rank], x + 5, y + 4)
  ctx.font = `${Math.round(h * 0.22)}px serif`
  ctx.fillText(SUIT_SYMBOLS[card.suit], x + 5, y + 4 + h * 0.26)

  ctx.textAlign = 'center'
  ctx.textBaseline = 'middle'
  ctx.font = `${Math.round(h * 0.42)}px serif`
  ctx.fillText(SUIT_SYMBOLS[card.suit], x + w / 2, y + h / 2 + 4)
}

function drawCardBack(ctx, x, y, w, h) {
  roundRect(ctx, x, y, w, h, 6)
  const g = ctx.createLinearGradient(x, y, x + w, y + h)
  g.addColorStop(0, '#7b2d3a')
  g.addColorStop(1, '#a23a4c')
  ctx.fillStyle = g
  ctx.fill()
  ctx.strokeStyle = 'rgba(255,255,255,0.6)'
  ctx.lineWidth = 2
  roundRect(ctx, x + 4, y + 4, w - 8, h - 8, 4)
  ctx.stroke()
}

function drawCardSlot(ctx, x, y, w, h) {
  roundRect(ctx, x, y, w, h, 6)
  ctx.fillStyle = 'rgba(255,255,255,0.05)'
  ctx.fill()
  ctx.strokeStyle = 'rgba(255,255,255,0.12)'
  ctx.lineWidth = 1.5
  ctx.stroke()
}

function drawBubble(ctx, x, y, text) {
  ctx.font = '14px system-ui, sans-serif'
  const padding = 12
  const tw = Math.min(ctx.measureText(text).width, 220)
  const bw = tw + padding * 2
  const bh = 34
  ctx.fillStyle = '#fffef5'
  roundRect(ctx, x - bw / 2, y - bh, bw, bh, 12)
  ctx.fill()
  // little tail
  ctx.beginPath()
  ctx.moveTo(x - 6, y)
  ctx.lineTo(x + 6, y)
  ctx.lineTo(x, y + 10)
  ctx.closePath()
  ctx.fill()
  ctx.fillStyle = '#222'
  ctx.textAlign = 'center'
  ctx.textBaseline = 'middle'
  ctx.fillText(trunc(text, 34), x, y - bh / 2)
}

function drawDealer(ctx, x, y, text) {
  ctx.font = '26px serif'
  ctx.textAlign = 'left'
  ctx.textBaseline = 'top'
  ctx.fillText('🎴', x, y + 4)
  ctx.fillStyle = '#fffef5'
  const tw = Math.min(ctx.measureText(text).width + 24, 320)
  roundRect(ctx, x + 40, y, tw, 36, 10)
  ctx.fill()
  ctx.fillStyle = '#222'
  ctx.font = '14px system-ui, sans-serif'
  ctx.textBaseline = 'middle'
  ctx.fillText(trunc(text, 40), x + 52, y + 18)
}

// ---- canvas primitives ----------------------------------------------------

function drawEllipse(ctx, cx, cy, rx, ry) {
  ctx.beginPath()
  ctx.ellipse(cx, cy, rx, ry, 0, 0, Math.PI * 2)
}

function roundRect(ctx, x, y, w, h, r) {
  ctx.beginPath()
  ctx.moveTo(x + r, y)
  ctx.arcTo(x + w, y, x + w, y + h, r)
  ctx.arcTo(x + w, y + h, x, y + h, r)
  ctx.arcTo(x, y + h, x, y, r)
  ctx.arcTo(x, y, x + w, y, r)
  ctx.closePath()
}

function actionLabel(a) {
  switch (a.type) {
    case 'fold': return 'Fold'
    case 'check': return 'Check'
    case 'call': return a.amount ? `Call ${a.amount}` : 'Call'
    case 'raise': return a.allIn ? `All-in ${a.total}` : `Raise ${a.total}`
    default: return ''
  }
}

function trunc(s, max) {
  return s.length > max ? s.slice(0, max - 1) + '…' : s
}
