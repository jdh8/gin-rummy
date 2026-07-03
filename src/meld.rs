//! Melds, arrangements, and the exact deadwood solver.
//!
//! A [`Meld`] is a *set* (3-4 cards of one rank) or a *run* (3+ consecutive
//! cards of one suit).  A [`Melds`] is one chosen arrangement of a hand into
//! at most three disjoint melds plus deadwood.  [`best_melds`] and
//! [`deadwood`] solve the optimization gin rummy revolves around: splitting
//! a hand into disjoint melds that minimize the pip value of the leftovers.
//!
//! The solver follows the approach popularized by Todd Neller's EAAI gin
//! rummy framework: all 329 possible melds (65 sets and 264 runs) are
//! precomputed as card bitsets at compile time, applicability is one bitwise
//! test per meld, and a branch-and-bound search over "lowest card is
//! deadwood or starts one of its melds" explores every maximal disjoint
//! packing.  Hands have at most 11 cards during play, so the search takes
//! microseconds.
//!
//! # Panic policy
//!
//! [`Meld::run`] panics on runs shorter than three cards and has
//! [`Meld::try_run`] for fallible construction.  [`best_melds`] panics on
//! hands of more than 11 cards, for which an arrangement of at most three
//! melds is not enough; [`deadwood`] and [`pip_sum`] accept any card set.

use crate::hand::ParseCardError;
use crate::{Card, Hand, Rank, Suit};
use core::fmt::{self, Write as _};
use core::str::FromStr;
use thiserror::Error;

/// The two shapes of a meld
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum MeldKind {
    /// 3 or 4 cards of the same rank
    Set,
    /// 3 or more consecutive cards of the same suit
    Run,
}

/// Error indicating that cards do not form a meld
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum InvalidMeld {
    /// A meld requires at least 3 cards
    #[error("A meld requires at least 3 cards")]
    TooFewCards,

    /// Cards of one suit do not form consecutive ranks
    #[error("Cards of one suit do not form consecutive ranks")]
    NotConsecutive,

    /// Cards of several suits form neither a set nor a run
    #[error("Cards of several suits form neither a set nor a run")]
    MixedCards,

    /// The card set contains bits outside the 52-card deck
    #[error("The card set contains bits outside the 52-card deck")]
    UnknownCards,
}

/// A validated meld: a card set that is entirely one set or one run
///
/// A meld is internally a [`Hand`], so meld membership tests are single
/// bitwise operations on the shared `u64` layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(
    feature = "serde",
    derive(serde_with::SerializeDisplay, serde_with::DeserializeFromStr)
)]
#[repr(transparent)]
pub struct Meld(Hand);

impl Meld {
    /// Create a set of the given rank, with all four suits (`None`) or the
    /// three suits other than `missing`
    #[must_use]
    #[inline]
    pub const fn set(rank: Rank, missing: Option<Suit>) -> Self {
        let bit = 1u64 << rank.get();
        let all = bit | bit << 16 | bit << 32 | bit << 48;
        Self(Hand::from_bits_retain(match missing {
            None => all,
            Some(suit) => all & !(0xFFFF << (16 * suit as u64)),
        }))
    }

    /// Create a run of the given suit spanning `low..=high`
    ///
    /// # Panics
    ///
    /// When the run would be shorter than 3 cards.  In const contexts, this
    /// is a compile-time error.
    #[must_use]
    #[inline]
    pub const fn run(suit: Suit, low: Rank, high: Rank) -> Self {
        match Self::try_run(suit, low, high) {
            Ok(run) => run,
            Err(_) => panic!("a run spans at least 3 consecutive ranks"),
        }
    }

    /// Try to create a run of the given suit spanning `low..=high`
    ///
    /// # Errors
    ///
    /// When the run would be shorter than 3 cards.
    #[inline]
    pub const fn try_run(suit: Suit, low: Rank, high: Rank) -> Result<Self, InvalidMeld> {
        if high.get() < low.get() + 2 {
            return Err(InvalidMeld::TooFewCards);
        }
        let holding = (1u64 << (high.get() + 1)) - (1 << low.get());
        Ok(Self(Hand::from_bits_retain(holding << (16 * suit as u64))))
    }

