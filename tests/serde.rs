#![cfg(feature = "serde")]
//! Serialization shapes and validated `Round` snapshots

use gin_rummy::{
    Card, Game, Hand, Holding, Meld, Phase, Player, Rank, Round, RoundResult, Rules, Suit,
    best_melds,
};
use serde_json::{Value, json};

#[test]
fn encoding_shapes() {
    // Fieldless enums serialize by variant name.
    assert_eq!(json!(Suit::Spades), json!("Spades"));
    assert_eq!(json!(Player::One), json!("One"));
    assert_eq!(json!(Phase::Layoff), json!("Layoff"));

    // Rank is a transparent number.
    assert_eq!(json!(Rank::K), json!(13));

    // Display-backed types serialize as their text form.
    assert_eq!(json!("♠A".parse::<Card>().unwrap()), json!("A♠"));
    assert_eq!(json!("A23".parse::<Holding>().unwrap()), json!("A23"));
    assert_eq!(
        json!("A23.456.789.T".parse::<Hand>().unwrap()),
        json!("A23.456.789.T")
    );
    assert_eq!(json!("♠5♠6♠7".parse::<Meld>().unwrap()), json!("5♠6♠7♠"));

    // Aggregates serialize by field.
    let result = RoundResult::Gin {
        winner: Player::Two,
        deadwood: 42,
    };
    assert_eq!(
        json!(result),
        json!({"Gin": {"winner": "Two", "deadwood": 42}})
    );
    assert_eq!(json!(Rules::default())["gin_bonus"], json!(25));

    // And they all round-trip.
    let game = Game::new(Rules::palace(), Player::Two);
    assert_eq!(serde_json::from_value::<Game>(json!(game)).unwrap(), game,);
    assert_eq!(
        serde_json::from_value::<Rank>(json!(Rank::A)).unwrap(),
        Rank::A,
    );
    assert!(serde_json::from_value::<Rank>(json!(14)).is_err());
    assert!(serde_json::from_value::<Hand>(json!("32A...")).is_err());
}

fn card(s: &str) -> Card {
    s.parse().unwrap()
}

/// A fresh deal still in the upcard phase, with the ♠J on offer.
fn fresh_round() -> Round {
    let hands: [Hand; 2] = [
        "A23.456.789.T".parse().unwrap(),
        "45.89J.JQK.23".parse().unwrap(),
    ];
    let upcard = card("♠J");
    let stock: Vec<Card> = Hand::ALL
        .iter()
        .filter(|&c| !hands[0].contains(c) && !hands[1].contains(c) && c != upcard)
        .collect();
    Round::from_deal(Rules::default(), Player::Two, hands, upcard, stock).unwrap()
}

/// A mid-layoff round: One knocked with ♠J deadwood, Two laid off the ♣4.
fn layoff_round() -> Round {
    let mut round = fresh_round();
    round.take_discard().unwrap();
    let melds = best_melds(round.hand(Player::One) - card("♠T").into());
    round.knock(card("♠T"), melds).unwrap();
    round.lay_off(card("♣4"), 0).unwrap();
    round
}

#[test]
fn round_snapshots_roundtrip() {
    // A fresh deal.
    let fresh = fresh_round();
    let json = serde_json::to_value(&fresh).unwrap();
    assert_eq!(json["initial_upcard"], json!("J♠"));
    assert_eq!(serde_json::from_value::<Round>(json).unwrap(), fresh);

    // Mid-layoff, with a spread and a laid-off card.
    let live = layoff_round();
    let json = serde_json::to_value(&live).unwrap();
    assert_eq!(json["phase"], json!("Layoff"));
    assert_eq!(json["knock"]["laid_off"], json!("4..."));
    let back: Round = serde_json::from_value(json).unwrap();
    assert_eq!(back, live);

    // Play the restored round to the end: it is fully functional.
    let mut back = back;
    back.lay_off(card("♣5"), 0).unwrap();
    let result = back.finish_layoffs().unwrap();
    assert_eq!(
        result,
        RoundResult::Knock {
            winner: Player::One,
            margin: 22,
        },
    );

    // A finished round round-trips too.
    let json = serde_json::to_value(&back).unwrap();
    assert_eq!(serde_json::from_value::<Round>(json).unwrap(), back);
}

