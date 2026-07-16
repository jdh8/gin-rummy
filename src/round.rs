//! One deal of gin rummy, from the opening upcard to the showdown.
//!
//! A [`Round`] is a runtime-checked state machine.  Accessors expose the
//! whole position — including both hands and the stock order, so bots and
//! UIs enforce their own information hygiene — and action methods mutate it,
//! returning [`RoundError`] on illegal moves.
//!
//! The flow of a round:
//!
//! 1. **Upcard** ([`Phase::Upcard`]): the non-dealer, then the dealer, may
//!    [`take_discard`](Round::take_discard) the upcard or
//!    [`pass`](Round::pass).  If both pass, the non-dealer must draw from
//!    the stock.
//! 2. **Turns** ([`Phase::Draw`] then [`Phase::Discard`]): draw from the
//!    stock or the discard pile, then [`discard`](Round::discard),
//!    [`knock`](Round::knock), or
//!    [`declare_big_gin`](Round::declare_big_gin).
//! 3. **Layoffs** ([`Phase::Layoff`], skipped after gin): the defender may
//!    [`lay_off`](Round::lay_off) deadwood onto the knocker's spread, then
//!    [`finish_layoffs`](Round::finish_layoffs) settles the score.
//!
//! The round ends ([`Phase::Finished`]) with a [`RoundResult`], also
//! covering the *dead hand*: a discard that leaves two cards in the stock
//! without a knock voids the deal.

use crate::meld::Melds;
use crate::{Card, Hand, Meld, Player, Rules, deadwood};
use core::fmt;
use thiserror::Error;

/// The phase of a [`Round`]
///
/// UIs are expected to match on all phases, so this enum is exhaustive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Phase {
    /// The initial upcard is offered: take it or pass
    Upcard,
    /// The player to move draws from the stock or the discard pile
    Draw,
    /// The player to move discards, knocks, or declares big gin
    Discard,
    /// The defender may lay off cards onto the knocker's spread
    Layoff,
    /// The round is over; see [`Round::result`]
    Finished,
}

impl fmt::Display for Phase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Upcard => "upcard",
            Self::Draw => "draw",
            Self::Discard => "discard",
            Self::Layoff => "layoff",
            Self::Finished => "finished",
        })
    }
}

/// The outcome of a round
///
/// A result records facts; [`RoundResult::points`] prices them under a
/// [`Rules`].  The enum is non-exhaustive to leave room for future variants
/// (e.g. an Oklahoma hand multiplier).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum RoundResult {
    /// The stock ran down to two cards without a knock: nobody scores, and
    /// the same dealer redeals
    Dead,
    /// A knock won by `margin`, the difference in deadwood
    Knock {
        /// The knocker
        winner: Player,
        /// The deadwood difference
        margin: u8,
    },
    /// The defender matched or beat the knocker's deadwood
    Undercut {
        /// The defender
        winner: Player,
        /// The deadwood difference
        margin: u8,
    },
    /// A knock with zero deadwood; layoffs are not allowed against it
    Gin {
        /// The knocker
        winner: Player,
        /// The loser's deadwood
        deadwood: u8,
    },
    /// Eleven melded cards declared instead of discarding
    BigGin {
        /// The declarer
        winner: Player,
        /// The loser's deadwood
        deadwood: u8,
    },
}

impl RoundResult {
    /// The player who scores this result, or `None` for a dead hand
    #[must_use]
    pub const fn winner(self) -> Option<Player> {
        match self {
            Self::Dead => None,
            Self::Knock { winner, .. }
            | Self::Undercut { winner, .. }
            | Self::Gin { winner, .. }
            | Self::BigGin { winner, .. } => Some(winner),
        }
    }

    /// The points this result awards its winner under the given rules
    ///
    /// The box bonus and game-level bonuses are accounted by
    /// [`Game`](crate::Game), not here.
    #[must_use]
    pub const fn points(self, rules: &Rules) -> u16 {
        match self {
            Self::Dead => 0,
            Self::Knock { margin, .. } => margin as u16,
            Self::Undercut { margin, .. } => margin as u16 + rules.undercut_bonus,
            Self::Gin { deadwood, .. } => deadwood as u16 + rules.gin_bonus,
            // A big gin recorded under rules without one is at least a gin.
            Self::BigGin { deadwood, .. } => {
                deadwood as u16
                    + match rules.big_gin_bonus {
                        Some(bonus) => bonus,
                        None => rules.gin_bonus,
                    }
            }
        }
    }
}

