# Design notes

This file records the decisions behind gin-rummy and the reasoning the code
cannot express.  API details belong in rustdoc; procedures belong in
`.claude/skills/`; this is the *why*.  When changing the crate, treat these
decisions as settled unless the change explicitly revisits them.

## Scope and positioning

- This is a **game-mechanism library**, not an engine: it adjudicates rules
  and scores.  Move selection, opponent modeling, and search belong to a
  planned `gin-rummy-engine` crate that will depend on this one.  The
  deadwood solver stays *here* regardless — it defines legality (a knock's
  deadwood) and scoring, not strategy.
- A new crate was warranted: the `rummy` crate on crates.io models basic
  rummy (public melding, race to empty the hand), and its action model
  structurally cannot host gin — concealed hands, knock/deadwood,
  end-of-round layoffs, and the upcard-pass protocol have no place in it.
  No gin rummy library existed on crates.io, so the name and niche were
  free.
- **Standalone, no dependency on contract-bridge.**  Its `Rank` is ace-HIGH
  (2..=14) and its `Holding` bit positions encode those rank values, so the
  entire bitset stack had to be re-encoded ace-low anyway.  The two crates
  share *patterns* (suit type, bitset holdings, iterator shapes), never
  code.  `pons` does not re-export contract-bridge either; standalone
  matches the ecosystem shape.

## Card core

- A `Hand` is four 16-bit suit lanes in one `u64`: the card of suit `s` and
  rank `r` is bit `16s + r`, so `Hand::ALL.to_bits() ==
  0x3FFE_3FFE_3FFE_3FFE`.  This layout is documented as a **stable API
  contract** — melds, subset tests, and the solver all operate on it, and
  downstream bots are expected to as well.
- `to_bits`/`from_bits_retain` are explicit shift compositions, not a
  transmute: endian-safe and unsafe-free.
- `Rank` is `NonZero<u8>` so `Option<Rank>` is free; the ace-low `1..=13`
  encoding makes run arithmetic trivial (no wrap: bit positions cannot
  extend below A or above K).
- The text format is dotted suit groups, **clubs first, ascending**:
  `"A23.456.789.T"`, empty hand `"-"`, empty suit group an empty string.
  This deliberately differs from the descending, spades-first PBN format of
  bridge software; gin reads low-to-high.  Display writes `T`; parsing also
  accepts `10`.  Suit parsing accepts letters and both filled and outlined
  glyphs and strips Unicode variation selectors.

## Melds and the solver

- `Meld` is a `#[repr(transparent)]` newtype over `Hand`, so "does this
  hand contain this meld" is `meld & hand == meld` — one bitwise test in
  the shared u64 layout.
- `Melds` (an arrangement) holds at most **3** melds: 4 melds require at
  least 12 cards, and a gin hand never exceeds 11.
- All **329** possible melds are precomputed in a const table: 65 sets (13
  ranks × one 4-card + four 3-card) and 264 runs (4 suits × Σ of 11+10+…+1
  = 66 spans).  Const `while` loops, no lazy statics, no build script.  The
  fixed table order makes the search deterministic.
- The solver follows Todd Neller's EAAI gin rummy framework: filter the
  table to melds applicable to the hand (subset test), then branch and
  bound on the lowest set bit — the lowest card is either deadwood or
  starts one of its melds — pruning when the accumulated pip value reaches
  the best found.  `deadwood()` is the lean variant (no meld tracking, so
  the 3-meld depth cap is irrelevant: packings differing only in melds
  score alike); `best_melds()` additionally records the chosen melds.
  Game-size hands take a few hundred nanoseconds (`benches/deadwood.rs`).
- Numeric bounds, argued once so nobody widens or narrows types blindly:
  - `pip_sum` returns `u16` because the full deck totals **340**, which
    overflows u8.
  - `deadwood` returns `u8` because an optimal remainder is meld-free
    (melding a leftover meld would only shrink the deadwood), and the most
    valuable meld-free card set is worth **170**: per suit, no three
    consecutive ranks caps the value at A 3 4 6 7 9 T Q K = 60; at most two
    copies of each rank (else a set forms) leaves the other two suits the
    complementary 2 5 8 J = 25; 2×60 + 2×25 = 170 < 256.
- Which optimal arrangement `best_melds` returns is **unspecified** and may
  change between releases; only optimality and disjointness are promised.
  The tie-break is whatever the deterministic search order yields.
- Correctness insurance: `tests/proptest.rs` cross-checks `deadwood`
  against an independent brute-force solver (kept naive on purpose — it
  must not share code or cleverness with the real one), and
  `tests/meld.rs` pins hand-computed literature cases.

## The `Round` state machine

- **Runtime-checked, not typestate.**  Phases branch dynamically (gin skips
  layoffs, a discard may end in a dead hand, big gin ends immediately), UIs
  want a single `Round` type to hold, and serde wants one snapshot shape.
  House precedent: contract-bridge's `Auction::push -> Result`.  Illegal
  actions return `RoundError`; they never panic.
- **The stock is an ordered `Vec<Card>`, not an rng-driven `Deck`.**  The
  discard pile must be ordered anyway; an ordered stock makes `Round`
  self-contained and deterministic — serde round-trips reproduce the game
  exactly, and bots can tree-search a cloned round without an rng.  `rand`
  stays a deal-time-only dependency (`Round::deal`, `Game::deal`, `Deck`).
  Top of both piles is the *last* element.