    /// Try to create a meld from a card set
    ///
    /// # Errors
    ///
    /// When the cards do not form exactly one set or one run.
    pub const fn try_from_cards(cards: Hand) -> Result<Self, InvalidMeld> {
        if cards.contains_unknown_bits() {
            return Err(InvalidMeld::UnknownCards);
        }
        if cards.len() < 3 {
            return Err(InvalidMeld::TooFewCards);
        }

        let bits = cards.to_bits();
        let lanes = [
            bits as u16,
            (bits >> 16) as u16,
            (bits >> 32) as u16,
            (bits >> 48) as u16,
        ];
        let union = lanes[0] | lanes[1] | lanes[2] | lanes[3];

        if union.count_ones() as usize == cards.len() {
            // The suits do not overlap in rank: one suit forms a run if its
            // ranks are consecutive, and several suits never meld together.
            if union != lanes[0] && union != lanes[1] && union != lanes[2] && union != lanes[3] {
                return Err(InvalidMeld::MixedCards);
            }
            if (union >> union.trailing_zeros()) + 1 != 1 << union.count_ones() {
                return Err(InvalidMeld::NotConsecutive);
            }
        } else if union.count_ones() != 1 {
            // Overlapping ranks must all be the same single rank.
            return Err(InvalidMeld::MixedCards);
        }

        Ok(Self(cards))
    }

    /// The shape of this meld
    #[must_use]
    #[inline]
    pub const fn kind(self) -> MeldKind {
        let bits = self.0.to_bits();
        let lanes = (bits & 0xFFFF != 0) as u8
            + (bits >> 16 & 0xFFFF != 0) as u8
            + (bits >> 32 & 0xFFFF != 0) as u8
            + (bits >> 48 != 0) as u8;
        if lanes == 1 {
            MeldKind::Run
        } else {
            MeldKind::Set
        }
    }

    /// The cards of this meld
    #[must_use]
    #[inline]
    pub const fn cards(self) -> Hand {
        self.0
    }

    /// The number of cards in this meld, from 3 to 13
    #[allow(clippy::len_without_is_empty)] // a meld is never empty
    #[must_use]
    #[inline]
    pub const fn len(self) -> usize {
        self.0.len()
    }

    /// The suit of a run, or `None` for a set
    #[must_use]
    pub const fn suit(self) -> Option<Suit> {
        match self.kind() {
            MeldKind::Set => None,
            MeldKind::Run => Some(Suit::ASC[self.0.to_bits().trailing_zeros() as usize / 16]),
        }
    }

    /// The rank of a set, or `None` for a run
    #[must_use]
    pub const fn rank(self) -> Option<Rank> {
        match self.kind() {
            // Truncation is exact: trailing_zeros % 16 is in 1..=13.
            MeldKind::Set => Some(Rank::new(self.0.to_bits().trailing_zeros() as u8 % 16)),
            MeldKind::Run => None,
        }
    }

    /// The lowest rank of a run, or `None` for a set
    #[must_use]
    pub const fn low(self) -> Option<Rank> {
        match self.kind() {
            MeldKind::Set => None,
            // Truncation is exact: trailing_zeros % 16 is in 1..=13.
            MeldKind::Run => Some(Rank::new(self.0.to_bits().trailing_zeros() as u8 % 16)),
        }
    }

    /// The highest rank of a run, or `None` for a set
    #[must_use]
    pub const fn high(self) -> Option<Rank> {
        match self.kind() {
            MeldKind::Set => None,
            // Truncation is exact: 63 - leading_zeros % 16 is in 1..=13.
            MeldKind::Run => Some(Rank::new(
                (63 - self.0.to_bits().leading_zeros() as u8) % 16,
            )),
        }
    }

