---
name: add-variant
description: Add a gin rummy rule variant or scoring knob (e.g. Oklahoma, straight gin, a new bonus) to Rules and thread it through Round, RoundResult, Game, serde, tests, and the changelog. Use when asked to support a new rule school, scoring option, or gameplay variant.
---

# Adding a rules knob or variant

`Rules` is `#[non_exhaustive]` with public fields precisely so variants land
as **minor, non-breaking** changes.  Work through this checklist top to
bottom; docs/DESIGN.md ("Variant roadmap", "Scoring split") has the
rationale behind each rule here.

## 1. Research the rule first

Scoring bonuses and legality details vary by rule school.  Cross-check
[Pagat](https://www.pagat.com/rummy/ginrummy.html) (most reliable) against
Wikipedia and Bicycle before hardcoding any number, and record per-school
values in the field's rustdoc, like the existing knobs do.

## 2. Extend `Rules` (src/rules.rs)

- Pick the established shapes: `u16` for bonuses, `bool` for toggles,
  `Option<u16>` for a bonus whose absence disables the action (see
  `big_gin_bonus`).
- Set the value in **all three presets deliberately** — `new()` (modern
  tournament), `classic()` (Bicycle), `palace()` (Gin Rummy Palace) — and
  extend the preset assertions in the `rules.rs` tests.  Do not let a
  preset inherit a default unexamined.

## 3. Thread the behavior — respect the layering

- **Legality** lives in `Round` actions.  For anything knock-limit-shaped,
  resolve through `Round::knock_limit()` — it exists as the per-round hook
  (Oklahoma reads the upcard there).
- **Pricing** lives in `RoundResult::points(&Rules)`.  `RoundResult`
  records *facts* (who won, margins, deadwood) and must stay replayable
  under different rules; a result recorded under one school should degrade
  gracefully under another (see how `BigGin` falls back to the gin bonus).
- **Cross-round effects** (boxes, game bonus, shutout, dealer rotation)
  live in `Game`.
- New `RoundResult` variants are allowed (`#[non_exhaustive]`), but update
  `winner()`, `points()`, the `describe` helper in `examples/simulate.rs`,
  and the result match in `tests/proptest.rs`.  Avoid new `Phase` variants:
  `Phase` is exhaustive and that is a breaking change.

## 4. Serde, if `Round` gains state

Any new `Round` field must be threaded through `round::repr::RoundRepr`,
`From<Round>`, and the `TryFrom` validation (decide which phases allow a
non-default value), or corrupt snapshots become accepted.  Add both a
round-trip and a corruption fixture to `tests/serde.rs`.

## 5. Tests

- Preset values: `src/rules.rs` inline tests.
- Legality: scripted deals in `tests/round.rs` (every error variant of the
  new action should be reachable).
- Scoring: fixtures in `tests/game.rs`, including interaction with
  `immediate_boxes` and the shutout rule when relevant.
- If the variant adds actions or state, teach the random-playout driver in
  `tests/proptest.rs` (`step`) to exercise it and keep the `conserved`
  card-conservation check true after it.

## 6. Wrap up

- Rustdoc on every new public item (`#![warn(missing_docs)]` enforces it);
  mention the variant in the module docs of `rules.rs` if it adds a preset.
- Run the full verification gate from CLAUDE.md.
- CHANGELOG entry under `### Added`; this is a minor version.
- Propose a commit message.
