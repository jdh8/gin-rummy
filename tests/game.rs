//! Match-level scoring fixtures across the rule presets

use gin_rummy::game::GameError;
use gin_rummy::{Game, Player, RoundResult, Rules, Shutout};

const fn knock(winner: Player, margin: u8) -> RoundResult {
    RoundResult::Knock { winner, margin }
}

#[test]
fn boxes_and_game_bonus_settle_at_the_end() {
    let mut game = Game::new(Rules::default(), Player::One);
    assert_eq!(game.next_dealer(), Player::One);

    // The winner of each hand deals the next.
    game.record(knock(Player::Two, 30)).unwrap();
    assert_eq!(game.next_dealer(), Player::Two);
    assert_eq!(game.score(Player::Two), 30);
    assert_eq!(game.boxes(Player::Two), 1);

    // A dead hand scores nothing and keeps the dealer.
    game.record(RoundResult::Dead).unwrap();
    assert_eq!(game.next_dealer(), Player::Two);
    assert_eq!(game.boxes(Player::One) + game.boxes(Player::Two), 1);

    game.record(knock(Player::One, 25)).unwrap();
    game.record(RoundResult::Undercut {
        winner: Player::One,
        margin: 5,
    })
    .unwrap();
    assert_eq!(game.score(Player::One), 25 + 30);

    assert!(!game.is_over());
    assert_eq!(game.winner(), None);
    assert_eq!(game.final_score(), None);

    game.record(RoundResult::Gin {
        winner: Player::Two,
        deadwood: 60,
    })
    .unwrap();

    // Two crossed 100: 30 + 85 = 115.
    assert!(game.is_over());
    assert_eq!(game.winner(), Some(Player::Two));
    assert_eq!(
        game.record(knock(Player::One, 1)).unwrap_err(),
        GameError::AlreadyOver,
    );

    // Boxes settle at 25 each; the winner adds the 100 game bonus.
    let along = game.final_score().unwrap();
    assert_eq!(along.winner, Player::Two);
    assert!(!along.shutout);
    assert_eq!(along.totals[Player::One as usize], 55 + 2 * 25);
    assert_eq!(along.totals[Player::Two as usize], 115 + 2 * 25 + 100);
}

#[test]
fn shutout_doubles_or_adds_flat() {
    // Modern rules double a shutout winner's total.
    let mut game = Game::new(Rules::default(), Player::One);
    game.record(knock(Player::One, 50)).unwrap();
    game.record(knock(Player::One, 50)).unwrap();

    let blitz = game.final_score().unwrap();
    assert!(blitz.shutout);
    assert_eq!(blitz.totals, [(100 + 2 * 25 + 100) * 2, 0]);

    // Classic rules add a flat 100 instead.
    let mut game = Game::new(Rules::classic(), Player::One);
    game.record(knock(Player::One, 50)).unwrap();
    game.record(knock(Player::One, 50)).unwrap();

    let blitz = game.final_score().unwrap();
    assert!(blitz.shutout);
    assert_eq!(blitz.totals, [100 + 2 * 20 + 100 + 100, 0]);

    // A single point spoils the shutout.
    let mut game = Game::new(Rules::default(), Player::One);
    game.record(knock(Player::Two, 1)).unwrap();
    game.record(knock(Player::One, 50)).unwrap();
    game.record(knock(Player::One, 50)).unwrap();

    let close = game.final_score().unwrap();
    assert!(!close.shutout);
    assert_eq!(close.totals, [100 + 2 * 25 + 100, 1 + 25]);
}

#[test]
fn immediate_boxes_shorten_the_game() {
    // Nine 10-point knocks: 90 raw points.  With palace scoring the ten
    // per-hand boxes are credited immediately, so the game ends at 90 + 90
    // ... in fact after five hands: 5 × (10 + 10) = 100.
    let results = [knock(Player::One, 10); 9];

    let mut palace = Game::new(Rules::palace(), Player::Two);
    let mut played = 0;
    for result in results {
        if palace.is_over() {
            break;
        }
        palace.record(result).unwrap();
        played += 1;
    }
    assert_eq!(played, 5);
    assert_eq!(palace.score(Player::One), 100);

    // The same sequence under traditional timing is still going after all
    // nine hands: boxes do not count toward the target.
    let mut modern = Game::new(Rules::default(), Player::Two);
    for result in results {
        modern.record(result).unwrap();
    }
    assert!(!modern.is_over());
    assert_eq!(modern.score(Player::One), 90);

    // Palace totals never double-count the boxes: 100 points, 5 boxes
    // already inside, no game bonus, and Flat(0) leaves the shutout alone.
    let settled = palace.final_score().unwrap();
    assert!(settled.shutout);
    assert_eq!(settled.totals, [100, 0]);
    assert_eq!(palace.boxes(Player::One), 5);
}

#[test]
fn big_gin_prices_by_rules() {
    let result = RoundResult::BigGin {
        winner: Player::One,
        deadwood: 20,
    };
    assert_eq!(result.points(&Rules::default()), 51);
    // Rules without big gin price a recorded one as plain gin.
    assert_eq!(result.points(&Rules::classic()), 40);

    let mut rules = Rules::default();
    rules.big_gin_bonus = Some(50);
    assert_eq!(result.points(&rules), 70);
    rules.shutout = Shutout::Flat(42);
    assert_eq!(rules.shutout, Shutout::Flat(42));
}
