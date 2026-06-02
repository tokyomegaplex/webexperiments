// ---------------------------------------------------------------------------
// tableRender.js — 2.5D PERSPECTIVE canvas renderer.
//
// Instead of a flat top-down felt, this draws the table as a tilted oval seen
// from a low camera angle, with players as upright "standees" (billboards)
// seated around the rim. Standees in back are smaller (depth), the table
// occludes their lower bodies so they look seated, and the human sits at the
// front as cards + a name plate (no body blocking the view).
//
// Framework-free: only touches a 2D canvas context, so it runs under React
// (Table.jsx) or node-canvas (scripts/snapshot.mjs).
//
// draw(ctx, state, { bubbles, dealerSay, revealAll, images })
//   images: { [url]: HTMLImageElement }  — preloaded character art (optional).
//
// Bring your own art by setting `image` on a character (characters.js); it gets
// drawn here. drawHoodedFallback()/drawAvatarStandee() are the stand-ins.
// ---------------------------------------------------------------------------

import { RANK_LABELS, SUIT_SYMBOLS, SUIT_COLORS } from '../engine/cards.js'

export const W = 1000
export const H = 760

// Table geometry (the felt oval, in perspective).
const T = { cx: 500, cy: 392, rx: 372, ry: 150, rail: 30 }

export function draw(ctx, state, opts = {}) {
  const { bubbles = {}, dealerSay = '', revealAll = false, images = {} } = opts

  // Backdrop
  const bg = ctx.createLinearGradient(0, 0, 0, H)
  bg.addColorStop(0, '#16241b')
  bg.addColorStop(1, '#070d09')
  ctx.fillStyle = bg
  ctx.fillRect(0, 0, W, H)

  const players = state.players
  const n = players.length
  const humanIndex = Math.max(0, players.findIndex((p) => p.isHuman))

  // Lay out every seat in perspective.
  const seats = players.map((p, i) => layoutSeat(p, i, n, humanIndex))
  for (const s of seats) s.active = state.actingIndex === s.seat

  // Draw order for depth: back-row standee BODIES first (so the table can
  // occlude their lower halves), then the table, then front standees.
  const opponents = seats.filter((s) => !s.p.isHuman)
  const backRow = opponents.filter((s) => s.back).sort((a, b) => a.depth - b.depth)
  const frontRow = opponents.filter((s) => !s.back).sort((a, b) => a.depth - b.depth)

  for (const s of backRow) drawStandee(ctx, s, images)
  drawTable(ctx)
  drawPotAndBoard(ctx, state)
  for (const s of frontRow) drawStandee(ctx, s, images)

  // Top layer: each seat's cards, info plate, chips, badges, bubbles.
  for (const s of seats) drawSeatOverlay(ctx, s, state, { revealAll, bubble: bubbles[s.seat] })

  if (dealerSay) drawDealer(ctx, 20, 18, dealerSay)
}

// ---- layout ----------------------------------------------------------------

function layoutSeat(p, seat, n, humanIndex) {
  // Human pinned to the front (bottom); others spread around the oval.
  const angle = Math.PI / 2 + ((seat - humanIndex) / n) * Math.PI * 2
  const sinA = Math.sin(angle)
  const depth = (sinA + 1) / 2 // 0 = far/back (top), 1 = near/front (bottom)
  const scale = 0.66 + depth * 0.5

  // Anchor point on (just outside) the rim.
  const ax = T.cx + Math.cos(angle) * (T.rx + 16)
  const ay = T.cy + sinA * (T.ry + 10)

  return {
    p,
    seat,
    angle,
    depth,
    scale,
    ax,
    ay,
    back: sinA < -0.12, // clearly in the back half → table occludes legs
    isHuman: p.isHuman,
  }
}

// ---- the table -------------------------------------------------------------

function drawTable(ctx) {
  const { cx, cy, rx, ry, rail } = T

  // Soft shadow under the table.
  ctx.save()
  ctx.fillStyle = 'rgba(0,0,0,0.45)'
  ellipse(ctx, cx, cy + rail + 12, rx + 18, ry + 14)
  ctx.fill()
  ctx.restore()

  // Wooden rail (front thickness): a second oval offset downward.
  const railGrad = ctx.createLinearGradient(0, cy, 0, cy + rail + 8)
  railGrad.addColorStop(0, '#5a3318')
  railGrad.addColorStop(1, '#2e1a0c')
  ctx.fillStyle = railGrad
  ellipse(ctx, cx, cy + rail, rx, ry)
  ctx.fill()

  // Felt top.
  const felt = ctx.createRadialGradient(cx, cy - 30, 40, cx, cy, rx)
  felt.addColorStop(0, '#2a8f5c')
  felt.addColorStop(1, '#0f5234')
  ctx.fillStyle = felt
  ellipse(ctx, cx, cy, rx, ry)
  ctx.fill()

  // Rail highlight ring on the felt edge.
  ctx.lineWidth = 8
  ctx.strokeStyle = '#3a2417'
  ellipse(ctx, cx, cy, rx, ry)
  ctx.stroke()
  ctx.lineWidth = 2
  ctx.strokeStyle = 'rgba(255,255,255,0.12)'
  ellipse(ctx, cx, cy, rx - 16, ry - 14)
  ctx.stroke()
}

