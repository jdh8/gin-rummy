//! Scoring configuration.
//!
//! Gin rummy bonuses vary by rule school, so every value is an independent
//! knob on [`Rules`].  Three presets cover the common schools: modern
//! tournament values ([`Rules::new`], the default), the classic Bicycle
//! rules ([`Rules::classic`]), and the Gin Rummy Palace app
//! ([`Rules::palace`]).
//!
//! `Rules` is `#[non_exhaustive]`: start from a preset and adjust fields,
//! e.g. `Rules { game_target: 250, ..Rules::default() }` does not compile,
//! but mutating `rules.game_target = 250;` does.  This keeps room for future
//! variants (an Oklahoma spade multiplier, Hollywood scoring) without
//! breakage.
//!
//! Two variants ride on existing knobs: Oklahoma gin is
//! [`Rules::oklahoma`], and straight gin — no knocking short of gin — is
//! exactly `knock_limit: 0`.

use crate::Card;

/// What happens to the winner's total when the loser scored nothing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum Shutout {
    /// The winner's final total is doubled (modern schools)
    Double,
    /// A flat bonus is added, e.g. 100 in the classic rules — `Flat(0)`
    /// disables the shutout rule entirely
    Flat(u16),
}

/// How an ace upcard sets the knock limit in Oklahoma gin
///
/// Pagat gives the base rule as the upcard's value, so an ace allows a
/// 1-point knock, and records that some tables instead demand gin
/// outright.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub enum OklahomaAce {
    /// The ace counts its pip value: knock at 1 or less
    One,
    /// The ace demands gin: the knock limit is 0
    GinOnly,
}

/// The scoring rules of a game
///
/// All fields are public knobs; see the [module documentation](self) for the
/// preset-and-adjust pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct Rules {
    /// The most deadwood a knocker may keep: 10 in every common school,
    /// and `0` plays straight gin — only a gin knock ends the round
    pub knock_limit: u8,

    /// Oklahoma gin: the opening upcard caps the knock limit at its
    /// deadwood value (pictures 10, aces per [`OklahomaAce`]), or `None`
    /// for a fixed [`knock_limit`](Self::knock_limit)
    ///
    /// No preset turns this on; enable it atop any of them.  Some tables
    /// also double the hand score on a spade upcard — not modeled yet.
    #[cfg_attr(feature = "serde", serde(default))]
    pub oklahoma: Option<OklahomaAce>,

    /// Bonus for going gin: 25 modern and palace, 20 classic
    pub gin_bonus: u16,

    /// Bonus for big gin — 11 melded cards declared instead of discarding —
    /// or `None` where the variant is not played (classic, palace)
    pub big_gin_bonus: Option<u16>,

    /// Bonus for undercutting the knocker: 25 modern, 10 classic, 20 palace
    pub undercut_bonus: u16,

    /// Whether the defender undercuts on equal deadwood (all three presets
    /// say yes; some traditional tables require strictly less)
    pub undercut_on_tie: bool,

    /// Bonus per won hand: 25 modern, 20 classic, 10 palace
    pub box_bonus: u16,

    /// When `true`, the box bonus is credited to the running score as each
    /// hand is won, counting toward [`game_target`](Self::game_target) (Gin
    /// Rummy Palace); when `false`, boxes are tallied only at game end
    /// (traditional)
    pub immediate_boxes: bool,

    /// Bonus to the winner of the game: 100 traditionally, 0 palace
    pub game_bonus: u16,

    /// The score that ends the game: 100 traditionally; house games play to
    /// 150 or 250, and Gin Rummy Palace offers 10 through 500
    pub game_target: u16,

    /// The shutout (blitz, schneider) rule for games whose loser never
    /// scored
    pub shutout: Shutout,
}

impl Rules {
    /// Modern tournament scoring, the default
    #[must_use]
    #[inline]
    pub const fn new() -> Self {
        Self {
            knock_limit: 10,
            oklahoma: None,
            gin_bonus: 25,
            big_gin_bonus: Some(31),
            undercut_bonus: 25,
            undercut_on_tie: true,
            box_bonus: 25,
            immediate_boxes: false,
            game_bonus: 100,
            game_target: 100,
            shutout: Shutout::Double,
        }
    }