/// Error returned when constructing a [`Round`] from an invalid deal
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DealError {
    /// Each player is dealt exactly 10 cards
    #[error("Each player is dealt exactly 10 cards")]
    WrongHandSize,

    /// The stock holds the remaining 31 cards
    #[error("The stock holds the remaining 31 cards")]
    WrongStockSize,

    /// A card appears twice across the hands, upcard, and stock
    #[error("A card appears twice across the hands, upcard, and stock")]
    RepeatedCard,
}

/// Error returned when a [`Round`] action is illegal
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum RoundError {
    /// The action does not belong to the current phase
    #[error("This action is not available in the {0} phase")]
    WrongPhase(Phase),

    /// After both players pass the upcard, the first draw must come from
    /// the stock
    #[error("After both players pass the upcard, the first draw must come from the stock")]
    MustDrawFromStock,

    /// The card is not in the acting player's hand
    #[error("{0} is not in the hand")]
    NotInHand(Card),

    /// The card was taken from the discard pile this turn
    #[error("{0} was taken from the discard pile this turn and cannot be discarded")]
    DiscardJustTaken(Card),

    /// The arrangement's deadwood exceeds the knock limit
    #[error("Deadwood {deadwood} exceeds the knock limit {limit}")]
    TooMuchDeadwood {
        /// The deadwood of the offered arrangement
        deadwood: u8,
        /// The knock limit in effect
        limit: u8,
    },

    /// The melds do not arrange the knocker's remaining hand
    #[error("The melds do not arrange the knocker's remaining hand")]
    MeldsMismatch,

    /// The hand is not fully melded
    #[error("The hand is not fully melded")]
    NotBigGin,

    /// Big gin is disabled by the rules
    #[error("Big gin is disabled by the rules")]
    BigGinDisabled,

    /// No meld at this index in the spread
    #[error("No meld at index {0} in the spread")]
    NoSuchMeld(usize),

    /// The card extends no end of the meld
    #[error("{0} does not fit the meld")]
    CannotLayOff(Card),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct KnockState {
    knocker: Player,
    spread: [Option<Meld>; 3],
    knocker_deadwood: u8,
    laid_off: Hand,
}

/// One deal of gin rummy
///
/// See the [module documentation](self) for the flow.  Construct with
/// [`Round::from_deal`] or, under the `rand` feature, [`Round::deal`].
///
/// With the `serde` feature, a `Round` serializes to a plain snapshot of
/// its position, and deserialization re-validates every invariant — a
/// corrupt snapshot is rejected, never trusted.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(into = "repr::RoundRepr", try_from = "repr::RoundRepr")
)]
pub struct Round {
    rules: Rules,
    dealer: Player,
    hands: [Hand; 2],
    /// Face-down stock; the top card is the last element.
    stock: Vec<Card>,
    /// Face-up discard pile; the top card is the last element.
    discards: Vec<Card>,
    /// The opening upcard, remembered for the Oklahoma knock limit.
    initial_upcard: Card,
    phase: Phase,
    turn: Player,
    /// Upcard offers declined so far (0..=2).
    passes: u8,
    /// Both players passed the upcard; the next draw must hit the stock.
    forced_stock: bool,
    /// The card taken from the discard pile this turn, if any.
    taken_discard: Option<Card>,
    knock: Option<KnockState>,
    result: Option<RoundResult>,
}

impl Round {
    /// Construct a round from a fixed initial deal
    ///
    /// `stock` is face-down draw order: the *last* element is drawn first.
    /// The upcard starts the discard pile, and the non-dealer moves first.
    ///
    /// # Errors
    ///
    /// When the hands are not 10 cards each, the stock is not the remaining
    /// 31 cards, or any card appears twice.
    pub fn from_deal(
        rules: Rules,
        dealer: Player,
        hands: [Hand; 2],
        upcard: Card,
        stock: Vec<Card>,
    ) -> Result<Self, DealError> {
        if hands[0].len() != 10 || hands[1].len() != 10 {
            return Err(DealError::WrongHandSize);
        }
        if stock.len() != 31 {
            return Err(DealError::WrongStockSize);
        }

        let mut seen = hands[0];
        if !(seen & hands[1]).is_empty() {
            return Err(DealError::RepeatedCard);
        }
        seen |= hands[1];
        if !seen.insert(upcard) {
            return Err(DealError::RepeatedCard);
        }
        if stock.iter().any(|&card| !seen.insert(card)) {
            return Err(DealError::RepeatedCard);
        }

        Ok(Self {
            rules,
            dealer,
            hands,
            stock,
            discards: vec![upcard],
            initial_upcard: upcard,
            phase: Phase::Upcard,
            turn: dealer.opponent(),
            passes: 0,
            forced_stock: false,
            taken_discard: None,
            knock: None,
            result: None,
        })
    }