function drawPotAndBoard(ctx, state) {
  const { cx, cy, ry } = T
  const pot = state.players.reduce((s, p) => s + p.totalBet, 0)

  // Pot pill (upper felt).
  ctx.fillStyle = 'rgba(0,0,0,0.4)'
  roundRect(ctx, cx - 66, cy - ry + 18, 132, 30, 15)
  ctx.fill()
  ctx.fillStyle = '#ffd86b'
  ctx.font = 'bold 18px system-ui, sans-serif'
  center(ctx)
  ctx.fillText(`POT ${pot}`, cx, cy - ry + 33)

  // Community cards laid on the felt (slight vertical squash for perspective).
  const cw = 58
  const ch = 82
  const gap = 9
  const totalW = 5 * cw + 4 * gap
  let bx = cx - totalW / 2
  const y = cy - ch * 0.42
  ctx.save()
  ctx.translate(0, y)
  ctx.scale(1, 0.9)
  for (let i = 0; i < 5; i++) {
    const card = state.community[i]
    if (card) drawCard(ctx, bx, 0, cw, ch, card)
    else drawCardSlot(ctx, bx, 0, cw, ch)
    bx += cw + gap
  }
  ctx.restore()
}

// ---- standees (character bodies) -------------------------------------------

function drawStandee(ctx, s, images) {
  const { p, ax, ay, scale } = s
  const sw = 168 * scale
  const sh = 188 * scale
  const bottom = ay + 30 * scale // dip slightly into the table so it occludes
  const img = p.character?.image ? images[p.character.image] : null

  ctx.save()
  if (p.folded) ctx.globalAlpha = 0.4

  // Soft ground shadow.
  ctx.fillStyle = 'rgba(0,0,0,0.28)'
  ellipse(ctx, ax, bottom - 4, sw * 0.32, 10 * scale)
  ctx.fill()

  if (img && img.complete && img.naturalWidth) {
    const ar = img.naturalWidth / img.naturalHeight
    let dw = sw
    let dh = sw / ar
    if (dh > sh) { dh = sh; dw = sh * ar }
    ctx.drawImage(img, ax - dw / 2, bottom - dh, dw, dh)
  } else if (p.character?.image) {
    drawHoodedFallback(ctx, ax, bottom, sw, sh, p.color)
  } else {
    drawAvatarStandee(ctx, p, ax, bottom, sw, sh)
  }
  ctx.restore()

  // Active highlight: a glowing ring on the floor under the active player.
  if (s.active) {
    ctx.save()
    ctx.strokeStyle = '#ffd86b'
    ctx.lineWidth = 4
    ctx.shadowColor = '#ffd86b'
    ctx.shadowBlur = 22
    ellipse(ctx, ax, bottom - 4, sw * 0.34, 12 * scale)
    ctx.stroke()
    ctx.restore()
  }
}

// Hand-drawn hooded figure — stand-in for hoodguy.png until the file is added.
function drawHoodedFallback(ctx, cxs, bottom, w, h, color) {
  const top = bottom - h
  const midX = cxs
  const cloakW = w * 0.7
  // Cloak/robe body.
  ctx.fillStyle = color || '#7a1f1f'
  ctx.beginPath()
  ctx.moveTo(midX - cloakW * 0.22, top + h * 0.18)
  ctx.quadraticCurveTo(midX, top - h * 0.02, midX + cloakW * 0.22, top + h * 0.18)
  ctx.quadraticCurveTo(midX + cloakW * 0.6, top + h * 0.6, midX + cloakW * 0.5, bottom)
  ctx.quadraticCurveTo(midX, bottom + h * 0.04, midX - cloakW * 0.5, bottom)
  ctx.quadraticCurveTo(midX - cloakW * 0.6, top + h * 0.6, midX - cloakW * 0.22, top + h * 0.18)
  ctx.closePath()
  ctx.fill()
  // Darker hood opening.
  ctx.fillStyle = '#170707'
  ctx.beginPath()
  ctx.ellipse(midX, top + h * 0.2, cloakW * 0.18, h * 0.16, 0, 0, Math.PI * 2)
  ctx.fill()
  // Two big eyes.
  const eyeR = w * 0.05
  for (const ex of [-0.5, 0.5]) {
    ctx.fillStyle = '#f4f4f4'
    ctx.beginPath()
    ctx.arc(midX + ex * cloakW * 0.12, top + h * 0.2, eyeR, 0, Math.PI * 2)
    ctx.fill()
    ctx.fillStyle = '#111'
    ctx.beginPath()
    ctx.arc(midX + ex * cloakW * 0.12, top + h * 0.21, eyeR * 0.45, 0, Math.PI * 2)
    ctx.fill()
  }
}

