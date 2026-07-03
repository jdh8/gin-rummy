//! Deadwood fixtures with hand-computed answers

use gin_rummy::{Hand, MeldKind, Suit, best_melds, deadwood, pip_sum};

fn hand(s: &str) -> Hand {
    s.parse().unwrap()
}

#[test]
fn run_beats_set_when_cheaper() {
    // ♠4♠5♠6 competes with the set of fours for ♠4.  The run strands ♦4 ♥4
    // (8 points); the set strands ♠5 ♠6 (11 points).  The run wins.
    let conflicted = hand("TQ.49.49J.456");
    assert_eq!(deadwood(conflicted), 56);

    let melds = best_melds(conflicted);
    assert_eq!(melds.deadwood(), 56);
    let run = melds.iter().next().unwrap();
    assert_eq!(run.kind(), MeldKind::Run);
    assert_eq!(run.suit(), Some(Suit::Spades));
}

#[test]
fn set_beats_run_when_cheaper() {
    // ♥5 sits in both the ♥3♥4♥5 run and the set of fives.  The set strands
    // ♥3 ♥4 (7 points); the run strands ♦5 ♠5 (10 points).  The set wins.
    let conflicted = hand("JK.5JK.345.25");
    assert_eq!(deadwood(conflicted), 49);

    let melds = best_melds(conflicted);
    let set = melds.iter().next().unwrap();
    assert_eq!(set.kind(), MeldKind::Set);
    assert_eq!(set.len(), 3);
}

#[test]
fn gin_hands() {
    // 4 + 3 + 3
    assert_eq!(deadwood(hand("A234.567.9TJ.")), 0);
    // 3 + 4 + 4 (big gin shape)
    assert_eq!(deadwood(hand("A23.4567.89TJ.")), 0);
    // 3 + 3 + 5 (big gin shape)
    assert_eq!(deadwood(hand("A23.456.789TJ.")), 0);

    let big_gin = best_melds(hand("A23.456.789TJ."));
    assert_eq!(big_gin.iter().count(), 3);
    assert!(big_gin.deadwood_cards().is_empty());
}

#[test]
fn eleven_card_single_suit_run() {
    // The worst case for the solver: 45 applicable melds.
    let run = hand("A23456789TJ...");
    assert_eq!(run.len(), 11);
    assert_eq!(deadwood(run), 0);
    assert_eq!(best_melds(run).iter().count(), 3);
}

#[test]
fn near_misses() {
    // No three cards meld anywhere.
    let nothing = hand("A47.28J.35Q.6K");
    assert_eq!(deadwood(nothing), u8::try_from(pip_sum(nothing)).unwrap());
    assert_eq!(best_melds(nothing).iter().count(), 0);

    // One meld, mixed leftovers: ♣A♣2♣3 melds, the rest is deadwood.
    let one_run = hand("A23.2TJ.5Q.9K");
    assert_eq!(deadwood(one_run), 2 + 10 + 10 + 5 + 10 + 9 + 10);
}

#[test]
fn best_melds_panics_beyond_eleven_cards() {
    let result = std::panic::catch_unwind(|| best_melds(Hand::ALL));
    assert!(result.is_err());
}