#[test]
fn corrupt_snapshots_are_rejected() {
    let live = layoff_round();
    let json = serde_json::to_value(&live).unwrap();

    let patched = |patch: &dyn Fn(&mut Value)| {
        let mut copy = json.clone();
        patch(&mut copy);
        serde_json::from_value::<Round>(copy)
    };

    // The unpatched snapshot is fine.
    assert!(patched(&|_| ()).is_ok());

    // Duplicate cards across the hands (♣A) and the layoffs (♣4).
    assert!(patched(&|v| v["hands"][1] = json!("A45.89J.JQK.23")).is_err());
    // Lose the ♦8 entirely.
    assert!(patched(&|v| v["hands"][1] = json!("5.9J.JQK.23")).is_err());
    // A knock lingering in the draw phase (and short hands to boot).
    assert!(patched(&|v| v["phase"] = json!("Draw")).is_err());
    // A spread without a knock.
    assert!(patched(&|v| v["knock"] = Value::Null).is_err());
    // A result without the finished phase.
    assert!(patched(&|v| v["result"] = json!({"Knock": {"winner": "One", "margin": 22}})).is_err());
    // A knocker whose spread cards are not theirs: swap the acting player.
    assert!(patched(&|v| v["knock"]["knocker"] = json!("Two")).is_err());
    // Deadwood over the knock limit: shrink the spread to one meld.
    assert!(
        patched(&|v| {
            let melds = v["knock"]["spread"].as_array().unwrap();
            v["knock"]["spread"] = json!([melds[0]]);
        })
        .is_err()
    );
    // Corrupt pass counter.
    assert!(patched(&|v| v["passes"] = json!(9)).is_err());
    // A stray forced-stock flag.
    assert!(patched(&|v| v["forced_stock"] = json!(true)).is_err());

    // Oklahoma reads the knock limit from the initial upcard: the ♠J
    // upcard allows One's 10-point knock, a rewritten ♦4 upcard does not.
    assert!(patched(&|v| v["rules"]["oklahoma"] = json!("One")).is_ok());
    assert!(
        patched(&|v| {
            v["rules"]["oklahoma"] = json!("One");
            v["initial_upcard"] = json!("4♦");
        })
        .is_err()
    );
    // Without Oklahoma, a taken upcard is unverifiable — and harmless.
    assert!(patched(&|v| v["initial_upcard"] = json!("4♦")).is_ok());
    // A pre-Oklahoma `Rules` snapshot still loads.
    assert!(
        patched(&|v| {
            v["rules"].as_object_mut().unwrap().remove("oklahoma");
        })
        .is_ok()
    );

    // While the upcard is on offer, it must head the discard pile.
    let mut wrong = serde_json::to_value(fresh_round()).unwrap();
    wrong["initial_upcard"] = json!("2♥");
    assert!(serde_json::from_value::<Round>(wrong).is_err());

    // A finished round with a falsified margin is rejected.
    let mut done = layoff_round();
    done.lay_off(card("♣5"), 0).unwrap();
    done.finish_layoffs().unwrap();
    let json = serde_json::to_value(&done).unwrap();

    let mut wrong = json.clone();
    wrong["result"]["Knock"]["margin"] = json!(1);
    assert!(serde_json::from_value::<Round>(wrong).is_err());

    let mut flipped = json.clone();
    flipped["result"] = json!({"Undercut": {"winner": "Two", "margin": 22}});
    assert!(serde_json::from_value::<Round>(flipped).is_err());

    assert!(serde_json::from_value::<Round>(json).is_ok());
}
