//! Unit tests for the poker engine. These run with `cargo test` (no graphics),
//! so the rules are actually verifiable in this environment.

use super::*;

/// Parse a 2-char code like "As", "Th", "2c" into a Card (test convenience).
fn card(code: &str) -> Card {
    let mut chars = code.chars();
    let r = chars.next().unwrap();
    let s = chars.next().unwrap();
    let rank = Rank::ALL
        .iter()
        .copied()
        .find(|rk| rk.label() == r)
        .unwrap();
    let suit = Suit::ALL
        .iter()
        .copied()
        .find(|su| su.label() == s)
        .unwrap();
    Card::new(rank, suit)
}

fn hand(codes: &[&str]) -> Vec<Card> {
    codes.iter().map(|c| card(c)).collect()
}

fn eval(codes: &[&str]) -> HandValue {
    evaluate_best(&hand(codes))
}

#[test]
fn card_codes_match_art_names() {
    assert_eq!(card("As").code(), "As");
    assert_eq!(card("Th").code(), "Th");
    assert_eq!(card("2c").code(), "2c");
    assert_eq!(card("Kd").code(), "Kd");
}

#[test]
fn deck_is_52_unique_and_shuffles_deterministically() {
    let deck = Deck::full();
    assert_eq!(deck.len(), 52);
    let mut seen = std::collections::HashSet::new();
    for &c in &deck.cards {
        assert!(seen.insert(c.code()), "duplicate card {}", c.code());
    }
    assert_eq!(seen.len(), 52);

    // Same seed → same order; different seed → (almost certainly) different.
    let mut a = Deck::full();
    let mut b = Deck::full();
    a.shuffle(&mut Rng::new(42));
    b.shuffle(&mut Rng::new(42));
    assert_eq!(a.cards, b.cards);
    let mut c = Deck::full();
    c.shuffle(&mut Rng::new(43));
    assert_ne!(a.cards, c.cards);
    // Still 52 unique after shuffling.
    let after: std::collections::HashSet<_> = a.cards.iter().map(|c| c.code()).collect();
    assert_eq!(after.len(), 52);
}

#[test]
fn hand_category_ordering() {
    let straight_flush = eval(&["9s", "8s", "7s", "6s", "5s", "2d", "2h"]);
    let quads = eval(&["9s", "9d", "9h", "9c", "5s", "2d", "Kh"]);
    let full_house = eval(&["9s", "9d", "9h", "5c", "5s", "2d", "Kh"]);
    let flush = eval(&["As", "Js", "8s", "5s", "3s", "2d", "Kh"]);
    let straight = eval(&["9s", "8d", "7h", "6c", "5s", "2d", "Kh"]);
    let trips = eval(&["9s", "9d", "9h", "5c", "4s", "2d", "Kh"]);
    let two_pair = eval(&["9s", "9d", "5h", "5c", "4s", "2d", "Kh"]);
    let pair = eval(&["9s", "9d", "Jh", "5c", "4s", "2d", "Kh"]);
    let high = eval(&["9s", "7d", "Jh", "5c", "4s", "2d", "Kh"]);

    let mut hands = vec![
        &high,
        &pair,
        &two_pair,
        &trips,
        &straight,
        &flush,
        &full_house,
        &quads,
        &straight_flush,
    ];
    // Already in ascending order; verify each strictly beats the previous.
    for w in hands.windows(2) {
        assert!(w[0] < w[1], "{:?} should be < {:?}", w[0], w[1]);
    }
    hands.sort();
    assert_eq!(*hands.last().unwrap(), &straight_flush);
}

#[test]
fn hand_descriptions_name_the_ranks() {
    assert_eq!(eval(&["Kh", "Kd", "9s", "5c", "2d"]).describe(), "a Pair of Kings");
    assert_eq!(
        eval(&["Kh", "Kd", "8s", "8c", "2d"]).describe(),
        "Two Pair, Kings & Eights"
    );
    assert_eq!(eval(&["7h", "7d", "7s", "5c", "2d"]).describe(), "Three Sevens");
    assert_eq!(
        eval(&["Ah", "Ad", "As", "Tc", "Td"]).describe(),
        "a Full House, Aces full of Tens"
    );
    assert_eq!(
        eval(&["9s", "8d", "7h", "6c", "5s"]).describe(),
        "a Nine-high Straight"
    );
    assert_eq!(eval(&["Qh", "Qd", "Qs", "Qc", "2d"]).describe(), "Four Queens");
}

#[test]
fn wheel_straight_is_five_high() {
    let wheel = eval(&["Ah", "2d", "3c", "4s", "5h", "Kd", "Qd"]);
    assert_eq!(wheel.category, HandCategory::Straight);
    assert_eq!(wheel.tiebreak, vec![5]);

    let six_high = eval(&["2d", "3c", "4s", "5h", "6h", "Kd", "Qd"]);
    assert!(six_high > wheel, "6-high straight beats the wheel");
}

#[test]
fn flush_and_full_house_tiebreaks() {
    // Higher flush wins on the second card.
    let flush_a = eval(&["As", "Ks", "8s", "5s", "3s"]);
    let flush_b = eval(&["As", "Qs", "8s", "5s", "3s"]);
    assert!(flush_a > flush_b);

    // Full house compares trips first, then the pair.
    let kings_full = eval(&["Ks", "Kd", "Kh", "2c", "2s"]);
    let queens_full = eval(&["Qs", "Qd", "Qh", "Ac", "As"]);
    assert!(kings_full > queens_full);
}

