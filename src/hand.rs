//! Card primitives: ranks, cards, holdings, and hands.
//!
//! [`Rank`] is a non-zero byte in `1..=13` with the ace LOW (A/J/Q/K as
//! 1/11/12/13), the ordering of gin rummy.  [`Card`] pairs a [`Rank`] with a
//! [`Suit`].  [`Holding`] is a 13-bit bitset of ranks within one suit, and
//! [`Hand`] is four holdings packed into a `u64` with a documented layout:
//! the card of suit `s` and rank `r` is bit `16 × s + r`.  Set operations on
//! holdings and hands are exposed through the standard bitwise operators.
//!
//! Iteration yields cards in ascending suit order (clubs first) and
//! ascending rank order within each suit, matching runs like A-2-3 and the
//! display order of this crate.
//!
//! # Panic policy
//!
//! [`Rank::new`] panics when the input is outside `1..=13` and has
//! [`Rank::try_new`] for fallible construction.  In const contexts the panic
//! becomes a compile-time error.

use crate::Suit;
use core::fmt::{self, Write as _};
use core::iter::FusedIterator;
use core::num::NonZero;
use core::ops;
use core::str::FromStr;
use thiserror::Error;

/// Error indicating an invalid rank
///
/// The rank of a card must be in `1..=13`, where A, J, Q, K are denoted as 1,
/// 11, 12, 13 respectively.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[error("{0} is not a valid rank (1..=13)")]
pub struct InvalidRank(pub u8);

/// The rank of a card, from 1 to 13, where A, J, Q, K are internally denoted
/// as 1, 11, 12, 13 respectively
///
/// The ace is LOW in gin rummy: A-2-3 is a run, Q-K-A is not, and the
/// derived ordering reflects that (`Rank::A < Rank::new(2)`).
///
/// With the `serde` feature, a rank serializes as its number, and
/// deserialization validates the range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(try_from = "u8", into = "u8")
)]
#[repr(transparent)]
pub struct Rank(NonZero<u8>);

impl Rank {
    /// Ace, the lowest rank
    pub const A: Self = Self(NonZero::new(1).unwrap());

    /// Ten
    pub const T: Self = Self(NonZero::new(10).unwrap());

    /// Jack
    pub const J: Self = Self(NonZero::new(11).unwrap());

    /// Queen
    pub const Q: Self = Self(NonZero::new(12).unwrap());

    /// King, the highest rank
    pub const K: Self = Self(NonZero::new(13).unwrap());

    /// Create a rank from a number
    ///
    /// # Panics
    ///
    /// When the rank is not in `1..=13`.  In const contexts, this is a
    /// compile-time error.
    #[must_use]
    #[inline]
    pub const fn new(rank: u8) -> Self {
        match Self::try_new(rank) {
            Ok(r) => r,
            Err(_) => panic!("rank must be in 1..=13"),
        }
    }

    /// Try to create a rank from a number
    ///
    /// # Errors
    ///
    /// When the rank is not in `1..=13`.
    #[inline]
    pub const fn try_new(rank: u8) -> Result<Self, InvalidRank> {
        match NonZero::new(rank) {
            Some(nonzero) if rank <= 13 => Ok(Self(nonzero)),
            _ => Err(InvalidRank(rank)),
        }
    }

    /// Get the stored rank as [`u8`]
    #[must_use]
    #[inline]
    pub const fn get(self) -> u8 {
        self.0.get()
    }

    /// Display character for this rank
    #[must_use]
    #[inline]
    pub const fn letter(self) -> char {
        b"A23456789TJQK"[self.get() as usize - 1] as char
    }

    /// The deadwood value of this rank: aces count 1, spot cards their pip
    /// value, and tens and court cards 10
    #[must_use]
    #[inline]
    pub const fn deadwood(self) -> u8 {
        if self.get() > 10 { 10 } else { self.get() }
    }
}

impl From<Rank> for u8 {
    #[inline]
    fn from(rank: Rank) -> Self {
        rank.get()
    }
}

impl TryFrom<u8> for Rank {
    type Error = InvalidRank;

    #[inline]
    fn try_from(rank: u8) -> Result<Self, InvalidRank> {
        Self::try_new(rank)
    }
}

impl fmt::Display for Rank {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_char(self.letter())
    }
}

