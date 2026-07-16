//! Property tests: text round-trips, bitset invariants, an independent
//! cross-check of the deadwood solver, and random legal playouts

use gin_rummy::{
    Card, Hand, Holding, Meld, OklahomaAce, Phase, Player, Rank, Round, RoundResult, Rules, Suit,
    best_melds, deadwood,
};
use proptest::prelude::*;

fn deck() -> Vec<Card> {
    Suit::ASC
        .into_iter()
        .flat_map(|suit| {
            (1..=13).map(move |rank| Card {
                suit,
                rank: Rank::new(rank),
            })
        })
        .collect()
}

fn rank() -> impl Strategy<Value = Rank> {
    (1u8..=13).prop_map(Rank::new)
}

fn suit() -> impl Strategy<Value = Suit> {
    (0usize..4).prop_map(|index| Suit::ASC[index])
}

fn card() -> impl Strategy<Value = Card> {
    (suit(), rank()).prop_map(|(suit, rank)| Card { suit, rank })
}

fn holding() -> impl Strategy<Value = Holding> {
    any::<u16>().prop_map(Holding::from_bits_truncate)
}

fn hand() -> impl Strategy<Value = Hand> {
    any::<u64>().prop_map(Hand::from_bits_truncate)
}

fn meld() -> impl Strategy<Value = Meld> {
    let set =
        (rank(), proptest::option::of(suit())).prop_map(|(rank, missing)| Meld::set(rank, missing));
    let run = (suit(), 1u8..=11).prop_flat_map(|(suit, low)| {
        (low + 2..=13).prop_map(move |high| Meld::run(suit, Rank::new(low), Rank::new(high)))
    });
    prop_oneof![set, run]
}

/// A hand of at most 11 cards, the sizes that occur in play
fn game_hand() -> impl Strategy<Value = Hand> {
    proptest::sample::subsequence(deck(), 0..=11).prop_map(Hand::from_iter)
}

/// Modern rules, sometimes with an Oklahoma knock limit, so playouts
/// exercise upcard-capped (and, on an ace, gin-only) knocking
fn rules() -> impl Strategy<Value = Rules> {
    prop_oneof![
        Just(None),
        Just(Some(OklahomaAce::One)),
        Just(Some(OklahomaAce::GinOnly))
    ]
    .prop_map(|oklahoma| {
        let mut rules = Rules::default();
        rules.oklahoma = oklahoma;
        rules
    })
}

/// An independent reference solver: enumerate melds by card arithmetic and
/// exhaustively pick disjoint subsets
fn naive_deadwood(hand: Hand) -> u16 {
    fn recurse(hand: Hand, melds: &[Hand]) -> u16 {
        let stop: u16 = hand
            .iter()
            .map(|card| u16::from(card.rank.deadwood()))
            .sum();
        melds
            .iter()
            .enumerate()
            .filter(|&(_, &meld)| meld & hand == meld)
            .map(|(i, &meld)| recurse(hand - meld, &melds[i..]))
            .fold(stop, u16::min)
    }

    let mut melds: Vec<Hand> = Vec::new();
    for rank in (1..=13).map(Rank::new) {
        let quad: Vec<Card> = Suit::ASC
            .into_iter()
            .map(|suit| Card { suit, rank })
            .collect();
        melds.push(quad.iter().copied().collect());
        for skip in 0..4 {
            melds.push(
                quad.iter()
                    .enumerate()
                    .filter_map(|(i, &card)| (i != skip).then_some(card))
                    .collect(),
            );
        }
    }
    for suit in Suit::ASC {
        for low in 1..=11u8 {
            for high in low + 2..=13 {
                melds.push(
                    (low..=high)
                        .map(|rank| Card {
                            suit,
                            rank: Rank::new(rank),
                        })
                        .collect(),
                );
            }
        }
    }

    recurse(hand, &melds)
}

