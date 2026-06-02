// ---------------------------------------------------------------------------
// optimize-art.mjs — shrink + recompress character PNGs for the web.
//
//   npm run art:optimize
//
// Downscales any image in public/characters/ to a sensible max width and
// rewrites it as a PNG. Standees are drawn ~190px tall on the table, so there
// is no reason to ship multi-thousand-pixel art. Safe to re-run (idempotent
// once images are already small).
//
// Requires the dev-only `canvas` package (already in devDependencies). If it
// fails to install on your machine, you can skip this — big PNGs still display
// fine, they just download slower.
// ---------------------------------------------------------------------------

import { readdirSync, statSync, writeFileSync } from 'node:fs'
import { join, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'

const MAX_W = 420 // plenty for a standee; tweak if you want crisper zoom
const dir = join(dirname(fileURLToPath(import.meta.url)), '..', 'public', 'characters')

let createCanvas, loadImage
try {
  ;({ createCanvas, loadImage } = await import('canvas'))
} catch {
  console.error('The "canvas" package is required. Run: npm i -D canvas')
  process.exit(1)
}

const files = readdirSync(dir).filter((f) => /\.(png|jpe?g)$/i.test(f))
if (!files.length) {
  console.log(`No images in ${dir} yet — drop your PNGs there first.`)
  process.exit(0)
}

const kb = (n) => `${Math.round(n / 1024)}KB`
for (const f of files) {
  const path = join(dir, f)
  const before = statSync(path).size
  const img = await loadImage(path)
  const scale = Math.min(1, MAX_W / img.width)
  const w = Math.round(img.width * scale)
  const h = Math.round(img.height * scale)
  const canvas = createCanvas(w, h)
  canvas.getContext('2d').drawImage(img, 0, 0, w, h)
  writeFileSync(path, canvas.toBuffer('image/png'))
  const after = statSync(path).size
  console.log(`${f.padEnd(16)} ${img.width}×${img.height} → ${w}×${h}   ${kb(before)} → ${kb(after)}`)
}
console.log('Done.')
