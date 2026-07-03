# gin-rummy

This crate is a game engine for gin rummy: card and meld types, an optimal
deadwood solver, a per-deal `Round` state machine, and a `Game` scoreboard
with configurable scoring rules.  It mirrors the card-modeling patterns of my
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

After updating the codebase, please

- Format the code with `cargo fmt`.
- Run the tests with `cargo test --all-features`.
- Update [CHANGELOG.md](CHANGELOG.md) with a summary of the changes and their impact on users.
- Propose a clear and descriptive commit message.