proptest! {
    #[test]
    fn rank_roundtrip(rank in rank()) {
        prop_assert_eq!(rank.to_string().parse(), Ok(rank));
    }

    #[test]
    fn suit_roundtrip(suit in suit()) {
        prop_assert_eq!(suit.to_string().parse(), Ok(suit));
        prop_assert_eq!(suit.letter().to_string().parse(), Ok(suit));
    }

    #[test]
    fn card_roundtrip(card in card()) {
        prop_assert_eq!(card.to_string().parse(), Ok(card));
    }

    #[test]
    fn holding_roundtrip(holding in holding()) {
        prop_assert_eq!(holding.to_string().parse(), Ok(holding));
    }

    #[test]
    fn hand_roundtrip(hand in hand()) {
        prop_assert_eq!(hand.to_string().parse(), Ok(hand));
    }

    #[test]
    fn hand_iteration_matches_membership(hand in hand()) {
        let mut count = 0;
        let mut prev: Option<Card> = None;

        for current in hand {
            prop_assert!(hand.contains(current));
            if let Some(prev) = prev {
                let ascending = prev.suit < current.suit
                    || (prev.suit == current.suit && prev.rank < current.rank);
                prop_assert!(ascending);
            }
            prev = Some(current);
            count += 1;
        }

        prop_assert_eq!(count, hand.len());
        prop_assert_eq!(hand.iter().collect::<Hand>(), hand);
    }

    #[test]
    fn hand_bits_roundtrip(hand in hand()) {
        prop_assert_eq!(Hand::from_bits(hand.to_bits()), Some(hand));
        prop_assert!(!hand.contains_unknown_bits());
    }

    #[test]
    fn hand_set_algebra(a in hand(), b in hand()) {
        prop_assert_eq!(a - b, a & !b);
        prop_assert_eq!((a | b).len() + (a & b).len(), a.len() + b.len());
        prop_assert_eq!(a ^ b, (a | b) - (a & b));
    }

    #[test]
    fn meld_roundtrip(meld in meld()) {
        prop_assert_eq!(meld.to_string().parse(), Ok(meld));
        prop_assert_eq!(Meld::try_from_cards(meld.cards()), Ok(meld));
    }

    #[test]
    fn solver_matches_brute_force(hand in game_hand()) {
        prop_assert_eq!(u16::from(deadwood(hand)), naive_deadwood(hand));
    }

    #[test]
    fn best_melds_invariants(hand in game_hand(), card in card()) {
        let melds = best_melds(hand);
        prop_assert_eq!(melds.hand(), hand);
        prop_assert_eq!(melds.deadwood(), deadwood(hand));
        prop_assert_eq!(melds.melded() | melds.deadwood_cards(), hand);
        prop_assert!((melds.melded() & melds.deadwood_cards()).is_empty());

        let mut union = Hand::EMPTY;
        for meld in melds.iter() {
            prop_assert_eq!(Meld::try_from_cards(meld.cards()), Ok(meld));
            prop_assert_eq!(meld.cards() & hand, meld.cards());
            prop_assert!((union & meld.cards()).is_empty());
            union |= meld.cards();
        }
        prop_assert_eq!(union, melds.melded());

        let mut bigger = hand;
        if bigger.insert(card) && bigger.len() <= 11 {
            prop_assert!(deadwood(bigger) <= deadwood(hand) + card.rank.deadwood());
        }
    }

    #[test]
    fn random_playouts_stay_consistent(
        rules in rules(),
        order in Just(deck()).prop_shuffle(),
        seeds in proptest::collection::vec(any::<u8>(), 120),
    ) {
        let hands = [
            order[..10].iter().copied().collect(),
            order[10..20].iter().copied().collect(),
        ];
        let round = Round::from_deal(rules, Player::One, hands, order[20], order[21..].to_vec());
        let mut round = round.unwrap();
        prop_assert!(conserved(&round));

        let mut seeds = seeds.into_iter();
        // Once the seeds run out, the driver defaults to stock draws and
        // knock-when-possible, which always terminates.
        for _ in 0..400 {
            if round.result().is_some() {
                break;
            }
            step(&mut round, seeds.next().unwrap_or(1));
            prop_assert!(conserved(&round));
        }

        let result = round.result();
        prop_assert!(result.is_some(), "playout did not terminate: {round:?}");
        match result.unwrap() {
            RoundResult::Dead => {
                prop_assert_eq!(round.knocker(), None);
                prop_assert_eq!(round.stock().len(), 2);
            }
            RoundResult::Knock { winner, margin } => {
                prop_assert_eq!(round.knocker(), Some(winner));
                // With undercut-on-tie, a knock win is strictly positive.
                prop_assert!(margin > 0);
            }
            RoundResult::Undercut { winner, .. } => {
                prop_assert_eq!(round.knocker(), Some(winner.opponent()));
            }
            RoundResult::Gin { winner, .. } | RoundResult::BigGin { winner, .. } => {
                prop_assert_eq!(round.knocker(), Some(winner));
                prop_assert_eq!(round.laid_off(), Hand::EMPTY);
            }
            _ => {}
        }
    }
}