    /// The meld extended by a card, or `None` if the card does not fit
    ///
    /// This is the layoff primitive: a card extends a 3-card set of its rank
    /// or prolongs a run of its suit at either end.  Chained layoffs work by
    /// extending the returned meld again.
    #[must_use]
    pub const fn extended(self, card: Card) -> Option<Self> {
        let bits = self.0.to_bits();
        let card = 1u64 << (16 * card.suit as u64 + card.rank.get() as u64);
        if bits & card != 0 {
            return None;
        }
        match Self::try_from_cards(Hand::from_bits_retain(bits | card)) {
            Ok(meld) => Some(meld),
            Err(_) => None,
        }
    }
}

impl TryFrom<Hand> for Meld {
    type Error = InvalidMeld;

    #[inline]
    fn try_from(cards: Hand) -> Result<Self, InvalidMeld> {
        Self::try_from_cards(cards)
    }
}

impl From<Meld> for Hand {
    #[inline]
    fn from(meld: Meld) -> Self {
        meld.cards()
    }
}

/// Concatenated cards in ascending order, e.g. `♠5♠6♠7`
impl fmt::Display for Meld {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.iter().try_for_each(|card| write!(f, "{card}"))
    }
}

/// Error returned when parsing a [`Meld`] fails
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ParseMeldError {
    /// Error in a card
    #[error(transparent)]
    Card(#[from] ParseCardError),

    /// The same card appears more than once
    #[error("The same card appears more than once")]
    RepeatedCard,

    /// The cards do not form a meld
    #[error(transparent)]
    Invalid(#[from] InvalidMeld),
}

impl FromStr for Meld {
    type Err = ParseMeldError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        const fn is_suit_char(c: char) -> bool {
            matches!(
                c.to_ascii_uppercase(),
                'C' | 'D' | 'H' | 'S' | '♣' | '♦' | '♥' | '♠' | '♧' | '♢' | '♡' | '♤'
            )
        }

        let mut cards = Hand::EMPTY;
        let mut rest = s.trim_ascii_start();

        while !rest.is_empty() {
            let mut split = rest.char_indices().skip(1);
            let end = split
                .find_map(|(i, c)| is_suit_char(c).then_some(i))
                .unwrap_or(rest.len());
            let card: Card = rest[..end].trim_ascii_end().parse()?;
            if !cards.insert(card) {
                return Err(ParseMeldError::RepeatedCard);
            }
            rest = &rest[end..];
        }

        Ok(Self::try_from_cards(cards)?)
    }
}

/// Error indicating an invalid arrangement of a hand into melds
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ArrangeError {
    /// An arrangement covers a game hand of at most 11 cards
    #[error("An arrangement covers a game hand of at most 11 cards")]
    TooManyCards,

    /// At most 3 disjoint melds fit in 11 cards
    #[error("At most 3 disjoint melds fit in 11 cards")]
    TooManyMelds,

    /// Two melds share a card
    #[error("Two melds share a card")]
    OverlappingMelds,

    /// A meld contains a card outside the hand
    #[error("A meld contains a card outside the hand")]
    MeldNotInHand,
}

/// One arrangement of a hand into disjoint melds plus deadwood
///
/// This is what a knocker spreads on the table.  The arrangement fixes which
/// cards are melded — and therefore what the opponent may lay off — so a
/// knocker may legitimately choose a non-optimal arrangement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Melds {
    melds: [Option<Meld>; 3],
    hand: Hand,
}

impl Melds {
    /// Arrange a hand into the given melds
    ///
    /// # Errors
    ///
    /// When the hand exceeds 11 cards, the melds exceed 3, the melds
    /// overlap, or a meld is not contained in the hand.
    pub fn try_new(hand: Hand, melds: &[Meld]) -> Result<Self, ArrangeError> {
        if hand.len() > 11 {
            return Err(ArrangeError::TooManyCards);
        }
        if melds.len() > 3 {
            return Err(ArrangeError::TooManyMelds);
        }

        let mut union = Hand::EMPTY;
        let mut array = [None; 3];

        for (slot, &meld) in array.iter_mut().zip(melds) {
            if meld.cards() & hand != meld.cards() {
                return Err(ArrangeError::MeldNotInHand);
            }
            if !(union & meld.cards()).is_empty() {
                return Err(ArrangeError::OverlappingMelds);
            }
            union |= meld.cards();
            *slot = Some(meld);
        }

        Ok(Self { melds: array, hand })
    }

