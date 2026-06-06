//! poker.rs — a self-contained Texas Hold'em engine: cards, deck, 7-card hand
//! evaluation, a No-Limit betting state machine (with side pots), and a simple
//! personality-driven AI.
//!
//! Deliberately has **no Bevy dependency** so the rules can be unit-tested with
//! `cargo test` (rendering can't be). The Bevy layer drives this engine and
//! reads its state to lay out cards, chips, and the betting UI.
#![allow(dead_code)] // engine is wired into the Bevy scene incrementally

use std::cmp::Ordering;

// ---------------------------------------------------------------------------
// Cards
// ---------------------------------------------------------------------------

/// Ranks in ascending order, so the derived `Ord` matches poker strength.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub enum Rank {
    Two,
    Three,
    Four,
    Five,
    Six,
    Seven,
    Eight,
    Nine,
    Ten,
    Jack,
    Queen,
    King,
    Ace,
}

impl Rank {
    /// Numeric value 2..=14 (Ace high).
    pub fn value(self) -> u8 {
        self as u8 + 2
    }

    /// Single-character label used by the card art filenames (`T`, `J`, …).
    pub fn label(self) -> char {
        match self {
            Rank::Two => '2',
            Rank::Three => '3',
            Rank::Four => '4',
            Rank::Five => '5',
            Rank::Six => '6',
            Rank::Seven => '7',
            Rank::Eight => '8',
            Rank::Nine => '9',
            Rank::Ten => 'T',
            Rank::Jack => 'J',
            Rank::Queen => 'Q',
            Rank::King => 'K',
            Rank::Ace => 'A',
        }
    }

    pub const ALL: [Rank; 13] = [
        Rank::Two,
        Rank::Three,
        Rank::Four,
        Rank::Five,
        Rank::Six,
        Rank::Seven,
        Rank::Eight,
        Rank::Nine,
        Rank::Ten,
        Rank::Jack,
        Rank::Queen,
        Rank::King,
        Rank::Ace,
    ];
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Suit {
    Clubs,
    Diamonds,
    Hearts,
    Spades,
}

impl Suit {
    /// Single-character label used by the card art filenames (`c`, `d`, …).
    pub fn label(self) -> char {
        match self {
            Suit::Clubs => 'c',
            Suit::Diamonds => 'd',
            Suit::Hearts => 'h',
            Suit::Spades => 's',
        }
    }

    pub const ALL: [Suit; 4] = [Suit::Clubs, Suit::Diamonds, Suit::Hearts, Suit::Spades];
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct Card {
    pub rank: Rank,
    pub suit: Suit,
}

impl Card {
    pub fn new(rank: Rank, suit: Suit) -> Self {
        Card { rank, suit }
    }

    /// e.g. `"As"`, `"Th"`, `"2c"` — matches the `cards/<code>.png` art.
    pub fn code(self) -> String {
        format!("{}{}", self.rank.label(), self.suit.label())
    }
}

// ---------------------------------------------------------------------------
// RNG — small deterministic PRNG (SplitMix64) so shuffles are seedable/testable
// without pulling in the `rand` crate.
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed.wrapping_add(0x9E37_79B9_7F4A_7C15))
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform integer in `0..n`.
    pub fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }

    /// Uniform float in `0.0..1.0`.
    pub fn unit(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }
}

// ---------------------------------------------------------------------------
// Deck
// ---------------------------------------------------------------------------

pub struct Deck {
    cards: Vec<Card>,
}

impl Deck {
    /// A fresh, ordered 52-card deck.
    pub fn full() -> Self {
        let mut cards = Vec::with_capacity(52);
        for &suit in &Suit::ALL {
            for &rank in &Rank::ALL {
                cards.push(Card::new(rank, suit));
            }
        }
        Deck { cards }
    }

    /// Fisher–Yates shuffle with the given RNG.
    pub fn shuffle(&mut self, rng: &mut Rng) {
        let n = self.cards.len();
        for i in (1..n).rev() {
            let j = rng.below(i + 1);
            self.cards.swap(i, j);
        }
    }

    pub fn deal(&mut self) -> Card {
        self.cards.pop().expect("dealt from an empty deck")
    }

