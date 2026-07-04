//! Print the best melds and deadwood of hands given on the command line.
//!
//! A hand is four dot-separated ascending suit groups, clubs first:
//!
//! ```console
//! $ cargo run --example deadwood -- "A23.456.789.T"
//! A23.456.789.T: A♣2♣3♣ 4♦5♦6♦ 7♥8♥9♥ | T♠ (10 deadwood)
//! ```

use gin_rummy::{Hand, best_melds};
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.is_empty() {
        eprintln!("Usage: deadwood <HAND>...");
        eprintln!();
        eprintln!("A hand is four dot-separated ascending suit groups, clubs first,");
        eprintln!("e.g. \"A23.456.789.T\" for ♣A23 ♦456 ♥789 ♠10.");
        return ExitCode::FAILURE;
    }

    let mut status = ExitCode::SUCCESS;
    for arg in &args {
        match arg.parse::<Hand>() {
            Ok(hand) if hand.len() > 11 => {
                eprintln!("{arg}: more than 11 cards; gin rummy hands hold 10 or 11");
                status = ExitCode::FAILURE;
            }
            Ok(hand) => {
                let melds = best_melds(hand);
                println!("{arg}: {melds} ({} deadwood)", melds.deadwood());
            }
            Err(error) => {
                eprintln!("{arg}: {error}");
                status = ExitCode::FAILURE;
            }
        }
    }
    status
}