    /// Shuffle and deal a fresh round: 10 cards to each player, an upcard,
    /// and a randomly ordered 31-card stock
    // The deal is disjoint by construction, so `from_deal` cannot fail.
    #[allow(clippy::missing_panics_doc)]
    #[cfg(feature = "rand")]
    #[must_use]
    pub fn deal(rules: Rules, dealer: Player, rng: &mut (impl rand::Rng + ?Sized)) -> Self {
        let mut deck = crate::deck::Deck::ALL;
        let hands = [deck.draw(rng, 10), deck.draw(rng, 10)];
        let upcard = deck.pop(rng).expect("32 cards remain after the hands");

        let mut stock = Vec::with_capacity(31);
        while let Some(card) = deck.pop(rng) {
            stock.push(card);
        }

        Self::from_deal(rules, dealer, hands, upcard, stock)
            .expect("a fresh deck deals disjoint cards")
    }

    /// The scoring rules of this round
    #[must_use]
    #[inline]
    pub const fn rules(&self) -> &Rules {
        &self.rules
    }

    /// The dealer of this round
    #[must_use]
    #[inline]
    pub const fn dealer(&self) -> Player {
        self.dealer
    }

    /// The non-dealer, who receives the first upcard offer
    #[must_use]
    #[inline]
    pub const fn non_dealer(&self) -> Player {
        self.dealer.opponent()
    }

    /// The current phase
    #[must_use]
    #[inline]
    pub const fn phase(&self) -> Phase {
        self.phase
    }

    /// The player to act, or `None` once the round is finished
    ///
    /// During the layoff phase this is the defender.
    #[must_use]
    pub const fn turn(&self) -> Option<Player> {
        match self.phase {
            Phase::Finished => None,
            _ => Some(self.turn),
        }
    }

    /// The hand of a player
    ///
    /// Both hands are visible by design; bots enforce their own information
    /// hygiene.
    #[must_use]
    #[inline]
    pub const fn hand(&self, player: Player) -> Hand {
        self.hands[player as usize]
    }

    /// The discard pile; the top card is the last element
    #[must_use]
    #[inline]
    pub fn discard_pile(&self) -> &[Card] {
        &self.discards
    }

    /// The face-down stock; the top card is the last element
    ///
    /// The full order is visible by design; bots enforce their own
    /// information hygiene.
    #[must_use]
    #[inline]
    pub fn stock(&self) -> &[Card] {
        &self.stock
    }

    /// The knock limit in effect for this round
    ///
    /// [`Rules::knock_limit`], capped by the opening upcard's value under
    /// an Oklahoma ruleset ([`Rules::oklahoma`]).  Read the limit here
    /// rather than from the rules.
    #[must_use]
    #[inline]
    pub const fn knock_limit(&self) -> u8 {
        self.rules.knock_limit_for(self.initial_upcard)
    }

    /// The opening upcard that seeded the discard pile
    ///
    /// Remembered even after the card is drawn; under [`Rules::oklahoma`]
    /// it sets [`knock_limit`](Self::knock_limit).
    #[must_use]
    #[inline]
    pub const fn initial_upcard(&self) -> Card {
        self.initial_upcard
    }

    /// The player who knocked or declared big gin, if any
    #[must_use]
    pub const fn knocker(&self) -> Option<Player> {
        match &self.knock {
            Some(state) => Some(state.knocker),
            None => None,
        }
    }

