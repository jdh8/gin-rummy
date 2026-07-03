//! Random card shuffling and dealing.
//!
//! [`Deck`] is a 52-card-bounded set backed by [`Hand`]; cards can be
//! inserted and drawn (uniformly at random) without materializing the full
//! card set.  [`Round::deal`](crate::Round::deal) and
//! [`Game::deal`](crate::Game::deal) build on it to start rounds from a
//! shuffled deck.
//!
//! This module is gated behind the `rand` feature.

use crate::{Card, Hand};
use core::fmt;
use core::str::FromStr;
use rand::{Rng, RngExt as _};

/// A subset of the standard 52-card deck
///
/// This is a set of unique cards backed by [`Hand`].  Duplicates are
/// structurally impossible.  It requires shuffling to partially retrieve
/// cards from the deck.  However, it is deterministic to collect all cards.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde_with::SerializeDisplay, serde_with::DeserializeFromStr)
)]
pub struct Deck(Hand);

impl Deck {
    /// The standard 52-card deck
    pub const ALL: Self = Self(Hand::ALL);

    /// An empty deck
    pub const EMPTY: Self = Self(Hand::EMPTY);

    /// The number of cards currently in the deck
    #[must_use]
    #[inline]
    pub const fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether the deck is empty
    #[must_use]
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Clear the deck, removing all the cards.
    pub const fn clear(&mut self) {
        self.0 = Hand::EMPTY;
    }

    /// Insert a card into the deck
    ///
    /// Returns `true` if the card was newly inserted, `false` if it was
    /// already present.
    pub fn insert(&mut self, card: Card) -> bool {
        self.0.insert(card)
    }

    /// Take the remaining cards in the deck into a hand.
    #[must_use]
    #[inline]
    pub const fn take(&mut self) -> Hand {
        core::mem::replace(&mut self.0, Hand::EMPTY)
    }

    /// Randomly draw `n` cards from the deck and collect them into a hand.
    ///
    /// If `n >= self.len()`, all remaining cards are drawn without
    /// shuffling.
    ///
    /// On each iteration, pick a uniform `k` in `0..remaining`, then strip
    /// the `k` lowest set bits from `self.0`.  The new lowest set bit is the
    /// `k`-th smallest card, which is moved from the deck to the hand.  This
    /// performs `n` selections without materializing the card set.
    #[must_use]
    pub fn draw(&mut self, rng: &mut (impl Rng + ?Sized), n: usize) -> Hand {
        let len = self.0.len();
        if n >= len {
            return self.take();
        }

        let mut hand = Hand::EMPTY;
        for i in 0..n {
            let bits = (0..rng.random_range(..len - i))
                .fold(self.0.to_bits(), |bits, _| bits & (bits - 1));
            let selected = Hand::from_bits_retain(bits & bits.wrapping_neg());
            hand |= selected;
            self.0 ^= selected;
        }
        hand
    }

    /// Randomly pop a card from the deck
    #[must_use]
    pub fn pop(&mut self, rng: &mut (impl Rng + ?Sized)) -> Option<Card> {
        self.draw(rng, 1).into_iter().next()
    }
}

impl From<Hand> for Deck {
    fn from(hand: Hand) -> Self {
        Self(hand)
    }
}

impl From<Deck> for Hand {
    fn from(deck: Deck) -> Self {
        deck.0
    }
}

impl fmt::Display for Deck {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for Deck {
    type Err = <Hand as FromStr>::Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse::<Hand>().map(Self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tiny deterministic generator, good enough to exercise the paths
    struct Lcg(u64);

    impl Lcg {
        fn step(&mut self) -> u32 {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (self.0 >> 32) as u32
        }
    }

    impl rand::rand_core::TryRng for Lcg {
        type Error = core::convert::Infallible;

        fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
            Ok(self.step())
        }

        fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
            Ok(u64::from(self.step()) << 32 | u64::from(self.step()))
        }

        fn try_fill_bytes(&mut self, dst: &mut [u8]) -> Result<(), Self::Error> {
            dst.fill_with(|| self.step() as u8);
            Ok(())
        }
    }

    #[test]
    fn drawing_partitions_the_deck() {
        let mut rng = Lcg(2026);
        let mut deck = Deck::ALL;

        let first = deck.draw(&mut rng, 10);
        let second = deck.draw(&mut rng, 10);
        let upcard = deck.pop(&mut rng).unwrap();

        assert_eq!(first.len(), 10);
        assert_eq!(second.len(), 10);
        assert_eq!(deck.len(), 31);
        assert!((first & second).is_empty());
        assert!(!first.contains(upcard) && !second.contains(upcard));

        let rest = deck.take();
        assert!(deck.is_empty());
        assert_eq!(first | second | Hand::from(upcard) | rest, Hand::ALL);

        assert_eq!(deck.pop(&mut rng), None);
        assert!(deck.insert(upcard));
        assert!(!deck.insert(upcard));
        deck.clear();
        assert_eq!(deck, Deck::EMPTY);
    }
}