/// Error returned when parsing a [`Rank`] fails
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
#[error("Invalid rank: expected A, 2-10, T, J, Q, K")]
pub struct ParseRankError;

impl FromStr for Rank {
    type Err = ParseRankError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_uppercase().as_str() {
            "A" => Ok(Self::A),
            "K" => Ok(Self::K),
            "Q" => Ok(Self::Q),
            "J" => Ok(Self::J),
            "T" | "10" => Ok(Self::T),
            "9" => Ok(Self::new(9)),
            "8" => Ok(Self::new(8)),
            "7" => Ok(Self::new(7)),
            "6" => Ok(Self::new(6)),
            "5" => Ok(Self::new(5)),
            "4" => Ok(Self::new(4)),
            "3" => Ok(Self::new(3)),
            "2" => Ok(Self::new(2)),
            _ => Err(ParseRankError),
        }
    }
}

/// A playing card
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(
    feature = "serde",
    derive(serde_with::SerializeDisplay, serde_with::DeserializeFromStr)
)]
pub struct Card {
    /// The suit of the card
    pub suit: Suit,
    /// The rank of the card
    pub rank: Rank,
}

impl fmt::Display for Card {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}", self.suit, self.rank)
    }
}

/// Error returned when parsing a [`Card`] fails
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ParseCardError {
    /// Invalid suit in card
    #[error("Invalid suit in card: expected one of C, D, H, S, ♣, ♦, ♥, ♠, ♧, ♢, ♡, ♤")]
    Suit,
    /// Invalid rank in card
    #[error("Invalid rank in card: expected A, 2-10, T, J, Q, K")]
    Rank,
}

impl FromStr for Card {
    type Err = ParseCardError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let rank_len = if s.ends_with("10") {
            2
        } else {
            s.chars().next_back().map_or(0, char::len_utf8)
        };
        let border = s.len().saturating_sub(rank_len);
        let (suit, rank) = s.split_at(border);
        let suit: Suit = suit.parse().map_err(|_| ParseCardError::Suit)?;
        let rank: Rank = rank.parse().map_err(|_| ParseCardError::Rank)?;
        Ok(Self { suit, rank })
    }
}

/// A set of cards of the same suit
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[cfg_attr(
    feature = "serde",
    derive(serde_with::SerializeDisplay, serde_with::DeserializeFromStr)
)]
#[repr(transparent)]
pub struct Holding(u16);

/// Iterator over the ranks in a [`Holding`], yielding [`Rank`]s in ascending
/// order
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HoldingIter {
    rest: u16,
    cursor: u8,
}

impl Iterator for HoldingIter {
    type Item = Rank;

    fn next(&mut self) -> Option<Self::Item> {
        if self.rest == 0 {
            return None;
        }

        // 1. Trailing zeros are in the range of 0..=15, which fits in `u8`
        // 2. Trailing zeros cannot be 15 since `iter` masks to valid ranks
        let step = self.rest.trailing_zeros() as u8 + 1;
        self.rest >>= step;
        self.cursor += step;

        // SAFETY: cursor is in 1..=13 since `iter` masks to valid ranks
        Some(Rank(unsafe { NonZero::new_unchecked(self.cursor - 1) }))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let count = self.rest.count_ones() as usize;
        (count, Some(count))
    }

    fn count(self) -> usize {
        self.rest.count_ones() as usize
    }
}

impl DoubleEndedIterator for HoldingIter {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.rest == 0 {
            return None;
        }

        // For non-zero u16, leading_zeros is in 0..=15, fitting in u8.
        let pos = 15 - self.rest.leading_zeros() as u8;
        self.rest &= !(1u16 << pos);

        // SAFETY: cursor + pos is in 1..=13 since `iter` masks to valid ranks
        Some(Rank(unsafe { NonZero::new_unchecked(self.cursor + pos) }))
    }
}

impl ExactSizeIterator for HoldingIter {
    fn len(&self) -> usize {
        self.rest.count_ones() as usize
    }
}

impl FusedIterator for HoldingIter {}

impl Holding {
    /// The empty set
    pub const EMPTY: Self = Self(0);

    /// The set containing all possible ranks (1..=13)
    pub const ALL: Self = Self(0x3FFE);

