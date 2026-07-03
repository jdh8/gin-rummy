//! The scoreboard across deals.
//!
//! A [`Game`] accumulates [`RoundResult`]s until a player reaches the game
//! target, then settles boxes, the game bonus, and the shutout rule into a
//! [`FinalScore`].  It also tracks whose deal is next: the winner of a hand
//! deals the following one, and a dead hand is redealt by the same dealer.

use crate::{Player, RoundResult, Rules, Shutout};
use thiserror::Error;

/// Error returned when recording into a finished [`Game`]
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum GameError {
    /// The game is already over
    #[error("The game is already over")]
    AlreadyOver,
}

/// The settled outcome of a [`Game`]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct FinalScore {
    /// The player who reached the game target
    pub winner: Player,
    /// Final totals by player, including boxes and bonuses
    pub totals: [u16; 2],
    /// Whether the loser never scored (a shutout, also called a blitz or
    /// schneider), with [`Rules::shutout`] already applied to the totals
    pub shutout: bool,
}

/// A game of gin rummy: the running scores across rounds
///
/// ```
/// use gin_rummy::{Game, Player, Rules, RoundResult};
///
/// let mut game = Game::new(Rules::default(), Player::One);
/// game.record(RoundResult::Gin { winner: Player::Two, deadwood: 51 })?;
/// assert_eq!(game.score(Player::Two), 76);
/// assert_eq!(game.next_dealer(), Player::Two);
/// # Ok::<(), gin_rummy::game::GameError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Game {
    rules: Rules,
    scores: [u16; 2],
    boxes: [u16; 2],
    next_dealer: Player,
}

impl Game {
    /// Start a game with the given rules and first dealer
    #[must_use]
    #[inline]
    pub const fn new(rules: Rules, first_dealer: Player) -> Self {
        Self {
            rules,
            scores: [0; 2],
            boxes: [0; 2],
            next_dealer: first_dealer,
        }
    }

    /// The scoring rules of this game
    #[must_use]
    #[inline]
    pub const fn rules(&self) -> &Rules {
        &self.rules
    }

    /// The running score of a player
    ///
    /// Under [`Rules::immediate_boxes`] this includes the boxes credited so
    /// far; otherwise boxes wait for [`Game::final_score`].
    #[must_use]
    #[inline]
    pub const fn score(&self, player: Player) -> u16 {
        self.scores[player as usize]
    }

    /// The hands a player has won
    #[must_use]
    #[inline]
    pub const fn boxes(&self, player: Player) -> u16 {
        self.boxes[player as usize]
    }

    /// Who deals the next round
    #[must_use]
    #[inline]
    pub const fn next_dealer(&self) -> Player {
        self.next_dealer
    }

    /// Whether a player has reached the game target
    #[must_use]
    pub const fn is_over(&self) -> bool {
        self.scores[0] >= self.rules.game_target || self.scores[1] >= self.rules.game_target
    }

    /// The player who reached the game target, or `None` while the game is
    /// in play
    #[must_use]
    pub const fn winner(&self) -> Option<Player> {
        // Only the round winner gains points, so exactly one player can
        // cross the target.
        if self.scores[0] >= self.rules.game_target {
            Some(Player::One)
        } else if self.scores[1] >= self.rules.game_target {
            Some(Player::Two)
        } else {
            None
        }
    }

    /// Record a finished round
    ///
    /// The winner gains the result's points — plus the box bonus right away
    /// under [`Rules::immediate_boxes`] — and one box, and deals the next
    /// round.  A dead hand scores nothing and keeps the same dealer.
    ///
    /// # Errors
    ///
    /// [`GameError::AlreadyOver`] once a player has reached the target.
    pub fn record(&mut self, result: RoundResult) -> Result<(), GameError> {
        if self.is_over() {
            return Err(GameError::AlreadyOver);
        }

        if let Some(winner) = result.winner() {
            let immediate = if self.rules.immediate_boxes {
                self.rules.box_bonus
            } else {
                0
            };
            self.scores[winner as usize] += result.points(&self.rules) + immediate;
            self.boxes[winner as usize] += 1;
            self.next_dealer = winner;
        }
        Ok(())
    }

    /// Shuffle and deal the next round with this game's rules and dealer
    #[cfg(feature = "rand")]
    #[must_use]
    pub fn deal(&self, rng: &mut (impl rand::Rng + ?Sized)) -> crate::Round {
        crate::Round::deal(self.rules, self.next_dealer, rng)
    }

    /// Settle the game, or `None` while it is in play
    ///
    /// Boxes are added at
    /// [`box_bonus`](Rules::box_bonus) each — unless already credited via
    /// [`Rules::immediate_boxes`] — the winner collects
    /// [`game_bonus`](Rules::game_bonus), and a shutout applies
    /// [`Rules::shutout`] to the winner's total.
    #[must_use]
    pub fn final_score(&self) -> Option<FinalScore> {
        let winner = self.winner()?;
        let loser = winner.opponent();
        let shutout = self.scores[loser as usize] == 0;

        let mut totals = self.scores;
        if !self.rules.immediate_boxes {
            for player in Player::ALL {
                totals[player as usize] += self.boxes[player as usize] * self.rules.box_bonus;
            }
        }
        totals[winner as usize] += self.rules.game_bonus;

        if shutout {
            totals[winner as usize] = match self.rules.shutout {
                Shutout::Double => totals[winner as usize] * 2,
                Shutout::Flat(bonus) => totals[winner as usize] + bonus,
            };
        }

        Some(FinalScore {
            winner,
            totals,
            shutout,
        })
    }
}