    /// Classic scoring per Bicycle: smaller bonuses, no big gin, and a flat
    /// 100-point shutout bonus
    #[must_use]
    #[inline]
    pub const fn classic() -> Self {
        Self {
            gin_bonus: 20,
            big_gin_bonus: None,
            undercut_bonus: 10,
            box_bonus: 20,
            shutout: Shutout::Flat(100),
            ..Self::new()
        }
    }

    /// Gin Rummy Palace app scoring: boxes of 10 credited immediately, no
    /// game or shutout bonus
    ///
    /// The app lets players set the target between 10 and 500; adjust
    /// [`game_target`](Self::game_target) to taste.
    #[must_use]
    #[inline]
    pub const fn palace() -> Self {
        Self {
            big_gin_bonus: None,
            undercut_bonus: 20,
            box_bonus: 10,
            immediate_boxes: true,
            game_bonus: 0,
            shutout: Shutout::Flat(0),
            ..Self::new()
        }
    }

    /// The knock limit in effect for a round opened by `initial_upcard`
    ///
    /// Under [`oklahoma`](Self::oklahoma) the upcard's deadwood value caps
    /// [`knock_limit`](Self::knock_limit) — the house limit only ever
    /// tightens, never widens; without Oklahoma it passes through
    /// unchanged.  [`Round::knock_limit`](crate::Round::knock_limit)
    /// resolves this per round.
    #[must_use]
    #[inline]
    pub const fn knock_limit_for(&self, initial_upcard: Card) -> u8 {
        let value = match self.oklahoma {
            None => return self.knock_limit,
            Some(OklahomaAce::GinOnly) if initial_upcard.rank.get() == 1 => 0,
            Some(_) => initial_upcard.rank.deadwood(),
        };
        if value < self.knock_limit {
            value
        } else {
            self.knock_limit
        }
    }
}

impl Default for Rules {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets() {
        let modern = Rules::default();
        assert_eq!(modern, Rules::new());
        assert_eq!(modern.knock_limit, 10);
        assert_eq!(modern.gin_bonus, 25);
        assert_eq!(modern.big_gin_bonus, Some(31));
        assert_eq!(modern.undercut_bonus, 25);
        assert_eq!(modern.box_bonus, 25);
        assert!(!modern.immediate_boxes);
        assert_eq!(modern.game_bonus, 100);
        assert_eq!(modern.game_target, 100);
        assert_eq!(modern.shutout, Shutout::Double);

        let classic = Rules::classic();
        assert_eq!(classic.gin_bonus, 20);
        assert_eq!(classic.big_gin_bonus, None);
        assert_eq!(classic.undercut_bonus, 10);
        assert_eq!(classic.box_bonus, 20);
        assert!(!classic.immediate_boxes);
        assert_eq!(classic.shutout, Shutout::Flat(100));

        let palace = Rules::palace();
        assert_eq!(palace.gin_bonus, 25);
        assert_eq!(palace.big_gin_bonus, None);
        assert_eq!(palace.undercut_bonus, 20);
        assert_eq!(palace.box_bonus, 10);
        assert!(palace.immediate_boxes);
        assert_eq!(palace.game_bonus, 0);
        assert_eq!(palace.shutout, Shutout::Flat(0));

        for rules in [modern, classic, palace] {
            assert_eq!(rules.knock_limit, 10);
            assert_eq!(rules.oklahoma, None);
            assert!(rules.undercut_on_tie);
            assert_eq!(rules.game_target, 100);
        }
    }

    #[test]
    fn oklahoma_knock_limits() {
        let card = |s: &str| s.parse::<Card>().unwrap();
        let mut rules = Rules::default();
        assert_eq!(rules.knock_limit_for(card("7♦")), 10);

        rules.oklahoma = Some(OklahomaAce::One);
        assert_eq!(rules.knock_limit_for(card("7♦")), 7);
        assert_eq!(rules.knock_limit_for(card("Q♥")), 10);
        assert_eq!(rules.knock_limit_for(card("A♣")), 1);

        rules.oklahoma = Some(OklahomaAce::GinOnly);
        assert_eq!(rules.knock_limit_for(card("7♦")), 7);
        assert_eq!(rules.knock_limit_for(card("A♣")), 0);

        // The upcard only caps the limit; a stricter house limit stays.
        rules.knock_limit = 3;
        assert_eq!(rules.knock_limit_for(card("7♦")), 3);
    }
}