    /// The number of cards in the holding
    #[must_use]
    #[inline]
    pub const fn len(self) -> usize {
        self.0.count_ones() as usize
    }

    /// Whether the holding is empty
    #[must_use]
    #[inline]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Whether the holding contains a rank
    #[must_use]
    #[inline]
    pub const fn contains(self, rank: Rank) -> bool {
        self.0 & 1 << rank.get() != 0
    }

    /// Insert a rank into the holding, returning whether it was newly inserted
    #[inline]
    pub const fn insert(&mut self, rank: Rank) -> bool {
        let insertion = 1 << rank.get() & Self::ALL.0;
        let inserted = insertion & !self.0 != 0;
        self.0 |= insertion;
        inserted
    }

    /// Remove a rank from the holding, returning whether it was present
    #[inline]
    pub const fn remove(&mut self, rank: Rank) -> bool {
        let removed = self.contains(rank);
        self.0 &= !(1 << rank.get());
        removed
    }

    /// Toggle a rank in the holding, returning whether it is now present
    #[inline]
    pub const fn toggle(&mut self, rank: Rank) -> bool {
        self.0 ^= 1 << rank.get() & Self::ALL.0;
        self.contains(rank)
    }

    /// Conditionally insert/remove a rank from the holding
    #[inline]
    pub const fn set(&mut self, rank: Rank, condition: bool) {
        let flag = 1 << rank.get();
        let mask = (condition as u16).wrapping_neg();
        self.0 = (self.0 & !flag) | (mask & flag);
    }

    /// Iterate over the ranks in the holding
    ///
    /// Ranks that would be invalid (bits outside [`Holding::ALL`], possible
    /// only via [`Holding::from_bits_retain`]) are skipped.
    #[inline]
    #[must_use]
    pub const fn iter(self) -> HoldingIter {
        HoldingIter {
            rest: self.0 & Self::ALL.0,
            cursor: 0,
        }
    }

    /// As a bitset of ranks
    #[must_use]
    #[inline]
    pub const fn to_bits(self) -> u16 {
        self.0
    }

    /// Create a holding from a bitset of ranks, retaining invalid ranks
    #[must_use]
    #[inline]
    pub const fn from_bits_retain(bits: u16) -> Self {
        Self(bits)
    }

    /// Whether the holding contains an invalid rank
    #[must_use]
    #[inline]
    pub const fn contains_unknown_bits(self) -> bool {
        self.0 & Self::ALL.0 != self.0
    }

    /// Create a holding from a bitset of ranks, checking for invalid ranks
    #[must_use]
    #[inline]
    pub const fn from_bits(bits: u16) -> Option<Self> {
        if bits & Self::ALL.0 == bits {
            Some(Self(bits))
        } else {
            None
        }
    }

    /// Create a holding from a bitset of ranks, removing invalid ranks
    #[must_use]
    #[inline]
    pub const fn from_bits_truncate(bits: u16) -> Self {
        Self(bits & Self::ALL.0)
    }

    /// Create a holding from a rank
    #[must_use]
    #[inline]
    pub const fn from_rank(rank: Rank) -> Self {
        Self(1 << rank.get())
    }
}

impl IntoIterator for Holding {
    type Item = Rank;
    type IntoIter = HoldingIter;

    fn into_iter(self) -> HoldingIter {
        self.iter()
    }
}

impl ops::BitAnd for Holding {
    type Output = Self;

    #[inline]
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}

impl ops::BitOr for Holding {
    type Output = Self;

    #[inline]
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl ops::BitXor for Holding {
    type Output = Self;

    #[inline]
    fn bitxor(self, rhs: Self) -> Self {
        Self(self.0 ^ rhs.0)
    }
}

impl ops::Not for Holding {
    type Output = Self;

    #[inline]
    fn not(self) -> Self {
        Self::from_bits_truncate(!self.0)
    }
}

impl ops::Sub for Holding {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Self(self.0 & !rhs.0)
    }
}

impl ops::BitAndAssign for Holding {
    #[inline]
    fn bitand_assign(&mut self, rhs: Self) {
        *self = *self & rhs;
    }
}

impl ops::BitOrAssign for Holding {
    #[inline]
    fn bitor_assign(&mut self, rhs: Self) {
        *self = *self | rhs;
    }
}