    /// The knocker's spread, extended by any layoffs so far
    ///
    /// Empty before a knock.
    pub fn spread(&self) -> impl Iterator<Item = Meld> + '_ {
        self.knock
            .iter()
            .flat_map(|state| state.spread.into_iter().flatten())
    }

    /// The cards the defender has laid off onto the spread
    #[must_use]
    pub fn laid_off(&self) -> Hand {
        self.knock.map_or(Hand::EMPTY, |state| state.laid_off)
    }

    /// The outcome, or `None` while the round is in play
    #[must_use]
    #[inline]
    pub const fn result(&self) -> Option<RoundResult> {
        self.result
    }

    const fn expect_phase(&self, phase: Phase) -> Result<(), RoundError> {
        if self.phase as u8 == phase as u8 {
            Ok(())
        } else {
            Err(RoundError::WrongPhase(self.phase))
        }
    }

    /// Decline the initial upcard
    ///
    /// The offer moves from the non-dealer to the dealer; when both pass,
    /// the non-dealer must open by drawing from the stock.
    ///
    /// # Errors
    ///
    /// [`RoundError::WrongPhase`] outside the upcard phase.
    pub fn pass(&mut self) -> Result<(), RoundError> {
        self.expect_phase(Phase::Upcard)?;
        self.passes += 1;
        if self.passes == 2 {
            self.turn = self.non_dealer();
            self.phase = Phase::Draw;
            self.forced_stock = true;
        } else {
            self.turn = self.dealer;
        }
        Ok(())
    }

    /// Take the top of the discard pile, returning the card
    ///
    /// Available as the upcard decision and as the draw of a normal turn.
    ///
    /// # Errors
    ///
    /// [`RoundError::WrongPhase`] outside the upcard and draw phases, and
    /// [`RoundError::MustDrawFromStock`] on the forced stock draw after both
    /// players passed the upcard.
    // The expect is unreachable: a draw decision implies a non-empty pile.
    #[allow(clippy::missing_panics_doc)]
    pub fn take_discard(&mut self) -> Result<Card, RoundError> {
        match self.phase {
            Phase::Draw if self.forced_stock => return Err(RoundError::MustDrawFromStock),
            Phase::Upcard | Phase::Draw => {}
            phase => return Err(RoundError::WrongPhase(phase)),
        }

        let card = self
            .discards
            .pop()
            .expect("the discard pile is never empty when it may be drawn from");
        self.hands[self.turn as usize].insert(card);
        self.taken_discard = Some(card);
        self.phase = Phase::Discard;
        Ok(card)
    }

    /// Draw the top of the stock, returning the card
    ///
    /// # Errors
    ///
    /// [`RoundError::WrongPhase`] outside the draw phase.
    // The expect is unreachable: the dead-hand rule ends the round before
    // the stock can empty.
    #[allow(clippy::missing_panics_doc)]
    pub fn draw_stock(&mut self) -> Result<Card, RoundError> {
        self.expect_phase(Phase::Draw)?;
        let card = self
            .stock
            .pop()
            .expect("the dead-hand rule keeps the stock non-empty");
        self.hands[self.turn as usize].insert(card);
        self.forced_stock = false;
        self.phase = Phase::Discard;
        Ok(card)
    }

    /// Check that `card` may leave the hand as this turn's discard.
    fn expect_sheddable(&self, card: Card) -> Result<(), RoundError> {
        if !self.hands[self.turn as usize].contains(card) {
            return Err(RoundError::NotInHand(card));
        }
        if self.taken_discard == Some(card) {
            return Err(RoundError::DiscardJustTaken(card));
        }
        Ok(())
    }

    /// Move `card` from the acting hand to the discard pile.
    fn shed(&mut self, card: Card) {
        self.hands[self.turn as usize].remove(card);
        self.discards.push(card);
        self.taken_discard = None;
    }

    /// Discard a card, ending the turn
    ///
    /// If this leaves two cards in the stock, the round ends as a dead
    /// hand: nobody scores and the same dealer redeals.
    ///
    /// # Errors
    ///
    /// [`RoundError::WrongPhase`] outside the discard phase,
    /// [`RoundError::NotInHand`], and [`RoundError::DiscardJustTaken`] for
    /// the card drawn from the discard pile this turn.
    pub fn discard(&mut self, card: Card) -> Result<(), RoundError> {
        self.expect_phase(Phase::Discard)?;
        self.expect_sheddable(card)?;
        self.shed(card);

        if self.stock.len() == 2 {
            self.result = Some(RoundResult::Dead);
            self.phase = Phase::Finished;
        } else {
            self.turn = self.turn.opponent();
            self.phase = Phase::Draw;
        }
        Ok(())
    }

    /// Discard a card and knock, spreading the given arrangement
    ///
    /// The knocker chooses the arrangement — it decides what the defender
    /// may lay off — so it is passed explicitly; `best_melds(hand - card)`
    /// recovers the automatic choice:
    ///
    /// ```
    /// # use gin_rummy::{best_melds, Round};
    /// # fn knock_optimally(round: &mut Round, card: gin_rummy::Card)
    /// # -> Result<(), gin_rummy::round::RoundError> {
    /// let hand = round.hand(round.turn().unwrap());
    /// round.knock(card, best_melds(hand - card.into()))
    /// # }
    /// ```
    ///
    /// An arrangement with zero deadwood is **gin**: the defender may not
    /// lay off, and the round finishes immediately.  Otherwise the round
    /// enters the layoff phase.
    ///
    /// # Errors
    ///
    /// [`RoundError::WrongPhase`] outside the discard phase,
    /// [`RoundError::NotInHand`], [`RoundError::DiscardJustTaken`],
    /// [`RoundError::MeldsMismatch`] when `melds` does not arrange exactly
    /// the hand minus the discard, and [`RoundError::TooMuchDeadwood`] over
    /// the knock limit.
    pub fn knock(&mut self, card: Card, melds: Melds) -> Result<(), RoundError> {
        self.expect_phase(Phase::Discard)?;
        self.expect_sheddable(card)?;
        if melds.hand() != self.hands[self.turn as usize] - card.into() {
            return Err(RoundError::MeldsMismatch);
        }

        let knocker_deadwood = melds.deadwood();
        if knocker_deadwood > self.knock_limit() {
            return Err(RoundError::TooMuchDeadwood {
                deadwood: knocker_deadwood,
                limit: self.knock_limit(),
            });
        }

        self.shed(card);
        let knocker = self.turn;
        self.knock = Some(KnockState {
            knocker,
            spread: melds.into_array(),
            knocker_deadwood,
            laid_off: Hand::EMPTY,
        });

        if knocker_deadwood == 0 {
            let loser = deadwood(self.hands[knocker.opponent() as usize]);
            self.result = Some(RoundResult::Gin {
                winner: knocker,
                deadwood: loser,
            });
            self.phase = Phase::Finished;
        } else {
            self.turn = knocker.opponent();
            self.phase = Phase::Layoff;
        }
        Ok(())
    }

    /// Declare big gin: all 11 cards melded, no discard
    ///
    /// The defender may not lay off, and the round finishes immediately.
    /// Declaring is never forced — nor ever a trap: an 11-card fully-melded
    /// hand always contains a meld of four or more cards, which can shed a
    /// card, so plain gin remains available when the rules disable big gin.
    ///
    /// # Errors
    ///
    /// [`RoundError::WrongPhase`] outside the discard phase,
    /// [`RoundError::BigGinDisabled`] when [`Rules::big_gin_bonus`] is
    /// `None`, [`RoundError::MeldsMismatch`] when `melds` does not arrange
    /// the full hand, and [`RoundError::NotBigGin`] on any deadwood.
    pub fn declare_big_gin(&mut self, melds: Melds) -> Result<(), RoundError> {
        self.expect_phase(Phase::Discard)?;
        if self.rules.big_gin_bonus.is_none() {
            return Err(RoundError::BigGinDisabled);
        }
        if melds.hand() != self.hands[self.turn as usize] {
            return Err(RoundError::MeldsMismatch);
        }
        if melds.deadwood() != 0 {
            return Err(RoundError::NotBigGin);
        }

        let winner = self.turn;
        self.taken_discard = None;
        self.knock = Some(KnockState {
            knocker: winner,
            spread: melds.into_array(),
            knocker_deadwood: 0,
            laid_off: Hand::EMPTY,
        });
        self.result = Some(RoundResult::BigGin {
            winner,
            deadwood: deadwood(self.hands[winner.opponent() as usize]),
        });
        self.phase = Phase::Finished;
        Ok(())
    }

    /// Lay off a card onto the meld at `index` in the knocker's spread
    ///
    /// Indices are stable across layoffs, so chained extensions — the ♠8
    /// then the ♠9 onto a 5-6-7 of spades — address the same meld.
    ///
    /// # Errors
    ///
    /// [`RoundError::WrongPhase`] outside the layoff phase,
    /// [`RoundError::NotInHand`], [`RoundError::NoSuchMeld`], and
    /// [`RoundError::CannotLayOff`] when the card extends neither end of a
    /// run nor completes a set.
    // The expect is unreachable: the layoff phase implies a knock.
    #[allow(clippy::missing_panics_doc)]
    pub fn lay_off(&mut self, card: Card, index: usize) -> Result<(), RoundError> {
        self.expect_phase(Phase::Layoff)?;
        if !self.hands[self.turn as usize].contains(card) {
            return Err(RoundError::NotInHand(card));
        }

        let state = self.knock.as_mut().expect("a layoff follows a knock");
        let meld = match state.spread.get(index) {
            Some(Some(meld)) => *meld,
            _ => return Err(RoundError::NoSuchMeld(index)),
        };
        let extended = meld.extended(card).ok_or(RoundError::CannotLayOff(card))?;

        state.spread[index] = Some(extended);
        state.laid_off.insert(card);
        self.hands[self.turn as usize].remove(card);
        Ok(())
    }

    /// End the layoff phase and settle the round
    ///
    /// The defender's remaining cards are melded optimally.  The defender
    /// wins an undercut on less deadwood than the knocker kept — or equal
    /// deadwood under [`Rules::undercut_on_tie`]; otherwise the knocker
    /// wins the deadwood difference.
    ///
    /// # Errors
    ///
    /// [`RoundError::WrongPhase`] outside the layoff phase.
    // The expect is unreachable: the layoff phase implies a knock.
    #[allow(clippy::missing_panics_doc)]
    pub fn finish_layoffs(&mut self) -> Result<RoundResult, RoundError> {
        self.expect_phase(Phase::Layoff)?;
        let state = self.knock.as_ref().expect("a layoff follows a knock");
        let knocker = state.knocker;
        let knocker_deadwood = state.knocker_deadwood;
        let defender_deadwood = deadwood(self.hands[knocker.opponent() as usize]);

        let undercut = defender_deadwood < knocker_deadwood
            || (defender_deadwood == knocker_deadwood && self.rules.undercut_on_tie);
        let result = if undercut {
            RoundResult::Undercut {
                winner: knocker.opponent(),
                margin: knocker_deadwood - defender_deadwood,
            }
        } else {
            RoundResult::Knock {
                winner: knocker,
                margin: defender_deadwood - knocker_deadwood,
            }
        };

        self.result = Some(result);
        self.phase = Phase::Finished;
        Ok(result)
    }
}

