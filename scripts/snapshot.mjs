// Offline render of the poker table to a PNG, for visual verification.
// Usage: node scripts/snapshot.mjs [outfile]
import { createCanvas } from 'canvas'
import { writeFileSync } from 'node:fs'
import { PokerGame } from '../src/engine/pokerGame.js'
import { decideAction } from '../src/engine/ai.js'
import { CHARACTERS } from '../src/engine/characters.js'
import { draw, W, H } from '../src/ui/tableRender.js'

// Seat the whole 4-character roster for the preview.
const opps = CHARACTERS.slice(0, 4)
const players = [
  { id: 'human', name: 'You', isHuman: true, avatar: '😎', color: '#2c8fd6' },
  ...opps.map((c) => ({ id: c.id, name: c.name, isHuman: false, avatar: c.avatar, color: c.color, character: c })),
]
const game = new PokerGame({ players, seed: 7 })
game.startHand()

// Play a few AI actions to reach the flop with bets/folds on the felt.
let steps = 0
while (game.state.actingIndex !== -1 && game.state.community.length < 3 && steps < 40) {
  const seat = game.state.actingIndex
  if (game.state.players[seat].isHuman) {
    game.act({ type: game.legalActions(seat).check ? 'check' : 'call' })
  } else {
    game.act(decideAction(game, seat))
  }
  steps++
}

// Try to load any character art so the snapshot matches the browser.
const images = {}
for (const p of players) {
  const rel = p.character?.image
  if (rel && !images[rel]) {
    try {
      const { loadImage } = await import('canvas')
      images[rel] = await loadImage(new URL(`../public/${rel}`, import.meta.url))
    } catch {
      /* file not present yet — the drawn fallback will be used */
    }
  }
}

const canvas = createCanvas(W, H)
const ctx = canvas.getContext('2d')
draw(ctx, game.state, { dealerSay: 'And the flop comes down...', images })

const out = process.argv[2] || 'table-preview.png'
writeFileSync(out, canvas.toBuffer('image/png'))
console.log(`Wrote ${out} — street: ${game.state.street}, pot: ${game.pot()}`)
