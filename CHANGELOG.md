# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.2] — 2026-07-05

### Changed

- `best_melds` now breaks equal-deadwood ties in favor of runs over sets, so
  a hand like `5♠6♠7♠` overlapping three sixes spreads the run rather than the
  set — a run gins more readily because it extends at both ends.  The tie-break
  remains documented as unspecified, and `deadwood` is unaffected (it stays a
  pure-pip search that never records an arrangement).

## [0.1.1] — 2026-07-05

### Changed

- `Card` now displays rank-first (`T♥` instead of `♥T`), since in gin rummy,
  as in poker, the rank carries most of the information.  This is the text and
  serde form, so `Meld` and `Melds` follow suit (`5♠6♠7♠`, `Q♣Q♦Q♠`).  Parsing
  is unchanged in spirit but now accepts either order, so existing suit-first
  text and snapshots still round-trip.

### Internal

- Recorded the design rationale — scope, bitset layout, solver bounds,
  `Round` and serde-validation decisions, and the variant roadmap — in
  `docs/DESIGN.md`.
- Expanded `CLAUDE.md` into a maintainer handbook: crate map, invariants,
  house style, and a CI-mirroring verification checklist.
- Added project skills for the release ritual and for adding rule variants
  under `.claude/skills/`.

## [0.1.0] — 2026-07-04

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