    pub fn len(&self) -> usize {
        self.cards.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cards.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Hand evaluation (best 5 of up to 7 cards)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum HandCategory {
    HighCard,
    Pair,
    TwoPair,
    Trips,
    Straight,
    Flush,
    FullHouse,
    Quads,
    StraightFlush,
}

impl HandCategory {
    pub fn name(self) -> &'static str {
        match self {
            HandCategory::HighCard => "High Card",
            HandCategory::Pair => "Pair",
            HandCategory::TwoPair => "Two Pair",
            HandCategory::Trips => "Three of a Kind",
            HandCategory::Straight => "Straight",
            HandCategory::Flush => "Flush",
            HandCategory::FullHouse => "Full House",
            HandCategory::Quads => "Four of a Kind",
            HandCategory::StraightFlush => "Straight Flush",
        }
    }
}

/// A fully comparable hand value: category first, then rank-ordered tiebreakers.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct HandValue {
    pub category: HandCategory,
    pub tiebreak: Vec<u8>,
}

impl PartialOrd for HandValue {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for HandValue {
    fn cmp(&self, other: &Self) -> Ordering {
        self.category
            .cmp(&other.category)
            .then_with(|| self.tiebreak.cmp(&other.tiebreak))
    }
}

/// If the (unique) rank set contains a 5-card straight, return its high card.
/// Handles the wheel (A-2-3-4-5, high card 5).
fn straight_high(unique_desc: &[u8]) -> Option<u8> {
    if unique_desc.len() < 5 {
        return None;
    }
    // Normal run of 5 consecutive.
    for w in unique_desc.windows(5) {
        if w[0] - w[4] == 4 {
            return Some(w[0]);
        }
    }
    // Wheel: Ace plays low.
    if unique_desc.contains(&14)
        && unique_desc.contains(&5)
        && unique_desc.contains(&4)
        && unique_desc.contains(&3)
        && unique_desc.contains(&2)
    {
        return Some(5);
    }
    None
}

/// Evaluate exactly five cards.
fn eval5(cards: &[Card; 5]) -> HandValue {
    let mut values: Vec<u8> = cards.iter().map(|c| c.rank.value()).collect();
    values.sort_unstable_by(|a, b| b.cmp(a)); // descending

    let is_flush = cards.iter().all(|c| c.suit == cards[0].suit);

    let mut unique = values.clone();
    unique.dedup();
    let straight = straight_high(&unique);

    // Group by rank value: (count, value), sorted by count desc then value desc.
    let mut groups: Vec<(u8, u8)> = Vec::new();
    for &v in &values {
        if let Some(g) = groups.iter_mut().find(|g| g.1 == v) {
            g.0 += 1;
        } else {
            groups.push((1, v));
        }
    }
    groups.sort_unstable_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));

    let counts: Vec<u8> = groups.iter().map(|g| g.0).collect();

    let category = if straight.is_some() && is_flush {
        HandCategory::StraightFlush
    } else if counts[0] == 4 {
        HandCategory::Quads
    } else if counts[0] == 3 && counts.get(1) == Some(&2) {
        HandCategory::FullHouse
    } else if is_flush {
        HandCategory::Flush
    } else if straight.is_some() {
        HandCategory::Straight
    } else if counts[0] == 3 {
        HandCategory::Trips
    } else if counts[0] == 2 && counts.get(1) == Some(&2) {
        HandCategory::TwoPair
    } else if counts[0] == 2 {
        HandCategory::Pair
    } else {
        HandCategory::HighCard
    };

    let tiebreak = match category {
        HandCategory::StraightFlush | HandCategory::Straight => vec![straight.unwrap()],
        HandCategory::Flush | HandCategory::HighCard => values.clone(),
        // For grouped hands, group order (count desc, value desc) is the
        // correct tiebreak ordering already.
        _ => groups.iter().map(|g| g.1).collect(),
    };

    HandValue { category, tiebreak }
}

/// Best 5-card value from 5, 6, or 7 cards.
pub fn evaluate_best(cards: &[Card]) -> HandValue {
    assert!(cards.len() >= 5 && cards.len() <= 7);
    let n = cards.len();
    let mut best: Option<HandValue> = None;
    // Enumerate all 5-card subsets.
    for a in 0..n {
        for b in (a + 1)..n {
            for c in (b + 1)..n {
                for d in (c + 1)..n {
                    for e in (d + 1)..n {
                        let hand = [cards[a], cards[b], cards[c], cards[d], cards[e]];
                        let v = eval5(&hand);
                        if best.as_ref().map_or(true, |best| v > *best) {
                            best = Some(v);
                        }
                    }
                }
            }
        }
    }
    best.unwrap()
}