- **Omniscient accessors by design**: `hand()` for either player and the
  full `stock()` order are public.  Hiding information is a UI/bot concern
  with many right answers (hotseat, network play, sampling bots); baking
  one into the library would be wrong for the others.
- **The knocker passes the spread explicitly** to `knock(card, melds)`
  because the arrangement fixes what the defender may lay off, and keeping
  a worse arrangement can be strategically correct.  The single equality
  check `melds.hand() == hand - card` covers provenance, disjointness, and
  consistency at once; `best_melds(hand - card.into())` recovers the
  automatic choice and is shown in the docs.
- Gin (0-deadwood knock) goes straight to `Finished` — "no layoffs against
  gin" falls out structurally rather than being a rule check.
- **Big gin is never a trap**: any partition of 11 cards into melds (parts
  of at least 3) contains a part of at least 4, and a 4-card meld can shed
  a card, so plain gin remains available when `Rules::big_gin_bonus` is
  `None` and `declare_big_gin` is rejected.
- **Dead hand**: the check lives at the end of a non-knock `discard` (stock
  down to 2 cards ⇒ `RoundResult::Dead`).  Consequently no turn ever starts
  with a 2-card stock, which is why `draw_stock`'s `expect` is unreachable.
  A *knock* that leaves 2 stock cards still scores.
- The `taken_discard` field enforces "may not discard the card taken from
  the pile this turn"; both `discard` and `knock` respect it, and it clears
  on every shed or stock draw.
- Layoff melds are addressed by **stable index** into the spread so chained
  extensions (the ♠8 then the ♠9 onto 5-6-7) target the same meld;
  `Meld::extended` is the primitive and naturally rejects 4-card sets and
  out-of-range run ends.

## Serde: validated snapshots

- `Round` serializes through `round::repr::RoundRepr`
  (`#[serde(into/try_from)]`).  Deserialization **re-validates everything**:
  the 52-card partition (hands, stock, pile, layoffs — each card exactly
  once), per-phase card counts, upcard-pass consistency, spread validity
  against the knocker's cards, the knock limit, and that a recorded result
  matches the recomputed score.  A corrupt snapshot is rejected, never
  trusted; `KnockState::knocker_deadwood` is *recomputed*, not read.
- Checklist for adding a field to `Round`: mirror it in `RoundRepr`, map it
  in `From<Round>`, decide in `TryFrom` which phases allow it to be
  non-default, and add both a round-trip and a corruption case to
  `tests/serde.rs`.

## Scoring split

- `RoundResult` records **facts** (who, margin or deadwood);
  `RoundResult::points(&Rules)` prices them; `Game` owns everything that
  spans rounds — boxes, the game bonus, the shutout rule, and the
  winner-deals-next rotation.  This keeps a `RoundResult` replayable under
  different rules.
- A `BigGin` result priced under rules with `big_gin_bonus: None` falls
  back to the gin bonus, so results recorded under one rule school stay
  meaningful under another.
- `Rules::immediate_boxes` exists because Gin Rummy Palace credits the box
  bonus into the running score as each hand is won (so boxes count toward
  the target and games end sooner), while traditional play tallies boxes
  only at game end.  `Game::record` credits immediately when set;
  `Game::final_score` adds end-tallied boxes only when *not* set — never
  both.  The knob was a user request; the author plays on Gin Rummy Palace.
- `Rules` is `#[non_exhaustive]` with public fields: construct via a preset
  and mutate (`rules.game_target = 250;`), never via a struct literal.
  Adding a knob is therefore a minor, non-breaking change.

## Variant roadmap (planned, all non-breaking)

- **Oklahoma gin**: a new `Rules` field; `Round` resolves the effective
  knock limit from the upcard.  `Round::knock_limit()` already exists as
  the per-round resolution hook — callers are told to use it instead of
  `Rules::knock_limit`.  `RoundResult` is `#[non_exhaustive]` partly to
  leave room for an Oklahoma spade-doubling variant.
- **Straight gin** is approximately `knock_limit: 0` (knock only on gin);
  check school-specific details before claiming full support.
- See the `add-variant` skill for the mechanical checklist.

## Testing strategy

- `tests/fmt.rs` — Display/FromStr fixtures, including `"H10"`, variation
  selectors, `"-"`, empty suit groups, and error variants.
- `tests/meld.rs` — hand-computed deadwood cases from the literature
  (set/run conflicts, 11-card covers 3+4+4 and 3+3+5, single-suit runs).
- `tests/round.rs` — scripted deals via `from_deal` (feature-free);
  every `RoundError` variant is reachable.
- `tests/game.rs` — scoring fixtures across all three presets, box timing,
  shutouts, dealer rotation.
- `tests/proptest.rs` — text round-trips, hand set-algebra, the solver
  cross-check, `best_melds` invariants, and random legal playouts
  asserting card conservation (52 cards, each in exactly one place) after
  every action and eventual termination.
- `tests/serde.rs` — encoding-shape fixtures per type, mid-game `Round`
  round-trips, and corrupt-snapshot rejection.
- New behavior should extend the matching file; new `Round` state must show
  up in the playout driver's `step` and `conserved` checks.

## Known warts

- The crates.io **0.1.0 artifact predates the docs reframe** (commit
  `fbe0f18`, "game engine" → game-mechanism library), so its bundled
  description and README still say "game engine".  Fixed by the next
  publish; the repository and docs.rs-from-source are already correct.
