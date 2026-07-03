# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] — Unreleased

### Added

- Card primitives with ace-low encoding: `Suit`, `Rank` (A = 1 … K = 13),
  `Card`, the per-suit bitset `Holding`, and the 52-card bitset `Hand` with a
  documented `u64` layout (bit `16 × suit + rank`).
- Meld types and the exact deadwood solver: `Meld`, `Melds`, `best_melds`,
  `deadwood`, and `pip_sum`, built on a const-evaluated table of all 329
  possible melds.
- The `Round` state machine covering the full flow of a deal: the opening
  upcard offers, draw/discard turns, knocking, gin and big gin, layoffs with
  run chaining, the two-card dead-hand rule, and `RoundResult` scoring.
- The `Game` scoreboard: running scores, boxes, winner-deals-next rotation,
  game bonus, shutout handling, and `FinalScore`.
- `Rules` with modern tournament defaults and `classic()` (Bicycle) and
  `palace()` (Gin Rummy Palace) presets; every value is an independent knob,
  including box-bonus timing (end-tallied or credited per hand).
- Optional `rand` feature: a lazily shuffled `Deck` plus `Round::deal` and
  `Game::deal`.
- Optional `serde` feature: serialization for all public types, including
  validated round-trips of mid-game `Round` snapshots.