/// Every card is in exactly one place: a hand, the stock, the pile, or laid
/// off onto the spread.
fn conserved(round: &Round) -> bool {
    let stock: Hand = round.stock().iter().copied().collect();
    let pile: Hand = round.discard_pile().iter().copied().collect();
    let union = round.hand(Player::One) | round.hand(Player::Two) | stock | pile | round.laid_off();
    let count = round.hand(Player::One).len()
        + round.hand(Player::Two).len()
        + round.stock().len()
        + round.discard_pile().len()
        + round.laid_off().len();
    union == Hand::ALL && count == 52
}

/// Advance the round by one action chosen from the legal set by `seed`.
fn step(round: &mut Round, seed: u8) {
    let turn = round.turn().expect("an unfinished round has a turn");
    let hand = round.hand(turn);

    match round.phase() {
        Phase::Upcard => {
            if seed.is_multiple_of(2) {
                round.pass().unwrap();
            } else {
                round.take_discard().unwrap();
            }
        }
        Phase::Draw => {
            // Prefer the stock; sometimes probe the pile (which the forced
            // first draw legitimately refuses).
            if !seed.is_multiple_of(3) || round.take_discard().is_err() {
                round.draw_stock().unwrap();
            }
        }
        Phase::Discard => {
            if deadwood(hand) == 0 {
                round.declare_big_gin(best_melds(hand)).unwrap();
                return;
            }

            // The driver may not shed the card it just took, and it knows
            // which one that is without peeking: the pile top before a take
            // is the take itself, so re-probing legality is enough here.
            let candidates: Vec<Card> = hand.iter().collect();
            let knockable = candidates
                .iter()
                .map(|&card| (card, deadwood(hand - card.into())))
                .filter(|&(_, dw)| dw <= round.knock_limit())
                .min_by_key(|&(_, dw)| dw);

            if let Some((card, _)) = knockable
                && !seed.is_multiple_of(4)
                && round.knock(card, best_melds(hand - card.into())).is_ok()
            {
                return;
            }

            let start = seed as usize % candidates.len();
            for offset in 0..candidates.len() {
                let card = candidates[(start + offset) % candidates.len()];
                if round.discard(card).is_ok() {
                    return;
                }
            }
            unreachable!("some discard is always legal");
        }
        Phase::Layoff => {
            if !seed.is_multiple_of(3) {
                for card in hand {
                    for index in 0..3 {
                        if round.lay_off(card, index).is_ok() {
                            return;
                        }
                    }
                }
            }
            round.finish_layoffs().unwrap();
        }
        Phase::Finished => unreachable!("the driver stops on finished rounds"),
    }
}