    /// Iterate over the melds of this arrangement
    #[inline]
    pub fn iter(self) -> impl Iterator<Item = Meld> {
        self.melds.into_iter().flatten()
    }

    /// The arranged hand
    #[must_use]
    #[inline]
    pub const fn hand(self) -> Hand {
        self.hand
    }

    /// The union of the melds
    #[must_use]
    pub fn melded(self) -> Hand {
        self.iter()
            .fold(Hand::EMPTY, |acc, meld| acc | meld.cards())
    }

    /// The unmelded cards
    #[must_use]
    pub fn deadwood_cards(self) -> Hand {
        self.hand - self.melded()
    }

    /// The deadwood value of the unmelded cards
    #[must_use]
    pub fn deadwood(self) -> u8 {
        // An arrangement holds at most 11 cards, worth at most 110 points.
        pip_sum(self.deadwood_cards()) as u8
    }

    pub(crate) const fn into_array(self) -> [Option<Meld>; 3] {
        self.melds
    }
}

/// Melds separated by spaces, then `|` and the deadwood cards if any,
/// e.g. `♥7♥8♥9 ♣Q♦Q♠Q | ♦A♦5`
///
/// This human-oriented format is informal and not parseable.
impl fmt::Display for Melds {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut leading = true;

        for meld in self.iter() {
            if !leading {
                f.write_char(' ')?;
            }
            write!(f, "{meld}")?;
            leading = false;
        }

        let deadwood = self.deadwood_cards();
        if !deadwood.is_empty() {
            if !leading {
                f.write_str(" | ")?;
            }
            deadwood.iter().try_for_each(|card| write!(f, "{card}"))?;
        }
        Ok(())
    }
}

/// The number of possible melds: 65 sets (5 per rank) and 264 runs (66 per
/// suit)
const MELD_COUNT: usize = 329;

/// All possible melds, precomputed at compile time
static MELDS: [Meld; MELD_COUNT] = build_melds();

const fn build_melds() -> [Meld; MELD_COUNT] {
    let mut table = [Meld::set(Rank::A, None); MELD_COUNT];
    let mut i = 0;

    let mut rank = 1;
    while rank <= 13 {
        table[i] = Meld::set(Rank::new(rank), None);
        i += 1;
        let mut suit = 0;
        while suit < 4 {
            table[i] = Meld::set(Rank::new(rank), Some(Suit::ASC[suit]));
            i += 1;
            suit += 1;
        }
        rank += 1;
    }

    let mut suit = 0;
    while suit < 4 {
        let mut low = 1;
        while low <= 11 {
            let mut high = low + 2;
            while high <= 13 {
                table[i] = Meld::run(Suit::ASC[suit], Rank::new(low), Rank::new(high));
                i += 1;
                high += 1;
            }
            low += 1;
        }
        suit += 1;
    }

    assert!(i == MELD_COUNT);
    table
}

/// The deadwood value of the lowest card of a non-empty bitset
const fn card_value(card: u64) -> u16 {
    let rank = card.trailing_zeros() as u16 % 16;
    if rank > 10 { 10 } else { rank }
}

/// The total deadwood value of a card set, melded or not
///
/// Cards outside the 52-card deck (possible only via
/// [`Hand::from_bits_retain`]) are ignored.
#[must_use]
pub const fn pip_sum(hand: Hand) -> u16 {
    let mut bits = hand.to_bits() & Hand::ALL.to_bits();
    let mut total = 0;
    while bits != 0 {
        total += card_value(bits);
        bits &= bits - 1;
    }
    total
}

/// Collect the applicable melds of a hand into `buf`, returning the count
fn applicable_melds(bits: u64, buf: &mut [u64; MELD_COUNT]) -> usize {
    let mut count = 0;
    for meld in &MELDS {
        let meld = meld.cards().to_bits();
        if meld & bits == meld {
            buf[count] = meld;
            count += 1;
        }
    }
    count
}

