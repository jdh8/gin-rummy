# Gin Rummy

[![Crates.io](https://img.shields.io/crates/v/gin-rummy)](https://crates.io/crates/gin-rummy)
[![Docs.rs](https://docs.rs/gin-rummy/badge.svg)](https://docs.rs/gin-rummy)
[![Build Status](https://github.com/jdh8/gin-rummy/actions/workflows/rust.yml/badge.svg)](https://github.com/jdh8/gin-rummy)

This crate models the mechanics of [gin rummy]: strongly typed cards and melds,
an exact deadwood solver, and a rules-driven state machine for complete
two-player rounds and games.

Gin rummy ranks the ace LOW: A-2-3 is a run, Q-K-A is not, and an ace counts
one point of deadwood.  `Rank` therefore encodes A = 1 through K = 13, unlike
crates for ace-high games such as [contract-bridge] whose patterns this crate
otherwise follows.

Hands are written as four dot-separated suit groups in ascending order,
clubs first: `"A23.456.789.T"` holds ♣A ♣2 ♣3 ♦4 ♦5 ♦6 ♥7 ♥8 ♥9 ♠10.

## Modules

- [`hand`][mod-hand]: cards and card sets — [`Suit`], [`Rank`], [`Card`],
  [`Holding`], [`Hand`]
- [`meld`][mod-meld]: sets, runs, and the deadwood solver — [`Meld`],
  [`Melds`], [`best_melds`], [`deadwood`]
- [`round`][mod-round]: one deal from upcard to showdown — [`Round`],
  [`Phase`], [`RoundResult`]
- [`game`][mod-game]: the scoreboard across deals — [`Game`], [`FinalScore`]
- [`rules`][mod-rules]: scoring configuration — [`Rules`], [`Shutout`]
- [`deck`][mod-deck] (feature `rand`): shuffled dealing — [`Deck`]

## Feature flags

- `rand`: shuffled dealing (`Deck`, `Round::deal`, `Game::deal`)
- `serde`: serialization for every public type, with validated
  deserialization of mid-game [`Round`] snapshots

## Quick start

Deadwood analysis needs no features:

```rust
use gin_rummy::{best_melds, deadwood};

let hand = "A23.456.789.T".parse::<gin_rummy::Hand>()?;
assert_eq!(deadwood(hand), 10);
println!("{}", best_melds(hand));
# Ok::<(), gin_rummy::hand::ParseHandError>(())
```

A complete bot-vs-bot game with the `rand` feature:

```rust
# #[cfg(feature = "rand")]
# fn main() {
use gin_rummy::{Game, Player, Rules};

let mut game = Game::new(Rules::default(), Player::One);
while !game.is_over() {
    let mut round = game.deal(&mut rand::rng());
    drive(&mut round); // pass / draw / discard / knock / lay_off …
    game.record(round.result().expect("round finished")).unwrap();
}
println!("{:?}", game.final_score());
# }
# #[cfg(feature = "rand")]
# fn drive(round: &mut gin_rummy::Round) {
#     use gin_rummy::{Phase, best_melds, deadwood};
#     while round.result().is_none() {
#         match round.phase() {
#             Phase::Upcard => round.pass().unwrap(),
#             Phase::Draw => {
#                 round.draw_stock().unwrap();
#             }
#             Phase::Discard => {
#                 let hand = round.hand(round.turn().unwrap());
#                 let (card, rest) = hand
#                     .iter()
#                     .map(|card| (card, deadwood(hand - card.into())))
#                     .min_by_key(|&(_, rest)| rest)
#                     .unwrap();
#                 if rest <= round.knock_limit() {
#                     round.knock(card, best_melds(hand - card.into())).unwrap();
#                 } else {
#                     round.discard(card).unwrap();
#                 }
#             }
#             Phase::Layoff => {
#                 round.finish_layoffs().unwrap();
#             }
#             Phase::Finished => unreachable!(),
#         }
#     }
# }
# #[cfg(not(feature = "rand"))]
# fn main() {}
```

## Examples

- `deadwood`: parse hands from the command line and print their best melds —
  `cargo run --example deadwood -- "45.456.567.789"`
- `simulate`: greedy bots play a full game —
  `cargo run --features rand --example simulate`

[gin rummy]: https://www.pagat.com/rummy/ginrummy.html
[contract-bridge]: https://crates.io/crates/contract-bridge
[mod-hand]: https://docs.rs/gin-rummy/latest/gin_rummy/hand/
[mod-meld]: https://docs.rs/gin-rummy/latest/gin_rummy/meld/
[mod-round]: https://docs.rs/gin-rummy/latest/gin_rummy/round/
[mod-game]: https://docs.rs/gin-rummy/latest/gin_rummy/game/
[mod-rules]: https://docs.rs/gin-rummy/latest/gin_rummy/rules/
[mod-deck]: https://docs.rs/gin-rummy/latest/gin_rummy/deck/
[`Suit`]: https://docs.rs/gin-rummy/latest/gin_rummy/enum.Suit.html
[`Rank`]: https://docs.rs/gin-rummy/latest/gin_rummy/hand/struct.Rank.html
[`Card`]: https://docs.rs/gin-rummy/latest/gin_rummy/hand/struct.Card.html
[`Holding`]: https://docs.rs/gin-rummy/latest/gin_rummy/hand/struct.Holding.html
[`Hand`]: https://docs.rs/gin-rummy/latest/gin_rummy/hand/struct.Hand.html
[`Meld`]: https://docs.rs/gin-rummy/latest/gin_rummy/meld/struct.Meld.html
[`Melds`]: https://docs.rs/gin-rummy/latest/gin_rummy/meld/struct.Melds.html
[`best_melds`]: https://docs.rs/gin-rummy/latest/gin_rummy/meld/fn.best_melds.html
[`deadwood`]: https://docs.rs/gin-rummy/latest/gin_rummy/meld/fn.deadwood.html
[`Round`]: https://docs.rs/gin-rummy/latest/gin_rummy/round/struct.Round.html
[`Phase`]: https://docs.rs/gin-rummy/latest/gin_rummy/round/enum.Phase.html
[`RoundResult`]: https://docs.rs/gin-rummy/latest/gin_rummy/round/enum.RoundResult.html
[`Game`]: https://docs.rs/gin-rummy/latest/gin_rummy/game/struct.Game.html
[`FinalScore`]: https://docs.rs/gin-rummy/latest/gin_rummy/game/struct.FinalScore.html
[`Rules`]: https://docs.rs/gin-rummy/latest/gin_rummy/rules/struct.Rules.html
[`Shutout`]: https://docs.rs/gin-rummy/latest/gin_rummy/rules/enum.Shutout.html
[`Deck`]: https://docs.rs/gin-rummy/latest/gin_rummy/deck/struct.Deck.html