impl ops::BitXorAssign for Holding {
    #[inline]
    fn bitxor_assign(&mut self, rhs: Self) {
        *self = *self ^ rhs;
    }
}

impl ops::SubAssign for Holding {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

/// Show cards in ascending order
///
/// 1. The ten is shown as `T` for symmetry with parsing.
/// 2. This implementation ignores formatting flags for simplicity and speed.
///    If you want to pad or align the output, use [`fmt::Formatter::pad`].
impl fmt::Display for Holding {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for rank in 1u8..14 {
            if self.0 & 1 << rank != 0 {
                f.write_char(b"A23456789TJQK"[rank as usize - 1] as char)?;
            }
        }
        Ok(())
    }
}

/// An error which can be returned when parsing a [`Holding`]
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ParseHoldingError {
    /// Ranks are not all valid or in ascending order
    #[error("Ranks are not all valid or in ascending order")]
    InvalidRanks,

    /// The same rank appears more than once
    #[error("The same rank appears more than once")]
    RepeatedRank,

    /// A suit contains more than 13 cards
    #[error("A suit contains more than 13 cards")]
    TooManyCards,
}

/// An error which can be returned when parsing a [`Hand`]
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ParseHandError {
    /// Error in a holding
    #[error(transparent)]
    Holding(#[from] ParseHoldingError),

    /// The hand does not contain 4 suits
    #[error("The hand does not contain 4 suits")]
    NotFourSuits,
}

impl FromStr for Holding {
    type Err = ParseHoldingError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // 13 cards + 1 extra char for "10"
        if s.len() > 14 {
            return Err(ParseHoldingError::TooManyCards);
        }

        let bytes = s.as_bytes();
        let mut i = 0;
        let mut prev_rank: u8 = 0;
        let mut holding = Self::EMPTY;

        while i < bytes.len() {
            let c = bytes[i].to_ascii_uppercase();
            let rank: u8 = match c {
                b'A' => 1,
                b'K' => 13,
                b'Q' => 12,
                b'J' => 11,
                b'T' => 10,
                b'1' => {
                    if bytes.get(i + 1) != Some(&b'0') {
                        return Err(ParseHoldingError::InvalidRanks);
                    }
                    i += 1;
                    10
                }
                b'2'..=b'9' => c - b'0',
                _ => return Err(ParseHoldingError::InvalidRanks),
            };

            if rank == prev_rank {
                return Err(ParseHoldingError::RepeatedRank);
            }
            if rank < prev_rank {
                return Err(ParseHoldingError::InvalidRanks);
            }
            prev_rank = rank;

            // SAFETY: rank is in 1..=13 by construction above
            holding.insert(Rank(unsafe { NonZero::new_unchecked(rank) }));
            i += 1;
        }

        Ok(holding)
    }
}

/// A set of playing cards
///
/// Despite the name, this type is a general card set: it also represents
/// melds, deadwood, and the union of several hands throughout this crate.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[cfg_attr(
    feature = "serde",
    derive(serde_with::SerializeDisplay, serde_with::DeserializeFromStr)
)]
pub struct Hand([Holding; 4]);

const _: () = assert!(size_of::<Hand>() == 8);
const _: () = assert!(Hand::ALL.to_bits() == 0x3FFE_3FFE_3FFE_3FFE);

/// Iterator over the cards in a [`Hand`], yielding [`Card`]s in ascending
/// suit order (clubs first) and ascending rank order within each suit
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandIter {
    suits: [HoldingIter; 4],
    fwd: u8,
    bwd: u8,
}

impl Iterator for HandIter {
    type Item = Card;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.fwd > 3 {
                return None;
            }
            let suit = Suit::ASC[self.fwd as usize];
            if let Some(rank) = self.suits[self.fwd as usize].next() {
                return Some(Card { suit, rank });
            }
            if self.fwd == self.bwd {
                self.fwd = 4;
                return None;
            }
            self.fwd += 1;
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let count = self.len();
        (count, Some(count))
    }

    fn count(self) -> usize {
        self.len()
    }
}

impl DoubleEndedIterator for HandIter {
    fn next_back(&mut self) -> Option<Self::Item> {
        loop {
            if self.fwd > 3 {
                return None;
            }
            let suit = Suit::ASC[self.bwd as usize];
            if let Some(rank) = self.suits[self.bwd as usize].next_back() {
                return Some(Card { suit, rank });
            }
            if self.fwd == self.bwd {
                self.fwd = 4;
                return None;
            }
            self.bwd -= 1;
        }
    }
}