// ---------------------------------------------------------------------------
// Game state machine
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Street {
    Preflop,
    Flop,
    Turn,
    River,
    Showdown,
    HandOver,
}

/// A player action. `Raise(to)` raises the player's total bet *this street* up
/// to `to` chips; going all-in is just a raise/call clamped to the stack.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    Fold,
    Check,
    Call,
    Raise(u32),
}

#[derive(Clone)]
pub struct Player {
    pub name: String,
    pub stack: u32,
    pub hole: [Card; 2],
    pub folded: bool,
    pub all_in: bool,
    pub bet: u32,       // chips committed this street
    pub committed: u32, // total chips committed this hand (for side pots)
    pub acted: bool,    // has acted since the last bet/raise this street
    pub is_human: bool,
    pub profile: [f32; 4], // [aggression, tightness, bluff, tilt]
}

impl Player {
    pub fn new(name: impl Into<String>, stack: u32, is_human: bool, profile: [f32; 4]) -> Self {
        Player {
            name: name.into(),
            stack,
            hole: [
                Card::new(Rank::Two, Suit::Clubs),
                Card::new(Rank::Two, Suit::Clubs),
            ],
            folded: false,
            all_in: false,
            bet: 0,
            committed: 0,
            acted: false,
            is_human,
            profile,
        }
    }

    /// Still contesting the pot (not folded, has chips or is all-in).
    pub fn in_hand(&self) -> bool {
        !self.folded
    }
}

/// A pot awarded at showdown (or sooner).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Payout {
    pub seat: usize,
    pub amount: u32,
}

pub struct Game {
    pub players: Vec<Player>,
    pub button: usize,
    pub community: Vec<Card>,
    pub street: Street,
    pub to_act: usize,
    pub current_bet: u32, // highest `bet` this street
    pub min_raise: u32,   // minimum legal raise increment
    pub small_blind: u32,
    pub big_blind: u32,
    pub last_payouts: Vec<Payout>,
    deck: Deck,
    rng: Rng,
}

impl Game {
    pub fn new(players: Vec<Player>, small_blind: u32, big_blind: u32, seed: u64) -> Self {
        Game {
            players,
            button: 0,
            community: Vec::new(),
            street: Street::HandOver,
            to_act: 0,
            current_bet: 0,
            min_raise: big_blind,
            small_blind,
            big_blind,
            last_payouts: Vec::new(),
            deck: Deck::full(),
            rng: Rng::new(seed),
        }
    }

    /// Total chips in the middle (collected + current-street bets).
    pub fn pot(&self) -> u32 {
        self.players.iter().map(|p| p.committed).sum()
    }

    /// Seats that still hold cards.
    pub fn live_count(&self) -> usize {
        self.players.iter().filter(|p| p.in_hand()).count()
    }

    /// Deal a new hand: move the button, post blinds, deal hole cards.
    pub fn start_hand(&mut self) {
        let n = self.players.len();
        for p in &mut self.players {
            p.folded = p.stack == 0; // chips-out players sit out
            p.all_in = false;
            p.bet = 0;
            p.committed = 0;
            p.acted = false;
        }
        self.community.clear();
        self.last_payouts.clear();
        self.current_bet = 0;
        self.min_raise = self.big_blind;

        // Shuffle and deal two hole cards to each live player (one at a time,
        // starting left of the button, like a real deal).
        self.deck = Deck::full();
        self.deck.shuffle(&mut self.rng);
        self.button = self.next_live(self.button);
        for round in 0..2 {
            let mut seat = self.button;
            for _ in 0..n {
                seat = (seat + 1) % n;
                if self.players[seat].in_hand() {
                    let card = self.deck.deal();
                    self.players[seat].hole[round] = card;
                }
            }
        }

        // Post blinds.
        let sb = self.next_live(self.button);
        let bb = self.next_live(sb);
        self.post(sb, self.small_blind);
        self.post(bb, self.big_blind);
        self.current_bet = self.big_blind;
        self.min_raise = self.big_blind;

        self.street = Street::Preflop;
        self.to_act = self.next_to_act_from(bb);
    }