// Generic standee for emoji characters: a colored robe + an emoji head.
function drawAvatarStandee(ctx, p, cxs, bottom, w, h) {
  const top = bottom - h
  const headR = w * 0.24
  const headY = top + headR + 4
  // Body
  ctx.fillStyle = p.color || '#666'
  ctx.beginPath()
  ctx.moveTo(cxs - w * 0.16, headY)
  ctx.quadraticCurveTo(cxs - w * 0.42, bottom, cxs - w * 0.34, bottom)
  ctx.lineTo(cxs + w * 0.34, bottom)
  ctx.quadraticCurveTo(cxs + w * 0.42, bottom, cxs + w * 0.16, headY)
  ctx.closePath()
  ctx.fill()
  // Head
  ctx.beginPath()
  ctx.arc(cxs, headY, headR, 0, Math.PI * 2)
  ctx.fillStyle = '#f7e8d0'
  ctx.fill()
  ctx.lineWidth = 3
  ctx.strokeStyle = 'rgba(0,0,0,0.25)'
  ctx.stroke()
  // Emoji face
  ctx.font = `${Math.round(headR * 1.5)}px serif`
  center(ctx)
  ctx.fillText(p.avatar || '🙂', cxs, headY + 1)
}

// ---- per-seat overlay (cards, plate, chips, badges, bubble) ----------------

function drawSeatOverlay(ctx, s, state, { revealAll, bubble }) {
  const { p, ax, ay, scale, isHuman } = s
  const reveal = revealAll || (state.results?.reveal && !p.folded)

  // Where the info plate / cards anchor for this seat.
  let plateX = ax
  let plateY
  let cardScale = scale

  if (isHuman) {
    // Human: big cards + plate across the bottom, fully on screen.
    plateY = H - 64
    cardScale = 1.15
    if (p.inHand && p.hole.length) {
      drawHand(ctx, ax, H - 168, p.hole, true, cardScale, p.folded)
    }
  } else if (s.back) {
    // Back row: float the plate ABOVE the head so it clears the felt and pot.
    plateY = clamp(ay - 160 * scale, 50, H - 96)
    if (p.inHand && p.hole.length) {
      drawHand(ctx, ax, ay - 60 * scale, p.hole, reveal, scale * 0.78, p.folded)
    }
  } else {
    // Side/front row: plate below the head, cards on the chest.
    plateY = clamp(ay + 28 * scale, 64, H - 96)
    if (p.inHand && p.hole.length) {
      drawHand(ctx, ax, ay - 74 * scale, p.hole, reveal, scale * 0.8, p.folded)
    }
  }
  plateX = clamp(plateX, 86, W - 86)

  drawPlate(ctx, p, plateX, plateY, state.actingIndex === s.seat)

  // Bet chips between the player and the pot.
  if (p.streetBet > 0) {
    const bx = ax + (T.cx - ax) * 0.26
    const by = ay + (T.cy - ay) * 0.26
    drawChip(ctx, bx, by, p.streetBet)
  }

  // Dealer button.
  if (state.buttonIndex === s.seat) {
    const dx = ax + (T.cx - ax) * 0.14
    const dy = ay + (T.cy - ay) * 0.14 + (isHuman ? -8 : 14)
    ctx.fillStyle = '#fff'
    ctx.beginPath()
    ctx.arc(dx, dy, 12, 0, Math.PI * 2)
    ctx.fill()
    ctx.fillStyle = '#1a1a1a'
    ctx.font = 'bold 12px system-ui, sans-serif'
    center(ctx)
    ctx.fillText('D', dx, dy)
  }

  // Win badge.
  if (state.results && p.won > 0) {
    ctx.fillStyle = '#ffd86b'
    roundRect(ctx, plateX - 44, plateY - 40, 88, 24, 12)
    ctx.fill()
    ctx.fillStyle = '#1a1a1a'
    ctx.font = 'bold 14px system-ui, sans-serif'
    center(ctx)
    ctx.fillText(`+${p.won}`, plateX, plateY - 28)
  }

  // Speech bubble (clamped on-screen).
  if (bubble && Date.now() - bubble.ts < 4200) {
    const by = clamp(isHuman ? plateY - 150 : ay - 150 * scale, 44, H - 60)
    drawBubble(ctx, plateX, by, bubble.text)
  }
}