impl ExactSizeIterator for HandIter {
    fn len(&self) -> usize {
        if self.fwd > 3 {
            return 0;
        }
        (self.fwd as usize..=self.bwd as usize)
            .map(|i| self.suits[i].len())
            .sum()
    }
}

impl FusedIterator for HandIter {}

impl ops::Index<Suit> for Hand {
    type Output = Holding;

    #[inline]
    fn index(&self, suit: Suit) -> &Holding {
        &self.0[suit as usize]
    }
}

impl ops::IndexMut<Suit> for Hand {
    #[inline]
    fn index_mut(&mut self, suit: Suit) -> &mut Holding {
        &mut self.0[suit as usize]
    }
}

impl Hand {
    /// As a bitset of cards
    ///
    /// The card of suit `s` and rank `r` is bit `16 × s + r`, i.e. each suit
    /// owns a 16-bit lane in ascending suit order and a lane is the
    /// [`Holding::to_bits`] of that suit.  This layout is a stable API
    /// contract of this crate.
    #[must_use]
    #[inline]
    pub const fn to_bits(self) -> u64 {
        self.0[0].to_bits() as u64
            | (self.0[1].to_bits() as u64) << 16
            | (self.0[2].to_bits() as u64) << 32
            | (self.0[3].to_bits() as u64) << 48
    }

    /// Create a hand from a bitset of cards, retaining invalid cards
    // Truncating casts keep exactly the 16-bit lane of each suit.
    #[must_use]
    #[inline]
    pub const fn from_bits_retain(bits: u64) -> Self {
        Self([
            Holding::from_bits_retain(bits as u16),
            Holding::from_bits_retain((bits >> 16) as u16),
            Holding::from_bits_retain((bits >> 32) as u16),
            Holding::from_bits_retain((bits >> 48) as u16),
        ])
    }

    /// Whether the hand contains an invalid card
    #[must_use]
    #[inline]
    pub const fn contains_unknown_bits(self) -> bool {
        self.to_bits() & Self::ALL.to_bits() != self.to_bits()
    }

    /// Create a hand from a bitset of cards, checking for invalid cards
    #[must_use]
    #[inline]
    pub const fn from_bits(bits: u64) -> Option<Self> {
        if bits & Self::ALL.to_bits() == bits {
            Some(Self::from_bits_retain(bits))
        } else {
            None
        }
    }

    /// Create a hand from a bitset of cards, removing invalid cards
    #[must_use]
    #[inline]
    pub const fn from_bits_truncate(bits: u64) -> Self {
        Self::from_bits_retain(bits & Self::ALL.to_bits())
    }

    /// Create a hand from four holdings in suit order (clubs, diamonds, hearts, spades)
    #[must_use]
    #[inline]
    pub const fn new(clubs: Holding, diamonds: Holding, hearts: Holding, spades: Holding) -> Self {
        Self([clubs, diamonds, hearts, spades])
    }

    /// Create a hand containing a single card
    #[must_use]
    #[inline]
    pub const fn from_card(card: Card) -> Self {
        let mut hand = Self::EMPTY;
        hand.0[card.suit as usize] = Holding::from_rank(card.rank);
        hand
    }

    /// The empty hand
    pub const EMPTY: Self = Self([Holding::EMPTY; 4]);

    /// The hand containing all 52 cards
    pub const ALL: Self = Self([Holding::ALL; 4]);

    /// The number of cards in the hand
    #[must_use]
    #[inline]
    pub const fn len(self) -> usize {
        self.to_bits().count_ones() as usize
    }

    /// Whether the hand is empty
    #[must_use]
    #[inline]
    pub const fn is_empty(self) -> bool {
        self.to_bits() == 0
    }

    /// Whether the hand contains a card
    #[must_use]
    #[inline]
    pub fn contains(self, card: Card) -> bool {
        self[card.suit].contains(card.rank)
    }

    /// Insert a card into the hand, returning whether it was newly inserted
    #[inline]
    pub fn insert(&mut self, card: Card) -> bool {
        self[card.suit].insert(card.rank)
    }