    fn next_live(&self, from: usize) -> usize {
        let n = self.players.len();
        let mut i = (from + 1) % n;
        let mut guard = 0;
        while !self.players[i].in_hand() && guard < n {
            i = (i + 1) % n;
            guard += 1;
        }
        i
    }

    fn post(&mut self, seat: usize, amount: u32) {
        let p = &mut self.players[seat];
        let pay = amount.min(p.stack);
        p.stack -= pay;
        p.bet += pay;
        p.committed += pay;
        if p.stack == 0 {
            p.all_in = true;
        }
    }

    /// Does this seat still owe an action this street?
    fn needs_to_act(&self, seat: usize) -> bool {
        let p = &self.players[seat];
        p.in_hand() && !p.all_in && (!p.acted || p.bet < self.current_bet)
    }

    /// Next seat (after `from`) that needs to act, if any.
    fn next_to_act_from(&self, from: usize) -> usize {
        let n = self.players.len();
        let mut i = (from + 1) % n;
        for _ in 0..n {
            if self.needs_to_act(i) {
                return i;
            }
            i = (i + 1) % n;
        }
        from // none — caller checks round completion separately
    }

    fn round_complete(&self) -> bool {
        !(0..self.players.len()).any(|i| self.needs_to_act(i))
    }

    /// How much `seat` must put in to call.
    pub fn call_amount(&self, seat: usize) -> u32 {
        let p = &self.players[seat];
        (self.current_bet.saturating_sub(p.bet)).min(p.stack)
    }

    /// Smallest legal total this seat can raise *to* (None if they can't raise).
    pub fn min_raise_to(&self, seat: usize) -> Option<u32> {
        let p = &self.players[seat];
        if p.all_in || !p.in_hand() {
            return None;
        }
        let max_to = p.bet + p.stack;
        if max_to <= self.current_bet {
            return None; // can't even cover a call; only call(all-in)/fold
        }
        Some((self.current_bet + self.min_raise).min(max_to))
    }

    /// Largest total this seat can raise *to* (their whole stack).
    pub fn max_raise_to(&self, seat: usize) -> u32 {
        let p = &self.players[seat];
        p.bet + p.stack
    }

    pub fn can_check(&self, seat: usize) -> bool {
        self.players[seat].bet == self.current_bet
    }

    /// Apply an action for the player whose turn it is. Advances the game.
    pub fn apply(&mut self, action: Action) {
        let seat = self.to_act;
        match action {
            Action::Fold => {
                self.players[seat].folded = true;
            }
            Action::Check => {
                // If there's actually a bet to face, treat it as a call so a
                // stray check can never leave an unmatched bet.
                if self.players[seat].bet < self.current_bet {
                    let amt = self.call_amount(seat);
                    self.move_chips(seat, amt);
                }
                self.players[seat].acted = true;
            }
            Action::Call => {
                let amt = self.call_amount(seat);
                self.move_chips(seat, amt);
                self.players[seat].acted = true;
            }
            Action::Raise(to) => {
                let max_to = self.max_raise_to(seat);
                let to = to.clamp(self.current_bet, max_to);
                let add = to - self.players[seat].bet;
                self.move_chips(seat, add);
                let increment = to.saturating_sub(self.current_bet);
                if increment >= self.min_raise {
                    self.min_raise = increment;
                }
                if to > self.current_bet {
                    self.current_bet = to;
                    // Re-open the action: everyone else must respond.
                    for (i, p) in self.players.iter_mut().enumerate() {
                        if i != seat && p.in_hand() && !p.all_in {
                            p.acted = false;
                        }
                    }
                }
                self.players[seat].acted = true;
            }
        }

        self.advance();
    }

    fn move_chips(&mut self, seat: usize, amount: u32) {
        let p = &mut self.players[seat];
        let pay = amount.min(p.stack);
        p.stack -= pay;
        p.bet += pay;
        p.committed += pay;
        if p.stack == 0 {
            p.all_in = true;
        }
    }

