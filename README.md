# Cartoon Hold'em

A cartoony **52-card Texas Hold'em** game you play against 1–5 AI characters,
each with their own personality and table-talk — built as a **foundation you
can mutate** into your own custom card game.

Inspired by the vibe of *Poker Night at the Inventory* (original characters,
not the licensed ones), with a host dealer who narrates the action.

```
npm install
npm run dev      # play at http://localhost:5173
npm test         # engine unit tests (hand ranking, side pots, full-hand sim)
npm run build    # production bundle in dist/
```

## What's in the base game

- Full Texas Hold'em rules: blinds, four betting streets, raises/re-raises,
  all-ins, and correct **side-pot** splitting.
- **Smart AI** opponents: each decision uses a Monte-Carlo equity estimate vs.
  pot odds, flavored by the character's personality (aggression / tightness /
  bluff).
- **Canvas-rendered** table: oval felt, seats, hole cards, community cards,
  pot, dealer button, and floating speech bubbles for character barks.
- A roster of 6 original cartoon characters + **Dottie the Dealer** as host.

## Architecture — built to be mutated

Everything is split so you can change the *game* without touching the *UI*.

```
src/
  engine/                 <- pure JavaScript, no React. The game itself.
    cards.js              .  52-card deck, seedable shuffle
    handEvaluator.js      .  rank any 5-7 cards into a comparable score
    pokerGame.js          .  the state machine (deal -> bet -> showdown -> award)
    ai.js                 .  opponent brains (equity + personality)
    characters.js         .  the cast: identity, personality, barks, power slot
    powers.js             .  the EXTENSION SEAM (hooks). Empty in the base game.
    examplePowers.js      .  copy-paste templates for custom abilities
  ui/                     <- React + canvas. Pure presentation.
    useGame.js            .  bridges the mutable engine to React, drives AI turns
    Table.jsx             .  draws the table to a <canvas>
    Controls.jsx          .  the human's Fold / Check / Call / Raise bar
  App.jsx                 .  setup screen + game screen
```

The golden rule: **game rules live in `engine/`, never in the UI.** The UI just
reads `game.state` and calls `game.act(...)`.

## Giving characters special powers

This is the part to build on. The engine broadcasts named **hooks** at every
meaningful moment (`onHandStart`, `onDeal`, `onStreet`, `beforeAction`,
`afterAction`, `filterEvaluation`, `filterAiDecision`, `onShowdown`,
`onHandEnd` — all documented in `powers.js`). A **power** is just a bundle of
hook handlers.

The base game registers **zero** powers, so the hooks are no-ops. To add one:

```js
import { PowerRegistry } from './engine/powers.js'
import { extraHoleCard } from './engine/examplePowers.js'
import { PokerGame } from './engine/pokerGame.js'

const powers = new PowerRegistry([
  extraHoleCard('baron'),   // the Baron secretly gets a 3rd hole card
])
const game = new PokerGame({ players, powers })
```

`examplePowers.js` ships four worked templates:

| Power            | Hook used          | Idea                                   |
|------------------|--------------------|----------------------------------------|
| `extraHoleCard`  | `onDeal`           | deal a player an extra card            |
| `luckySuit`      | `filterEvaluation` | bias a player's showdown hand          |
| `bluffCatcher`   | `filterAiDecision` | override an AI's choice                 |
| `chaoticRiver`   | `onStreet`         | re-draw the river for chaos            |

Each character in `characters.js` has a `power: null` slot ready to point at
one of these (or your own).

## Tuning knobs

- **Personalities:** the `profile` numbers in `characters.js`
  (`aggression`, `tightness`, `bluff`, `tilt`).
- **AI strength:** Monte-Carlo `iterations` and the equity/pot-odds thresholds
  in `ai.js`.
- **Stakes:** `smallBlind` / `bigBlind` / `startingChips` in `App.jsx`'s setup.
- **Look:** swap the emoji avatars in `characters.js` and the `drawAvatar()`
  function in `Table.jsx` for your own art when you're ready.

## Next steps you might take

- Replace emoji avatars with sprite art per character.
- Flesh out one signature power per character and wire `character.power`.
- Add a tournament/blind-increase structure, or a campaign of opponents.
