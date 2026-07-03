//! Greedy bots play a full game of gin rummy and narrate the score.
//!
//! Each bot draws from the pile only when that strictly lowers its deadwood,
//! sheds the discard minimizing deadwood, knocks (or declares gin and big
//! gin) as soon as the rules allow, and lays off every card that fits.
//!
//! ```console
//! cargo run --features rand --example simulate
//! ```

use anyhow::{Context as _, Result};
use gin_rummy::{Card, Game, Hand, Phase, Player, Round, RoundResult, Rules, best_melds, deadwood};

fn main() -> Result<()> {
    let mut rng = rand::rng();
    let mut game = Game::new(Rules::default(), Player::One);

    for number in 1.. {
        if game.is_over() {
            break;
        }

        let mut round = game.deal(&mut rng);
        play(&mut round)?;
        let result = round.result().context("the bots play every round out")?;

        game.record(result)?;
        println!("Round {number}: {}", describe(result));
        println!(
            "  score {} : {}, dealer {}",
            game.score(Player::One),
            game.score(Player::Two),
            game.next_dealer(),
        );
    }

    let settled = game.final_score().context("the game just ended")?;
    println!();
    println!(
        "{} wins {} : {}",
        settled.winner,
        settled.totals[settled.winner as usize],
        settled.totals[settled.winner.opponent() as usize],
    );
    if settled.shutout {
        println!("A shutout!");
    }
    Ok(())
}

/// Drive one round to completion with both seats played greedily.
fn play(round: &mut Round) -> Result<()> {
    let mut taken: Option<Card> = None;

    while round.result().is_none() {
        let player = round.turn().context("an unfinished round has a turn")?;
        let hand = round.hand(player);

        match round.phase() {
            Phase::Upcard => {
                let top = pile_top(round)?;
                if improves(hand, top) {
                    taken = Some(round.take_discard()?);
                } else {
                    round.pass()?;
                }
            }
            Phase::Draw => {
                // Taking the pile top can be refused (the forced first
                // draw); fall back to the stock.
                if improves(hand, pile_top(round)?)
                    && let Ok(card) = round.take_discard()
                {
                    taken = Some(card);
                } else {
                    round.draw_stock()?;
                    taken = None;
                }
            }
            Phase::Discard => {
                if deadwood(hand) == 0 && round.rules().big_gin_bonus.is_some() {
                    round.declare_big_gin(best_melds(hand))?;
                } else {
                    let (card, rest) = best_shed(hand, taken);
                    if rest <= round.knock_limit() {
                        round.knock(card, best_melds(hand - card.into()))?;
                    } else {
                        round.discard(card)?;
                    }
                    taken = None;
                }
            }
            Phase::Layoff => {
                let laid = round
                    .hand(player)
                    .iter()
                    .find(|&card| (0..3).any(|index| round.lay_off(card, index).is_ok()));
                if laid.is_none() {
                    round.finish_layoffs()?;
                }
            }
            Phase::Finished => unreachable!("the loop exits on finished rounds"),
        }
    }
    Ok(())
}

fn pile_top(round: &Round) -> Result<Card> {
    round
        .discard_pile()
        .last()
        .copied()
        .context("the discard pile is never empty on a draw decision")
}

/// Whether taking `top` strictly lowers deadwood after the best discard
/// (which may not be `top` itself).
fn improves(hand: Hand, top: Card) -> bool {
    let with = hand | top.into();
    let (_, rest) = best_shed(with, Some(top));
    rest < deadwood(hand)
}

/// The discard leaving the least deadwood, skipping the just-taken card.
fn best_shed(hand: Hand, taken: Option<Card>) -> (Card, u8) {
    hand.iter()
        .filter(|&card| Some(card) != taken)
        .map(|card| (card, deadwood(hand - card.into())))
        .min_by_key(|&(card, rest)| (rest, u8::MAX - card.rank.deadwood()))
        .expect("an 11-card hand always has a legal discard")
}

fn describe(result: RoundResult) -> String {
    match result {
        RoundResult::Dead => "dead hand, no score".into(),
        RoundResult::Knock { winner, margin } => format!("{winner} knocks for {margin}"),
        RoundResult::Undercut { winner, margin } => format!("{winner} undercuts by {margin}"),
        RoundResult::Gin { winner, deadwood } => format!("{winner} goes gin (+{deadwood})"),
        RoundResult::BigGin { winner, deadwood } => format!("{winner} goes BIG gin (+{deadwood})"),
        _ => format!("{result:?}"),
    }
}