    /// Advance the game after an action: pick the next actor, deal the next
    /// street, or go to showdown — looping past all-in players as needed.
    fn advance(&mut self) {
        // Everyone but one folded → that player wins immediately.
        if self.live_count() <= 1 {
            self.settle();
            return;
        }

        if !self.round_complete() {
            self.to_act = self.next_to_act_from(self.to_act);
            return;
        }

        // Betting round done — keep dealing streets until someone can act or
        // we reach showdown.
        loop {
            match self.street {
                Street::Preflop => self.deal_community(3, Street::Flop),
                Street::Flop => self.deal_community(1, Street::Turn),
                Street::Turn => self.deal_community(1, Street::River),
                Street::River => {
                    self.settle();
                    return;
                }
                Street::Showdown | Street::HandOver => return,
            }

            // Can anyone actually act this street?
            let actor = (0..self.players.len()).find(|&i| self.needs_to_act(i));
            if let Some(seat) = actor {
                // Only meaningful if more than one player can still act.
                let can_act = (0..self.players.len())
                    .filter(|&i| self.players[i].in_hand() && !self.players[i].all_in)
                    .count();
                if can_act >= 2 {
                    self.to_act = seat;
                    return;
                }
            }
            // Otherwise loop: deal the next street.
        }
    }

    fn deal_community(&mut self, count: usize, next: Street) {
        for p in &mut self.players {
            p.bet = 0;
            p.acted = false;
        }
        self.current_bet = 0;
        self.min_raise = self.big_blind;
        for _ in 0..count {
            let c = self.deck.deal();
            self.community.push(c);
        }
        self.street = next;
        // First to act postflop is the first live player left of the button.
        self.to_act = self.next_to_act_from(self.button);
    }

    /// Resolve the hand: build side pots from contributions, award to the best
    /// eligible hand(s), credit stacks, and record payouts.
    fn settle(&mut self) {
        let n = self.players.len();
        let mut payouts: Vec<u32> = vec![0; n];

        // Single player left (everyone folded): they take the whole pot.
        if self.live_count() == 1 {
            let winner = (0..n).find(|&i| self.players[i].in_hand()).unwrap();
            let total = self.pot();
            payouts[winner] = total;
        } else {
            // Side pots from per-player contributions. Each layer is defined by
            // the smallest remaining contribution; everyone who paid into the
            // layer forms that pot, and the live (non-folded) contributors are
            // eligible to win it.
            let mut contrib: Vec<u32> = self.players.iter().map(|p| p.committed).collect();
            loop {
                let layer = match contrib.iter().cloned().filter(|&c| c > 0).min() {
                    Some(l) => l,
                    None => break,
                };
                let contributors: Vec<usize> = (0..n).filter(|&i| contrib[i] > 0).collect();
                let mut pot_amount = 0u32;
                for &i in &contributors {
                    contrib[i] -= layer;
                    pot_amount += layer;
                }

                let mut eligible: Vec<usize> = contributors
                    .iter()
                    .cloned()
                    .filter(|&i| self.players[i].in_hand())
                    .collect();
                // Fallback (shouldn't happen): if every contributor to this
                // layer folded, award it to the best remaining live hand.
                if eligible.is_empty() {
                    eligible = (0..n).filter(|&i| self.players[i].in_hand()).collect();
                }

                let winners = self.best_hands(&eligible);
                if winners.is_empty() {
                    continue;
                }
                let share = pot_amount / winners.len() as u32;
                let mut remainder = pot_amount % winners.len() as u32;
                for &w in &winners {
                    payouts[w] += share;
                }
                // Odd chip(s) go to the first winner(s) left of the button.
                let mut seat = self.button;
                while remainder > 0 {
                    seat = (seat + 1) % n;
                    if winners.contains(&seat) {
                        payouts[seat] += 1;
                        remainder -= 1;
                    }
                }
            }
        }

        for (i, p) in self.players.iter_mut().enumerate() {
            p.stack += payouts[i];
            // Pot has been distributed back into stacks; clear the round/hand
            // contributions so `pot()` reads 0 and chip totals stay consistent.
            p.bet = 0;
            p.committed = 0;
        }
        self.last_payouts = (0..n)
            .filter(|&i| payouts[i] > 0)
            .map(|i| Payout {
                seat: i,
                amount: payouts[i],
            })
            .collect();
        self.street = Street::HandOver;
    }