#[test]
fn best_of_seven_picks_the_nuts() {
    // Board makes a flush available; player should use it.
    let v = eval(&["As", "Ks", "Qs", "2s", "7s", "2d", "2h"]);
    assert_eq!(v.category, HandCategory::Flush);
}

fn test_players(stacks: &[u32]) -> Vec<Player> {
    stacks
        .iter()
        .enumerate()
        .map(|(i, &s)| Player::new(format!("P{i}"), s, false, [0.5, 0.5, 0.2, 0.2]))
        .collect()
}

#[test]
fn side_pots_award_correctly() {
    // C is all-in for less; A and B match a larger amount. A has the best hand,
    // so A scoops both the main and the side pot.
    let mut g = Game::new(test_players(&[0, 0, 0]), 5, 10, 1);
    g.button = 0;
    g.community = hand(&["Ac", "Kd", "Qh", "Js", "2c"]);
    g.players[0].hole = [card("Th"), card("9h")]; // A: broadway straight
    g.players[1].hole = [card("Ad"), card("Ah")]; // B: trip aces
    g.players[2].hole = [card("2d"), card("2h")]; // C: trip twos
    g.players[0].committed = 300;
    g.players[1].committed = 300;
    g.players[2].committed = 100;
    for p in &mut g.players {
        p.folded = false;
    }

    g.settle();

    // Main pot 300 (all three) + side pot 400 (A,B) all to A.
    assert_eq!(g.players[0].stack, 700);
    assert_eq!(g.players[1].stack, 0);
    assert_eq!(g.players[2].stack, 0);
}

#[test]
fn split_pot_halves_evenly() {
    let mut g = Game::new(test_players(&[0, 0]), 5, 10, 1);
    g.button = 0;
    g.community = hand(&["Ac", "Kd", "Qh", "Js", "Ts"]); // straight on board
    g.players[0].hole = [card("2d"), card("3h")];
    g.players[1].hole = [card("4d"), card("5h")];
    g.players[0].committed = 100;
    g.players[1].committed = 100;

    g.settle();

    // Both play the board (A-high straight) → split 200.
    assert_eq!(g.players[0].stack, 100);
    assert_eq!(g.players[1].stack, 100);
}

#[test]
fn all_in_player_is_not_busted() {
    let mut p = Player::new("x", 0, false, [0.0; 4]);
    // 0 chips but all-in: still live, must NOT be treated as out (don't leave).
    p.all_in = true;
    assert!(!p.busted());
    // 0 chips, not all-in (sitting out): truly out.
    p.all_in = false;
    assert!(p.busted());
    // Has chips: never busted.
    p.stack = 100;
    assert!(!p.busted());
}

#[test]
fn fold_out_winner_takes_pot() {
    let mut g = Game::new(test_players(&[1000, 1000, 1000]), 5, 10, 7);
    g.start_hand();
    let start_total: u32 = 3000;
    // Everyone folds until one remains.
    let mut guard = 0;
    while g.street != Street::HandOver && guard < 50 {
        g.apply(Action::Fold);
        guard += 1;
    }
    assert_eq!(g.street, Street::HandOver);
    assert_eq!(g.live_count(), 1);
    assert_eq!(total_chips(&g), start_total);
}

fn total_chips(g: &Game) -> u32 {
    g.players.iter().map(|p| p.stack).sum::<u32>() + g.pot()
}

#[test]
fn full_ai_hands_conserve_chips_and_terminate() {
    for seed in 0..40u64 {
        let mut g = Game::new(test_players(&[1000, 1000, 1000, 1000, 1000, 1000]), 5, 10, seed);
        let start = total_chips(&g);
        g.start_hand();
        // Chips are conserved at every step.
        assert_eq!(total_chips(&g), start, "seed {seed}: chips not conserved at deal");

        let mut guard = 0;
        while g.street != Street::HandOver {
            let seat = g.to_act;
            let action = ai_decide(&mut g, seat);
            g.apply(action);
            assert_eq!(
                total_chips(&g),
                start,
                "seed {seed}: chips not conserved after action"
            );
            guard += 1;
            assert!(guard < 500, "seed {seed}: hand did not terminate");
        }
        // After the hand, all chips are back in stacks.
        assert_eq!(g.players.iter().map(|p| p.stack).sum::<u32>(), start);
        assert!(!g.last_payouts.is_empty(), "seed {seed}: nobody won the pot");
    }
}

#[test]
fn many_hands_in_a_row_stay_consistent() {
    let mut g = Game::new(test_players(&[1000, 1000, 1000, 1000, 1000, 1000]), 5, 10, 99);
    let start = total_chips(&g);
    for _ in 0..60 {
        if g.players.iter().filter(|p| p.stack > 0).count() < 2 {
            break; // someone won everything
        }
        g.start_hand();
        let mut guard = 0;
        while g.street != Street::HandOver {
            let seat = g.to_act;
            let action = ai_decide(&mut g, seat);
            g.apply(action);
            guard += 1;
            assert!(guard < 500, "hand did not terminate");
        }
        assert_eq!(g.players.iter().map(|p| p.stack).sum::<u32>(), start);
    }
}
