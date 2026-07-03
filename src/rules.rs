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
//! variants (an Oklahoma knock limit, straight gin) without breakage.

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

/// The scoring rules of a game
///
/// All fields are public knobs; see the [module documentation](self) for the
/// preset-and-adjust pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[non_exhaustive]
pub struct Rules {
    /// The most deadwood a knocker may keep (10 in every common school)
    pub knock_limit: u8,

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
            assert!(rules.undercut_on_tie);
            assert_eq!(rules.game_target, 100);
        }
    }
}