    /// Among `seats`, the indices holding the best (tied) hand.
    fn best_hands(&self, seats: &[usize]) -> Vec<usize> {
        let mut best: Option<HandValue> = None;
        let mut winners: Vec<usize> = Vec::new();
        for &i in seats {
            let mut cards = self.community.clone();
            cards.push(self.players[i].hole[0]);
            cards.push(self.players[i].hole[1]);
            let v = evaluate_best(&cards);
            match &best {
                None => {
                    best = Some(v);
                    winners = vec![i];
                }
                Some(b) => match v.cmp(b) {
                    Ordering::Greater => {
                        best = Some(v);
                        winners = vec![i];
                    }
                    Ordering::Equal => winners.push(i),
                    Ordering::Less => {}
                },
            }
        }
        winners
    }

    /// The best made hand for a seat (for UI / showdown labels).
    pub fn hand_value(&self, seat: usize) -> Option<HandValue> {
        if self.community.len() < 3 {
            return None;
        }
        let mut cards = self.community.clone();
        cards.push(self.players[seat].hole[0]);
        cards.push(self.players[seat].hole[1]);
        Some(evaluate_best(&cards))
    }
}

// ---------------------------------------------------------------------------
// Simple personality-driven AI
// ---------------------------------------------------------------------------

/// A rough 0..1 estimate of how good `seat`'s hand is right now.
fn hand_strength(game: &Game, seat: usize) -> f32 {
    let p = &game.players[seat];
    if game.community.is_empty() {
        // Preflop: a light Chen-style score normalised to ~0..1.
        let (a, b) = (p.hole[0], p.hole[1]);
        let hi = a.rank.value().max(b.rank.value()) as f32;
        let lo = a.rank.value().min(b.rank.value()) as f32;
        let pair = a.rank == b.rank;
        let suited = a.suit == b.suit;
        let gap = (hi - lo) as f32;
        let mut s = hi / 14.0 * 0.5;
        if pair {
            s += 0.35 + hi / 14.0 * 0.15;
        }
        if suited {
            s += 0.08;
        }
        if gap == 1.0 {
            s += 0.06;
        }
        s.clamp(0.0, 1.0)
    } else {
        // Postflop: map the made-hand category onto 0..1.
        let v = game.hand_value(seat).unwrap();
        let base = match v.category {
            HandCategory::HighCard => 0.12,
            HandCategory::Pair => 0.32,
            HandCategory::TwoPair => 0.52,
            HandCategory::Trips => 0.68,
            HandCategory::Straight => 0.80,
            HandCategory::Flush => 0.86,
            HandCategory::FullHouse => 0.92,
            HandCategory::Quads => 0.97,
            HandCategory::StraightFlush => 1.0,
        };
        base
    }
}

/// Choose a (legal) action for an AI seat using its personality profile
/// `[aggression, tightness, bluff, tilt]`.
pub fn ai_decide(game: &mut Game, seat: usize) -> Action {
    let strength = hand_strength(game, seat);
    let [aggression, tightness, bluff, _tilt] = game.players[seat].profile;
    let call = game.call_amount(seat);
    let pot = game.pot().max(1);
    let r = game.rng.unit();

    // Pot odds for calling.
    let pot_odds = call as f32 / (pot + call) as f32;

    // Bluff occasionally when checked to / facing a small bet.
    let bluffing = r < bluff * 0.18;

    let want_raise = strength > 0.62 + aggression * -0.12 || bluffing;
    let want_continue = strength + 0.05 > pot_odds + tightness * 0.18;

    if call == 0 {
        // No bet to face: check, or bet for value/bluff.
        if want_raise {
            if let Some(min_to) = game.min_raise_to(seat) {
                let sizing = (pot as f32 * (0.4 + aggression * 0.5)) as u32;
                let to = (game.current_bet + sizing).max(min_to).min(game.max_raise_to(seat));
                return Action::Raise(to);
            }
        }
        Action::Check
    } else if want_raise && strength > 0.5 {
        if let Some(min_to) = game.min_raise_to(seat) {
            let sizing = (pot as f32 * (0.5 + aggression * 0.6)) as u32;
            let to = (game.current_bet + sizing).max(min_to).min(game.max_raise_to(seat));
            return Action::Raise(to);
        }
        Action::Call
    } else if want_continue {
        Action::Call
    } else {
        Action::Fold
    }
}

#[cfg(test)]
mod tests;