    /// Remove a card from the hand, returning whether it was present
    #[inline]
    pub fn remove(&mut self, card: Card) -> bool {
        self[card.suit].remove(card.rank)
    }

    /// Toggle a card in the hand, returning whether it is now present
    #[inline]
    pub fn toggle(&mut self, card: Card) -> bool {
        self[card.suit].toggle(card.rank)
    }

    /// Conditionally insert/remove a card from the hand
    #[inline]
    pub fn set(&mut self, card: Card, condition: bool) {
        self[card.suit].set(card.rank, condition);
    }

    /// Iterate over the cards in the hand
    #[inline]
    #[must_use]
    pub const fn iter(self) -> HandIter {
        HandIter {
            suits: [
                self.0[0].iter(),
                self.0[1].iter(),
                self.0[2].iter(),
                self.0[3].iter(),
            ],
            fwd: 0,
            bwd: 3,
        }
    }
}

impl IntoIterator for Hand {
    type Item = Card;
    type IntoIter = HandIter;

    #[inline]
    fn into_iter(self) -> HandIter {
        self.iter()
    }
}

impl FromIterator<Card> for Hand {
    fn from_iter<I: IntoIterator<Item = Card>>(iter: I) -> Self {
        iter.into_iter().fold(Self::EMPTY, |mut hand, card| {
            hand.insert(card);
            hand
        })
    }
}

impl From<Card> for Hand {
    #[inline]
    fn from(card: Card) -> Self {
        Self::from_card(card)
    }
}

/// Dotted display of a hand: four suit groups in ascending order, clubs
/// first, e.g. `A23.456.789.T`
///
/// An empty hand is shown as `-`.  Note that this deliberately differs from
/// the descending, spades-first PBN format of bridge software.
///
/// This implementation ignores formatting flags for simplicity and speed.
impl fmt::Display for Hand {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.is_empty() {
            return f.write_char('-');
        }

        self[Suit::Clubs].fmt(f)?;
        f.write_char('.')?;

        self[Suit::Diamonds].fmt(f)?;
        f.write_char('.')?;

        self[Suit::Hearts].fmt(f)?;
        f.write_char('.')?;

        self[Suit::Spades].fmt(f)
    }
}

impl FromStr for Hand {
    type Err = ParseHandError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // 52 cards + 4 tens + 3 dots
        if s.len() > 52 + 4 + 3 {
            return Err(ParseHoldingError::TooManyCards.into());
        }

        if s == "-" {
            return Ok(Self::EMPTY);
        }

        let holdings: Result<Vec<_>, _> = s.split('.').map(Holding::from_str).collect();

        Ok(Self(
            holdings?
                .try_into()
                .map_err(|_| ParseHandError::NotFourSuits)?,
        ))
    }
}

impl ops::BitAnd for Hand {
    type Output = Self;

    #[inline]
    fn bitand(self, rhs: Self) -> Self {
        Self::from_bits_retain(self.to_bits() & rhs.to_bits())
    }
}

impl ops::BitOr for Hand {
    type Output = Self;

    #[inline]
    fn bitor(self, rhs: Self) -> Self {
        Self::from_bits_retain(self.to_bits() | rhs.to_bits())
    }
}

impl ops::BitXor for Hand {
    type Output = Self;

    #[inline]
    fn bitxor(self, rhs: Self) -> Self {
        Self::from_bits_retain(self.to_bits() ^ rhs.to_bits())
    }
}

impl ops::Not for Hand {
    type Output = Self;

    #[inline]
    fn not(self) -> Self {
        Self::from_bits_truncate(!self.to_bits())
    }
}

impl ops::Sub for Hand {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self {
        Self::from_bits_retain(self.to_bits() & !rhs.to_bits())
    }
}

impl ops::BitAndAssign for Hand {
    #[inline]
    fn bitand_assign(&mut self, rhs: Self) {
        *self = *self & rhs;
    }
}

impl ops::BitOrAssign for Hand {
    #[inline]
    fn bitor_assign(&mut self, rhs: Self) {
        *self = *self | rhs;
    }
}

impl ops::BitXorAssign for Hand {
    #[inline]
    fn bitxor_assign(&mut self, rhs: Self) {
        *self = *self ^ rhs;
    }
}

