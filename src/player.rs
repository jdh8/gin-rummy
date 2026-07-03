//! The two players of gin rummy.

use core::fmt;

/// One of the two players
///
/// Players index per-player arrays via `player as usize`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum Player {
    /// The first player
    One,
    /// The second player
    Two,
}

impl Player {
    /// Both players in order
    pub const ALL: [Self; 2] = [Self::One, Self::Two];

    /// The other player
    #[must_use]
    #[inline]
    pub const fn opponent(self) -> Self {
        match self {
            Self::One => Self::Two,
            Self::Two => Self::One,
        }
    }
}

impl fmt::Display for Player {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::One => "Player 1",
            Self::Two => "Player 2",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opponents() {
        assert_eq!(Player::One.opponent(), Player::Two);
        assert_eq!(Player::Two.opponent(), Player::One);
        assert_eq!(Player::ALL[Player::One as usize], Player::One);
        assert_eq!(Player::One.to_string(), "Player 1");
    }
}