/// Branch and bound over "the lowest card is deadwood or in one of its
/// melds", the exhaustive search over maximal disjoint meld packings
fn search(hand: u64, melds: &[u64], acc: u16, tracker: &mut Tracker<'_>) {
    if acc >= tracker.best {
        return;
    }
    if hand == 0 {
        tracker.record(acc);
        return;
    }

    let card = hand & hand.wrapping_neg();
    if tracker.depth() < 3 {
        for &meld in melds {
            if meld & card != 0 && meld & hand == meld {
                tracker.push(meld);
                search(hand & !meld, melds, acc, tracker);
                tracker.pop();
            }
        }
    }
    search(hand & !card, melds, acc + card_value(card), tracker);
}

struct Tracker<'a> {
    best: u16,
    chosen: [u64; 3],
    count: usize,
    best_melds: Option<&'a mut [u64; 3]>,
}

impl Tracker<'_> {
    const fn depth(&self) -> usize {
        // Without a consumer of the chosen melds, the depth cap is
        // irrelevant: packings differing only in melds score alike.
        if self.best_melds.is_some() {
            self.count
        } else {
            0
        }
    }

    const fn push(&mut self, meld: u64) {
        if self.best_melds.is_some() {
            self.chosen[self.count] = meld;
        }
        self.count += 1;
    }

    const fn pop(&mut self) {
        self.count -= 1;
    }

    fn record(&mut self, acc: u16) {
        self.best = acc;
        if let Some(best_melds) = &mut self.best_melds {
            **best_melds = [0; 3];
            best_melds[..self.count].copy_from_slice(&self.chosen[..self.count]);
        }
    }
}

/// The minimum deadwood value of a card set over all meld arrangements
///
/// Cards outside the 52-card deck (possible only via
/// [`Hand::from_bits_retain`]) are ignored.  Any number of cards is
/// accepted; hands beyond the 11 cards of gin rummy merely take longer.
#[must_use]
pub fn deadwood(hand: Hand) -> u8 {
    let bits = hand.to_bits() & Hand::ALL.to_bits();
    let mut buf = [0; MELD_COUNT];
    let count = applicable_melds(bits, &mut buf);
    let mut tracker = Tracker {
        best: u16::MAX,
        chosen: [0; 3],
        count: 0,
        best_melds: None,
    };
    search(bits, &buf[..count], 0, &mut tracker);

    // An optimal remainder is meld-free — melding a leftover meld would only
    // shrink the deadwood — and the most valuable meld-free card set is worth
    // 170 points (two suits of A 3 4 6 7 9 T Q K), so the minimum fits u8.
    tracker.best as u8
}

