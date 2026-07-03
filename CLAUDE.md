# gin-rummy

This crate implements the game mechanics of gin rummy: card and meld types, an
optimal deadwood solver, a per-deal `Round` state machine, and a `Game`
scoreboard with configurable scoring rules.  Gameplay strategy (move selection,
opponent AI) is out of scope here and is planned for a separate
`gin-rummy-engine` crate.  It mirrors the card-modeling patterns of my
[contract-bridge](https://crates.io/crates/contract-bridge) crate, but the
rank encoding here is ace-LOW (A = 1, K = 13) because gin runs are A-2-3 and
never Q-K-A; the two crates deliberately share no code.

Domain vocabulary: a *meld* is a set (3-4 of a rank) or a run (3+ consecutive
cards of a suit); *deadwood* is the pip total of unmelded cards; a player
*knocks* to end a round with deadwood at or under the knock limit, and *gin*
means knocking with zero deadwood.  After a knock (but never after gin), the
defender may *lay off* cards onto the knocker's melds.  For rules questions,
[Pagat](https://www.pagat.com/rummy/ginrummy.html) is the most reliable
source; scoring bonuses vary by rule school and are all knobs on `Rules`.

Design rationale the code cannot show — why APIs look the way they do,
rejected alternatives, the numeric-bound arguments, and the variant roadmap —
lives in [docs/DESIGN.md](docs/DESIGN.md).  Read it before changing public
API, the solver, or `Round`/serde internals, and follow it rather than
re-deriving decisions.  Recurring procedures are project skills under
[.claude/skills/](.claude/skills/): `release` (cutting a version) and
`add-variant` (threading a new rules knob through the crate).

## Map of the crate

- `src/lib.rs` — `Suit`, crate lints, module declarations, flat re-exports.
  The crate doc is `README.md` via `include_str!`, so README doctests must
  compile both with and without features.
- `src/hand.rs` — `Rank` (ace-low `NonZero<u8>`, 1..=13), `Card`, `Holding`
  (u16 bitset of ranks in one suit), `Hand` (`[Holding; 4]` in one u64).
- `src/meld.rs` — `Meld` (validated card set), `Melds` (arrangement into at
  most 3 disjoint melds plus deadwood), the const table of all 329 melds,
  and the solver: `best_melds`, `deadwood`, `pip_sum`.
- `src/player.rs` — `Player::{One, Two}`; indexes per-player arrays.
- `src/rules.rs` — `Rules` scoring knobs; presets `new()` (modern
  tournament, the default), `classic()` (Bicycle), `palace()` (Gin Rummy
  Palace); `Shutout`.
- `src/round.rs` — `Phase`, `Round` (runtime-checked state machine),
  `RoundResult`, the action errors, and the serde `RoundRepr` that
  re-validates snapshots.
- `src/game.rs` — `Game` scoreboard and `FinalScore`.
- `src/deck.rs` (feature `rand`) — `Deck`, bit-trick random drawing.
- `tests/` — `fmt` (Display/FromStr fixtures), `meld` (hand-computed
  deadwood cases), `round` (scripted deals reaching every `RoundError`),
  `game` (scoring across all presets), `proptest` (text round-trips, solver
  vs. an independent brute force, random playouts with card conservation),
  `serde` (encoding shapes, corrupt-snapshot rejection).

## Invariants — do not break; docs/DESIGN.md has the arguments

- The `u64` layout of `Hand` — the card of suit `s` and rank `r` is bit
  `16s + r` — is a documented, stable API contract.  Bit 0 and bits 14-15 of
  each 16-bit suit lane are always zero.
- The ace is LOW everywhere; `Rank::deadwood()` is A = 1, spot cards pip,
  T/J/Q/K = 10.
- `pip_sum` returns `u16` (the full deck is 340 points); `deadwood` returns
  `u8` (an optimal remainder is meld-free, and the largest meld-free set is
  worth 170).  `best_melds` panics above 11 cards; `deadwood` and `pip_sum`
  accept any card set.
- Which optimal arrangement `best_melds` returns is documented as
  unspecified — retuning the tie-break is not a breaking change — but its
  disjointness and optimality invariants are property-tested.
- The knocker's spread is fixed at `knock()` time because it determines what
  the defender may lay off.  Layoffs only extend spread melds (chaining
  allowed, indices stable) and never form new melds.  Gin and big gin skip
  the layoff phase entirely.
- A dead hand triggers when a non-knock discard leaves 2 stock cards, so no
  turn ever starts with a 2-card stock and `draw_stock` cannot empty it.
  Knocking that leaves 2 stock cards still scores.
- A card taken from the discard pile cannot be discarded the same turn.
- `Round` accessors expose both hands and the stock order on purpose; bots
  and UIs enforce their own information hygiene.
- Deserializing a `Round` re-validates every invariant in `round::repr`.  A
  new `Round` field must be threaded through `RoundRepr`, its `TryFrom`
  validation, and `tests/serde.rs`, or corrupt snapshots become accepted.
- `Rules`, `Shutout`, `RoundResult`, `FinalScore`, and the error enums are
  `#[non_exhaustive]`; `Phase` is deliberately exhaustive because UIs match
  on it, so adding a phase is a breaking change.
- The winner of a hand deals the next; a dead hand is redealt by the same
  dealer.  Only the round winner gains points, so exactly one player can
  reach the game target.

## House style

- Derive order `Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash`,
  plus `Default` when meaningful; `#[must_use]`, `#[inline]`, and `const fn`
  on small methods; `#![warn(missing_docs)]` covers every public item.
- Errors use thiserror v2: unit structs for simple parse failures,
  `#[non_exhaustive]` enums with `#[from]` for composites.
- Panicking constructors pair with fallible `try_` twins and document the
  panic (const contexts turn it into a compile-time error); every `unsafe`
  carries a `SAFETY:` comment.
- Pedantic clippy is kept clean locally but NOT enforced in CI; any
  crate-level `#![allow(clippy::…)]` needs a comment justifying it.
- serde uses three encodings: variant-name derives for fieldless enums,
  numeric `try_from`/`into` for `Rank`, and `serde_with`'s Display/FromStr
  for the text-format types (`Card`, `Holding`, `Hand`, `Meld`, `Deck`).
- Prose — docs, comments, CHANGELOG — uses double spaces after periods.

## Verification — mirror CI before declaring work done

CI (`.github/workflows/rust.yml`) runs five jobs; reproduce them locally:

```sh
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --all-features
cargo test --all-features
cargo test  # no-features build, including the README doctests
```

When `Cargo.toml` dependencies change, also run the minimal-versions check:
`cargo +nightly update -Z direct-minimal-versions && cargo +nightly check
--all-features --all-targets`, then restore `Cargo.lock` (it is committed).
The MSRV (`rust-version = "1.93"`) lives in TWO places — `Cargo.toml` and
the CI test matrix — and stays out of the README on purpose.

## After updating the codebase

- Format the code with `cargo fmt`.
- Run the tests with `cargo test --all-features`, plus the rest of the
  verification gate above for anything nontrivial.
- Update [CHANGELOG.md](CHANGELOG.md) with a summary of the changes and
  their impact on users.  Keep-a-Changelog format; the pending section is
  headed `## [X.Y.Z] — Unreleased` (em dash), and maintainer-only changes
  go under a custom `### Internal` heading.
- Propose a clear and descriptive commit message.