impl ops::SubAssign for Hand {
    #[inline]
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rank_letters_and_deadwood() {
        let letters: Vec<char> = (1..=13).map(|r| Rank::new(r).letter()).collect();
        assert_eq!(letters.iter().collect::<String>(), "A23456789TJQK");

        let values: Vec<u8> = (1..=13).map(|r| Rank::new(r).deadwood()).collect();
        assert_eq!(values, [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 10, 10, 10]);

        assert!(Rank::A < Rank::new(2));
        assert!(Rank::T < Rank::J);
        assert_eq!(Rank::try_new(0), Err(InvalidRank(0)));
        assert_eq!(Rank::try_new(14), Err(InvalidRank(14)));
    }

    #[test]
    fn holding_bit_ops() {
        let mut holding = Holding::EMPTY;
        assert!(holding.insert(Rank::A));
        assert!(!holding.insert(Rank::A));
        assert!(holding.insert(Rank::K));
        assert_eq!(holding.len(), 2);
        assert!(holding.contains(Rank::A));
        assert!(!holding.contains(Rank::Q));

        assert!(holding.remove(Rank::A));
        assert!(!holding.remove(Rank::A));
        assert!(holding.toggle(Rank::Q));
        assert!(!holding.toggle(Rank::Q));

        holding.set(Rank::T, true);
        holding.set(Rank::K, false);
        assert_eq!(holding.iter().collect::<Vec<_>>(), [Rank::T]);

        assert_eq!(!Holding::EMPTY, Holding::ALL);
        assert_eq!(Holding::ALL.len(), 13);
        assert_eq!(Holding::ALL - Holding::ALL, Holding::EMPTY);
    }

    #[test]
    fn holding_iteration_is_ascending() {
        let holding: Holding = "A23TK".parse().unwrap();
        let ranks: Vec<u8> = holding.iter().map(Rank::get).collect();
        assert_eq!(ranks, [1, 2, 3, 10, 13]);

        let reversed: Vec<u8> = holding.iter().rev().map(Rank::get).collect();
        assert_eq!(reversed, [13, 10, 3, 2, 1]);

        let mut iter = holding.iter();
        assert_eq!(iter.len(), 5);
        assert_eq!(iter.next().map(Rank::get), Some(1));
        assert_eq!(iter.next_back().map(Rank::get), Some(13));
        assert_eq!(iter.len(), 3);
        assert_eq!(iter.next().map(Rank::get), Some(2));
        assert_eq!(iter.next_back().map(Rank::get), Some(10));
        assert_eq!(iter.next().map(Rank::get), Some(3));
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next_back(), None);
    }

    #[test]
    fn invalid_bits_are_not_iterated() {
        let holding = Holding::from_bits_retain(0x8001);
        assert!(holding.contains_unknown_bits());
        assert_eq!(holding.iter().count(), 0);
        assert_eq!(Holding::from_bits(0x8001), None);
        assert_eq!(Holding::from_bits_truncate(0x8001), Holding::EMPTY);
    }

    #[test]
    fn hand_layout() {
        let mut hand = Hand::EMPTY;
        assert!(hand.insert(Card {
            suit: Suit::Clubs,
            rank: Rank::A,
        }));
        assert_eq!(hand.to_bits(), 2);

        assert!(hand.insert(Card {
            suit: Suit::Spades,
            rank: Rank::K,
        }));
        assert_eq!(hand.to_bits(), 2 | 1 << 61);

        assert_eq!(Hand::from_bits_retain(hand.to_bits()), hand);
        assert_eq!(hand.len(), 2);
    }

    #[test]
    fn hand_iteration_is_ascending() {
        let hand: Hand = "A23.456.789.T".parse().unwrap();
        assert_eq!(hand.len(), 10);

        let cards: Vec<Card> = hand.iter().collect();
        assert_eq!(cards.first().map(ToString::to_string), Some("♣A".into()));
        assert_eq!(cards.last().map(ToString::to_string), Some("♠T".into()));
        assert!(cards.windows(2).all(|w| {
            let (a, b) = (w[0], w[1]);
            a.suit < b.suit || (a.suit == b.suit && a.rank < b.rank)
        }));

        assert_eq!(hand.iter().rev().count(), 10);
        assert_eq!(Hand::from_iter(cards), hand);
    }
}