/// The serialized form of a [`Round`], re-validated on deserialization
#[cfg(feature = "serde")]
mod repr {
    use super::{KnockState, Phase, Round, RoundResult};
    use crate::{Card, Hand, Meld, Player, Rules, deadwood, pip_sum};
    use thiserror::Error;

    #[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
    pub enum InvalidRound {
        #[error(
            "The hands, stock, discard pile, and layoffs must hold each of the 52 cards exactly once"
        )]
        NotAPartition,
        #[error("Card counts are inconsistent with the phase")]
        WrongCounts,
        #[error("The pass counter is out of range or contradicts the acting player")]
        BadUpcardState,
        #[error("The initial upcard contradicts the discard pile")]
        BadInitialUpcard,
        #[error("A flag or component contradicts the phase")]
        PhaseMismatch,
        #[error("The spread is not disjoint melds over the knocker's cards and the layoffs")]
        BadSpread,
        #[error("The knocker's deadwood violates the knock limit")]
        BadDeadwood,
        #[error("The result does not match the recomputed score")]
        BadResult,
    }

    #[derive(Clone, serde::Serialize, serde::Deserialize)]
    pub struct KnockRepr {
        knocker: Player,
        spread: Vec<Meld>,
        laid_off: Hand,
    }

    #[derive(Clone, serde::Serialize, serde::Deserialize)]
    pub struct RoundRepr {
        rules: Rules,
        dealer: Player,
        hands: [Hand; 2],
        stock: Vec<Card>,
        discards: Vec<Card>,
        initial_upcard: Card,
        phase: Phase,
        turn: Player,
        passes: u8,
        forced_stock: bool,
        taken_discard: Option<Card>,
        knock: Option<KnockRepr>,
        result: Option<RoundResult>,
    }

    impl From<Round> for RoundRepr {
        fn from(round: Round) -> Self {
            Self {
                rules: round.rules,
                dealer: round.dealer,
                hands: round.hands,
                stock: round.stock,
                discards: round.discards,
                initial_upcard: round.initial_upcard,
                phase: round.phase,
                turn: round.turn,
                passes: round.passes,
                forced_stock: round.forced_stock,
                taken_discard: round.taken_discard,
                knock: round.knock.map(|state| KnockRepr {
                    knocker: state.knocker,
                    spread: state.spread.into_iter().flatten().collect(),
                    laid_off: state.laid_off,
                }),
                result: round.result,
            }
        }
    }

    /// Rebuild the knock state, checking the spread against the knocker's
    /// cards and computing the deadwood frozen at knock time.
    fn validate_knock(repr: &KnockRepr, hands: &[Hand; 2]) -> Result<KnockState, InvalidRound> {
        if repr.spread.len() > 3 {
            return Err(InvalidRound::BadSpread);
        }

        let mut union = Hand::EMPTY;
        let mut spread = [None; 3];
        for (slot, &meld) in spread.iter_mut().zip(&repr.spread) {
            if !(union & meld.cards()).is_empty() {
                return Err(InvalidRound::BadSpread);
            }
            union |= meld.cards();
            *slot = Some(meld);
        }

        // Laid-off cards extend spread melds; the rest of the spread is the
        // knocker's own melded cards.
        let own = union - repr.laid_off;
        if repr.laid_off & union != repr.laid_off || own & hands[repr.knocker as usize] != own {
            return Err(InvalidRound::BadSpread);
        }

        // The knocker holds at most 11 cards here, so the sum fits `u8`.
        let knocker_deadwood = pip_sum(hands[repr.knocker as usize] - union) as u8;
        Ok(KnockState {
            knocker: repr.knocker,
            spread,
            knocker_deadwood,
            laid_off: repr.laid_off,
        })
    }

    impl TryFrom<RoundRepr> for Round {
        type Error = InvalidRound;

        #[allow(clippy::too_many_lines)]
        fn try_from(repr: RoundRepr) -> Result<Self, InvalidRound> {
            // Every card lives in exactly one place.
            let mut seen = repr.hands[0];
            if !(seen & repr.hands[1]).is_empty() {
                return Err(InvalidRound::NotAPartition);
            }
            seen |= repr.hands[1];
            for &card in repr.stock.iter().chain(&repr.discards) {
                if !seen.insert(card) {
                    return Err(InvalidRound::NotAPartition);
                }
            }
            if let Some(knock) = &repr.knock {
                if !(seen & knock.laid_off).is_empty() {
                    return Err(InvalidRound::NotAPartition);
                }
                seen |= knock.laid_off;
            }
            if seen != Hand::ALL {
                return Err(InvalidRound::NotAPartition);
            }

            let len = |player: Player| repr.hands[player as usize].len();
            let turn = repr.turn;
            if repr.passes > 2 {
                return Err(InvalidRound::BadUpcardState);
            }

            // The forced stock draw exists only right after both players
            // pass the upcard, and nothing else may linger across phases it
            // does not belong to.
            if repr.forced_stock
                && !(repr.phase == Phase::Draw && repr.passes == 2 && repr.stock.len() == 31)
            {
                return Err(InvalidRound::PhaseMismatch);
            }
            if repr.taken_discard.is_some() && repr.phase != Phase::Discard {
                return Err(InvalidRound::PhaseMismatch);
            }
            if repr.result.is_some() != (repr.phase == Phase::Finished) {
                return Err(InvalidRound::PhaseMismatch);
            }

            // While the upcard is on offer the pile is exactly that card,
            // and once both players pass it, it is buried for good: the
            // pile never shrinks across turns, so it stays `discards[0]`.
            // After a take the field is unverifiable and trusted.
            if (repr.phase == Phase::Upcard || repr.passes == 2)
                && repr.discards.first() != Some(&repr.initial_upcard)
            {
                return Err(InvalidRound::BadInitialUpcard);
            }
            let knock_limit = repr.rules.knock_limit_for(repr.initial_upcard);

            let knock = match repr.phase {
                Phase::Upcard => {
                    if repr.passes > 1
                        || turn
                            != if repr.passes == 0 {
                                repr.dealer.opponent()
                            } else {
                                repr.dealer
                            }
                    {
                        return Err(InvalidRound::BadUpcardState);
                    }
                    if repr.stock.len() != 31
                        || repr.discards.len() != 1
                        || len(Player::One) != 10
                        || len(Player::Two) != 10
                    {
                        return Err(InvalidRound::WrongCounts);
                    }
                    if repr.knock.is_some() {
                        return Err(InvalidRound::PhaseMismatch);
                    }
                    None
                }
                Phase::Draw => {
                    if len(Player::One) != 10
                        || len(Player::Two) != 10
                        || repr.stock.len() < 3
                        || repr.discards.is_empty()
                    {
                        return Err(InvalidRound::WrongCounts);
                    }
                    if repr.knock.is_some() {
                        return Err(InvalidRound::PhaseMismatch);
                    }
                    None
                }
                Phase::Discard => {
                    if len(turn) != 11 || len(turn.opponent()) != 10 || repr.stock.len() < 2 {
                        return Err(InvalidRound::WrongCounts);
                    }
                    if repr.knock.is_some() {
                        return Err(InvalidRound::PhaseMismatch);
                    }
                    if let Some(card) = repr.taken_discard
                        && !repr.hands[turn as usize].contains(card)
                    {
                        return Err(InvalidRound::PhaseMismatch);
                    }
                    None
                }
                Phase::Layoff => {
                    let knock = repr.knock.as_ref().ok_or(InvalidRound::PhaseMismatch)?;
                    let state = validate_knock(knock, &repr.hands)?;
                    // Gin skips layoffs, so a live layoff phase means the
                    // knocker kept deadwood within the limit.
                    if state.knocker_deadwood == 0 {
                        return Err(InvalidRound::PhaseMismatch);
                    }
                    if state.knocker_deadwood > knock_limit {
                        return Err(InvalidRound::BadDeadwood);
                    }
                    if turn != state.knocker.opponent() {
                        return Err(InvalidRound::PhaseMismatch);
                    }
                    if len(state.knocker) != 10
                        || len(turn) + state.laid_off.len() != 10
                        || repr.stock.len() < 2
                        || repr.discards.is_empty()
                    {
                        return Err(InvalidRound::WrongCounts);
                    }
                    Some(state)
                }
                Phase::Finished => {
                    let result = repr.result.ok_or(InvalidRound::PhaseMismatch)?;
                    match result {
                        RoundResult::Dead => {
                            if repr.knock.is_some() {
                                return Err(InvalidRound::PhaseMismatch);
                            }
                            if repr.stock.len() != 2
                                || len(Player::One) != 10
                                || len(Player::Two) != 10
                            {
                                return Err(InvalidRound::WrongCounts);
                            }
                            None
                        }
                        RoundResult::Knock { winner, margin }
                        | RoundResult::Undercut { winner, margin } => {
                            let knock = repr.knock.as_ref().ok_or(InvalidRound::PhaseMismatch)?;
                            let state = validate_knock(knock, &repr.hands)?;
                            let undercut = matches!(result, RoundResult::Undercut { .. });
                            let knocker = if undercut { winner.opponent() } else { winner };
                            if state.knocker != knocker || state.knocker_deadwood == 0 {
                                return Err(InvalidRound::BadResult);
                            }
                            if state.knocker_deadwood > knock_limit {
                                return Err(InvalidRound::BadDeadwood);
                            }
                            let defender = knocker.opponent();
                            if len(knocker) != 10
                                || len(defender) + state.laid_off.len() != 10
                                || repr.stock.len() < 2
                            {
                                return Err(InvalidRound::WrongCounts);
                            }

                            // Wrapping subtraction: a corrupt snapshot may
                            // put the margin on the wrong side, which the
                            // `undercut != expected` test rejects anyway.
                            let defender_deadwood = deadwood(repr.hands[defender as usize]);
                            let expected = defender_deadwood < state.knocker_deadwood
                                || (defender_deadwood == state.knocker_deadwood
                                    && repr.rules.undercut_on_tie);
                            let expected_margin = if undercut {
                                state.knocker_deadwood.wrapping_sub(defender_deadwood)
                            } else {
                                defender_deadwood.wrapping_sub(state.knocker_deadwood)
                            };
                            if undercut != expected || margin != expected_margin {
                                return Err(InvalidRound::BadResult);
                            }
                            Some(state)
                        }
                        RoundResult::Gin {
                            winner,
                            deadwood: loser,
                        }
                        | RoundResult::BigGin {
                            winner,
                            deadwood: loser,
                        } => {
                            let knock = repr.knock.as_ref().ok_or(InvalidRound::PhaseMismatch)?;
                            let state = validate_knock(knock, &repr.hands)?;
                            let big = matches!(result, RoundResult::BigGin { .. });
                            let expected_len = if big { 11 } else { 10 };
                            if state.knocker != winner
                                || state.knocker_deadwood != 0
                                || !state.laid_off.is_empty()
                            {
                                return Err(InvalidRound::BadResult);
                            }
                            if big && repr.rules.big_gin_bonus.is_none() {
                                return Err(InvalidRound::BadResult);
                            }
                            if len(winner) != expected_len
                                || len(winner.opponent()) != 10
                                || repr.stock.len() < 2
                            {
                                return Err(InvalidRound::WrongCounts);
                            }
                            if loser != deadwood(repr.hands[winner.opponent() as usize]) {
                                return Err(InvalidRound::BadResult);
                            }
                            Some(state)
                        }
                    }
                }
            };

            Ok(Self {
                rules: repr.rules,
                dealer: repr.dealer,
                hands: repr.hands,
                stock: repr.stock,
                discards: repr.discards,
                initial_upcard: repr.initial_upcard,
                phase: repr.phase,
                turn: repr.turn,
                passes: repr.passes,
                forced_stock: repr.forced_stock,
                taken_discard: repr.taken_discard,
                knock,
                result: repr.result,
            })
        }
    }
}
