#![doc = include_str!("../README.md")]
#![warn(missing_docs)]
// The 52-card bitset design extracts 16-bit suit lanes and 4-bit ranks from
// `u64` masks all over the crate; every such cast keeps exactly the intended
// bits.  Allowed here so a pedantic lint run is quiet on `src/`; this is a
// no-op under the default CI lint set.
#![allow(clippy::cast_possible_truncation)]

#[cfg(feature = "rand")]
pub mod deck;
pub mod game;
pub mod hand;
pub mod meld;
pub mod player;
pub mod round;
pub mod rules;

pub use game::{FinalScore, Game};
pub use hand::{Card, Hand, Holding, Rank};
pub use meld::{Meld, MeldKind, Melds, best_melds, deadwood, pip_sum};
pub use player::Player;
pub use round::{Phase, Round, RoundResult};
pub use rules::{OklahomaAce, Rules, Shutout};

use core::fmt::{self, Write as _};
use core::str::FromStr;
use thiserror::Error;

/// A suit of playing cards
///
/// Suits are ranked alphabetically, clubs low, by deriving [`PartialOrd`] and
/// [`Ord`].  Gin rummy itself treats suits as equals; the order only fixes
/// the display and encoding conventions of this crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum Suit {
    /// ♣
    Clubs,
    /// ♦
    Diamonds,
    /// ♥
    Hearts,
    /// ♠
    Spades,
}

impl Suit {
    /// Suits in the ascending order, the order in this crate
    pub const ASC: [Self; 4] = [Self::Clubs, Self::Diamonds, Self::Hearts, Self::Spades];

    /// Suits in the descending order
    pub const DESC: [Self; 4] = [Self::Spades, Self::Hearts, Self::Diamonds, Self::Clubs];

    /// Uppercase letter
    #[must_use]
    #[inline]
    pub const fn letter(self) -> char {
        match self {
            Self::Clubs => 'C',
            Self::Diamonds => 'D',
            Self::Hearts => 'H',
            Self::Spades => 'S',
        }
    }
}

impl fmt::Display for Suit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_char(match self {
            Self::Clubs => '♣',
            Self::Diamonds => '♦',
            Self::Hearts => '♥',
            Self::Spades => '♠',
        })
    }
}

/// Unicode variation selectors that may appear after suit emojis
///
/// We want to ignore these suffixes when parsing suits.
const EMOJI_SELECTORS: [char; 2] = ['\u{FE0F}', '\u{FE0E}'];

/// Error returned when parsing a [`Suit`] fails
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
#[error("Invalid suit: expected one of C, D, H, S, ♣, ♦, ♥, ♠, ♧, ♢, ♡, ♤")]
pub struct ParseSuitError;

impl FromStr for Suit {
    type Err = ParseSuitError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s
            .to_ascii_uppercase()
            .as_str()
            .trim_end_matches(EMOJI_SELECTORS)
        {
            "C" | "♣" | "♧" => Ok(Self::Clubs),
            "D" | "♦" | "♢" => Ok(Self::Diamonds),
            "H" | "♥" | "♡" => Ok(Self::Hearts),
            "S" | "♠" | "♤" => Ok(Self::Spades),
            _ => Err(ParseSuitError),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suit_order_matches_arrays() {
        assert!(Suit::Clubs < Suit::Diamonds);
        assert!(Suit::Diamonds < Suit::Hearts);
        assert!(Suit::Hearts < Suit::Spades);

        let mut sorted = Suit::DESC;
        sorted.sort();
        assert_eq!(sorted, Suit::ASC);
    }

    #[test]
    fn suit_parsing() {
        for suit in Suit::ASC {
            assert_eq!(suit.to_string().parse(), Ok(suit));
            assert_eq!(suit.letter().to_string().parse(), Ok(suit));
            assert_eq!(
                suit.letter().to_ascii_lowercase().to_string().parse(),
                Ok(suit)
            );
        }

        assert_eq!("SPADES".parse::<Suit>(), Err(ParseSuitError));
        assert_eq!("".parse::<Suit>(), Err(ParseSuitError));
        assert_eq!("♠♠".parse::<Suit>(), Err(ParseSuitError));
        assert_eq!("♠\u{FE0F}".parse(), Ok(Suit::Spades));
        assert_eq!("♤\u{FE0E}".parse(), Ok(Suit::Spades));
    }
}