function drawPlate(ctx, p, x, y, active) {
  const w = 156
  ctx.fillStyle = active ? 'rgba(201,150,47,0.92)' : p.folded ? 'rgba(20,20,20,0.6)' : 'rgba(0,0,0,0.62)'
  roundRect(ctx, x - w / 2, y - 16, w, 40, 10)
  ctx.fill()
  if (active) {
    ctx.lineWidth = 2
    ctx.strokeStyle = '#ffd86b'
    roundRect(ctx, x - w / 2, y - 16, w, 40, 10)
    ctx.stroke()
  }
  center(ctx)
  ctx.fillStyle = p.folded ? '#999' : active ? '#1a1a1a' : '#fff'
  ctx.font = 'bold 14px system-ui, sans-serif'
  ctx.fillText(trunc(p.name, 18), x, y - 3)
  ctx.fillStyle = p.folded ? '#888' : active ? '#241a05' : p.chips > 0 ? '#7ee0a8' : '#e06f6f'
  ctx.font = '12px system-ui, sans-serif'
  ctx.fillText(`${p.chips} chips`, x, y + 14)
}

// ---- card / chip / bubble primitives ---------------------------------------

function drawHand(ctx, cx, top, hole, showFace, scale, folded) {
  const cw = 46 * scale
  const ch = 64 * scale
  const x0 = cx - cw - 3
  ctx.save()
  if (folded) ctx.globalAlpha = 0.34
  if (showFace) {
    drawCard(ctx, x0, top, cw, ch, hole[0])
    drawCard(ctx, x0 + cw + 6, top, cw, ch, hole[1])
  } else {
    drawCardBack(ctx, x0, top, cw, ch)
    drawCardBack(ctx, x0 + cw + 6, top, cw, ch)
  }
  ctx.restore()
}

function drawCard(ctx, x, y, w, h, card) {
  roundRect(ctx, x, y, w, h, 6)
  ctx.fillStyle = '#fdfdf6'
  ctx.fill()
  ctx.lineWidth = 1
  ctx.strokeStyle = 'rgba(0,0,0,0.25)'
  ctx.stroke()

  ctx.fillStyle = SUIT_COLORS[card.suit]
  ctx.textAlign = 'left'
  ctx.textBaseline = 'top'
  ctx.font = `bold ${Math.round(h * 0.26)}px system-ui, sans-serif`
  ctx.fillText(RANK_LABELS[card.rank], x + 5, y + 4)
  ctx.font = `${Math.round(h * 0.22)}px serif`
  ctx.fillText(SUIT_SYMBOLS[card.suit], x + 5, y + 4 + h * 0.26)

  center(ctx)
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
  ctx.fillStyle = 'rgba(255,255,255,0.06)'
  ctx.fill()
  ctx.strokeStyle = 'rgba(255,255,255,0.14)'
  ctx.lineWidth = 1.5
  ctx.stroke()
}

function drawChip(ctx, x, y, amount) {
  ctx.fillStyle = '#caa45a'
  ctx.beginPath()
  ctx.arc(x, y, 11, 0, Math.PI * 2)
  ctx.fill()
  ctx.strokeStyle = '#fff8e0'
  ctx.lineWidth = 2
  ctx.setLineDash([4, 4])
  ctx.beginPath()
  ctx.arc(x, y, 11, 0, Math.PI * 2)
  ctx.stroke()
  ctx.setLineDash([])
  ctx.fillStyle = '#1a1a1a'
  ctx.font = 'bold 12px system-ui, sans-serif'
  center(ctx)
  ctx.fillText(String(amount), x + 26, y)
}

function drawBubble(ctx, x, y, text) {
  ctx.font = '14px system-ui, sans-serif'
  const tw = Math.min(ctx.measureText(text).width, 220)
  const bw = tw + 24
  const bh = 34
  ctx.fillStyle = '#fffef5'
  roundRect(ctx, x - bw / 2, y - bh, bw, bh, 12)
  ctx.fill()
  ctx.beginPath()
  ctx.moveTo(x - 6, y)
  ctx.lineTo(x + 6, y)
  ctx.lineTo(x, y + 10)
  ctx.closePath()
  ctx.fill()
  ctx.fillStyle = '#222'
  center(ctx)
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
  ctx.textAlign = 'left'
  ctx.textBaseline = 'middle'
  ctx.fillText(trunc(text, 40), x + 52, y + 18)
}

// ---- small helpers ----------------------------------------------------------

function ellipse(ctx, cx, cy, rx, ry) {
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
function center(ctx) {
  ctx.textAlign = 'center'
  ctx.textBaseline = 'middle'
}
function clamp(v, lo, hi) {
  return Math.max(lo, Math.min(v, hi))
}
function trunc(s, max) {
  return s.length > max ? s.slice(0, max - 1) + '…' : s
}