/// A best arrangement of a hand: disjoint melds minimizing deadwood
///
/// Ties between optimal arrangements are broken deterministically, but which
/// arrangement wins is unspecified and may change between releases.  Cards
/// outside the 52-card deck (possible only via [`Hand::from_bits_retain`])
/// are ignored.
///
/// # Panics
///
/// When the hand has more than 11 cards, for which an arrangement of at most
/// three melds is not enough.  Use [`deadwood`] for arbitrary card sets.
#[must_use]
pub fn best_melds(hand: Hand) -> Melds {
    let hand = Hand::from_bits_truncate(hand.to_bits());
    assert!(
        hand.len() <= 11,
        "best_melds arranges game hands of at most 11 cards"
    );

    let mut buf = [0; MELD_COUNT];
    let count = applicable_melds(hand.to_bits(), &mut buf);
    let mut best_melds = [0; 3];
    let mut tracker = Tracker {
        best: u16::MAX,
        chosen: [0; 3],
        count: 0,
        best_melds: Some(&mut best_melds),
    };
    search(hand.to_bits(), &buf[..count], 0, &mut tracker);

    let mut melds = [None; 3];
    for (slot, &bits) in melds.iter_mut().zip(&best_melds) {
        if bits != 0 {
            *slot = Some(Meld(Hand::from_bits_retain(bits)));
        }
    }
    Melds { melds, hand }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_is_sound() {
        assert_eq!(MELDS.len(), 329);

        for (i, meld) in MELDS.iter().enumerate() {
            assert_eq!(Meld::try_from_cards(meld.cards()), Ok(*meld));
            for other in &MELDS[..i] {
                assert_ne!(meld, other);
            }
        }

        let sets = MELDS.iter().filter(|m| m.kind() == MeldKind::Set).count();
        assert_eq!(sets, 65);
    }

    #[test]
    fn meld_constructors() {
        let set = Meld::set(Rank::Q, None);
        assert_eq!(set.kind(), MeldKind::Set);
        assert_eq!(set.len(), 4);
        assert_eq!(set.rank(), Some(Rank::Q));
        assert_eq!((set.suit(), set.low(), set.high()), (None, None, None));

        let set = Meld::set(Rank::A, Some(Suit::Hearts));
        assert_eq!(set.len(), 3);
        assert!(!set.cards().contains("♥A".parse().unwrap()));

        let run = Meld::run(Suit::Spades, Rank::A, Rank::new(3));
        assert_eq!(run.kind(), MeldKind::Run);
        assert_eq!(run.len(), 3);
        assert_eq!(run.suit(), Some(Suit::Spades));
        assert_eq!(run.low(), Some(Rank::A));
        assert_eq!(run.high(), Some(Rank::new(3)));
        assert_eq!(run.rank(), None);

        assert_eq!(
            Meld::try_run(Suit::Clubs, Rank::A, Rank::new(2)),
            Err(InvalidMeld::TooFewCards),
        );

        let all_spades = Meld::run(Suit::Spades, Rank::A, Rank::K);
        assert_eq!(all_spades.len(), 13);
    }

    #[test]
    fn try_from_cards_rejects_non_melds() {
        let parse = |s: &str| Meld::try_from_cards(s.parse::<Hand>().unwrap());

        assert!(parse("567...").is_ok());
        assert!(parse("7.7.7.").is_ok());
        assert!(parse("7.7.7.7").is_ok());
        assert_eq!(parse("57.5.."), Err(InvalidMeld::MixedCards));
        assert_eq!(parse("567.8.."), Err(InvalidMeld::MixedCards));
        assert_eq!(parse("579..."), Err(InvalidMeld::NotConsecutive));
        assert_eq!(parse("56..."), Err(InvalidMeld::TooFewCards));
        assert_eq!(parse("..."), Err(InvalidMeld::TooFewCards));
        assert_eq!(
            Meld::try_from_cards(Hand::from_bits_retain(7 << 14)),
            Err(InvalidMeld::UnknownCards),
        );
    }

    #[test]
    fn extension() {
        let run = Meld::run(Suit::Spades, Rank::new(5), Rank::new(7));
        let extended = run.extended("♠8".parse().unwrap()).unwrap();
        assert_eq!(extended.high(), Some(Rank::new(8)));
        let chained = extended.extended("♠9".parse().unwrap()).unwrap();
        assert_eq!(chained.high(), Some(Rank::new(9)));
        let low_end = run.extended("♠4".parse().unwrap()).unwrap();
        assert_eq!(low_end.low(), Some(Rank::new(4)));

        assert_eq!(run.extended("♠9".parse().unwrap()), None);
        assert_eq!(run.extended("♥8".parse().unwrap()), None);
        assert_eq!(run.extended("♠6".parse().unwrap()), None);

        let low_run = Meld::run(Suit::Clubs, Rank::A, Rank::new(3));
        let high_run = Meld::run(Suit::Clubs, Rank::J, Rank::K);
        assert_eq!(low_run.extended("♣K".parse().unwrap()), None);
        assert_eq!(high_run.extended("♣A".parse().unwrap()), None);

        let set = Meld::set(Rank::Q, Some(Suit::Diamonds));
        let full = set.extended("♦Q".parse().unwrap()).unwrap();
        assert_eq!(full.len(), 4);
        assert_eq!(full.extended("♦Q".parse().unwrap()), None);
        assert_eq!(set.extended("♦J".parse().unwrap()), None);
    }

    #[test]
    fn melds_arrangement() {
        let hand: Hand = "A23.456.789.T".parse().unwrap();
        let runs = [
            Meld::run(Suit::Clubs, Rank::A, Rank::new(3)),
            Meld::run(Suit::Diamonds, Rank::new(4), Rank::new(6)),
            Meld::run(Suit::Hearts, Rank::new(7), Rank::new(9)),
        ];

        let melds = Melds::try_new(hand, &runs).unwrap();
        assert_eq!(melds.iter().count(), 3);
        assert_eq!(melds.deadwood_cards(), "...T".parse().unwrap());
        assert_eq!(melds.deadwood(), 10);
        assert_eq!(melds.to_string(), "♣A♣2♣3 ♦4♦5♦6 ♥7♥8♥9 | ♠T");

        assert_eq!(
            Melds::try_new(hand, &runs[1..]).map(|m| m.deadwood()),
            Ok(16),
        );
        assert_eq!(
            Melds::try_new("A23...".parse().unwrap(), &runs[..1]).map(|m| m.deadwood()),
            Ok(0),
        );

        assert_eq!(
            Melds::try_new(hand, &[runs[0], runs[0]]),
            Err(ArrangeError::OverlappingMelds),
        );
        assert_eq!(
            Melds::try_new(Hand::EMPTY, &runs[..1]),
            Err(ArrangeError::MeldNotInHand),
        );
        assert_eq!(
            Melds::try_new(Hand::ALL, &runs),
            Err(ArrangeError::TooManyCards),
        );

        let two_runs: Hand = "A23456...".parse().unwrap();
        let split = [
            Meld::run(Suit::Clubs, Rank::A, Rank::new(3)),
            Meld::run(Suit::Clubs, Rank::new(4), Rank::new(6)),
        ];
        assert_eq!(
            Melds::try_new(two_runs, &split).map(|m| m.deadwood()),
            Ok(0)
        );
    }

    #[test]
    fn meld_parsing() {
        let run: Meld = "♠5♠6♠7".parse().unwrap();
        assert_eq!(run, Meld::run(Suit::Spades, Rank::new(5), Rank::new(7)));
        assert_eq!(run.to_string(), "♠5♠6♠7");
        assert_eq!("S5 S6 S7".parse(), Ok(run));
        assert_eq!("s5s6s7".parse(), Ok(run));

        let set: Meld = "♣7♦7♠7".parse().unwrap();
        assert_eq!(set, Meld::set(Rank::new(7), Some(Suit::Hearts)));

        let tens: Meld = "♣10♦10♥10♠10".parse().unwrap();
        assert_eq!(tens, Meld::set(Rank::T, None));

        assert_eq!(
            "♠5♠6".parse::<Meld>(),
            Err(ParseMeldError::Invalid(InvalidMeld::TooFewCards)),
        );
        assert_eq!(
            "♠5♠5♠6♠7".parse::<Meld>(),
            Err(ParseMeldError::RepeatedCard),
        );
        assert_eq!(
            "♠5♠6♥7".parse::<Meld>(),
            Err(ParseMeldError::Invalid(InvalidMeld::MixedCards)),
        );
        assert!(matches!(
            "5♠6♠7".parse::<Meld>(),
            Err(ParseMeldError::Card(_)),
        ));
        assert!(matches!(
            "".parse::<Meld>(),
            Err(ParseMeldError::Invalid(_))
        ));
    }

    #[test]
    fn deadwood_basics() {
        let gin: Hand = "A23.456.789.T".parse().unwrap();
        assert_eq!(deadwood(gin), 10);
        assert_eq!(best_melds(gin).deadwood(), 10);

        assert_eq!(deadwood(Hand::EMPTY), 0);
        assert_eq!(deadwood(Hand::ALL), 0);
        assert_eq!(pip_sum(Hand::ALL), 340);
        assert_eq!(pip_sum(gin), 55);
    }
}
