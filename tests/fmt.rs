//! Fixtures for `Display` and `FromStr` implementations

use gin_rummy::hand::{ParseCardError, ParseHandError, ParseHoldingError};
use gin_rummy::{Card, Hand, Holding, Rank, Suit};

#[test]
fn cards() {
    let ace_of_spades = Card {
        suit: Suit::Spades,
        rank: Rank::A,
    };
    assert_eq!(ace_of_spades.to_string(), "A♠");
    // Rank-first is the canonical form; suit-first still parses.
    assert_eq!("A♠".parse(), Ok(ace_of_spades));
    assert_eq!("♠A".parse(), Ok(ace_of_spades));
    assert_eq!("SA".parse(), Ok(ace_of_spades));
    assert_eq!("AS".parse(), Ok(ace_of_spades));
    assert_eq!("sa".parse(), Ok(ace_of_spades));
    assert_eq!("♤\u{FE0E}A".parse(), Ok(ace_of_spades));
    assert_eq!("A♤\u{FE0E}".parse(), Ok(ace_of_spades));

    let ten_of_hearts = Card {
        suit: Suit::Hearts,
        rank: Rank::T,
    };
    assert_eq!(ten_of_hearts.to_string(), "T♥");
    assert_eq!("T♥".parse(), Ok(ten_of_hearts));
    assert_eq!("♥T".parse(), Ok(ten_of_hearts));
    assert_eq!("♥10".parse(), Ok(ten_of_hearts));
    assert_eq!("10♥".parse(), Ok(ten_of_hearts));
    assert_eq!("H10".parse(), Ok(ten_of_hearts));
    assert_eq!("h10".parse(), Ok(ten_of_hearts));

    // A lone suit glyph is its own rank suffix, leaving an empty suit.
    assert_eq!("♥".parse::<Card>(), Err(ParseCardError::Suit));
    assert_eq!("A".parse::<Card>(), Err(ParseCardError::Suit));
    assert_eq!("♥1".parse::<Card>(), Err(ParseCardError::Rank));
    assert_eq!("X7".parse::<Card>(), Err(ParseCardError::Suit));
    assert_eq!("".parse::<Card>(), Err(ParseCardError::Suit));
}

#[test]
fn holdings() {
    let holding: Holding = "A23456789TJQK".parse().unwrap();
    assert_eq!(holding, Holding::ALL);
    assert_eq!(holding.to_string(), "A23456789TJQK");

    assert_eq!("".parse(), Ok(Holding::EMPTY));
    assert_eq!(Holding::EMPTY.to_string(), "");

    assert_eq!("a2j".parse::<Holding>().map(|h| h.len()), Ok(3));
    assert_eq!(
        "9105".parse::<Holding>(),
        Err(ParseHoldingError::InvalidRanks)
    );
    assert_eq!("910J".parse::<Holding>().map(|h| h.len()), Ok(3));

    assert_eq!(
        "3A".parse::<Holding>(),
        Err(ParseHoldingError::InvalidRanks)
    );
    assert_eq!(
        "AA".parse::<Holding>(),
        Err(ParseHoldingError::RepeatedRank)
    );
    assert_eq!(
        "T10".parse::<Holding>(),
        Err(ParseHoldingError::RepeatedRank)
    );
    assert_eq!(
        "A2x".parse::<Holding>(),
        Err(ParseHoldingError::InvalidRanks)
    );
    // 14 bytes still fits a full holding spelled with "10"; the repeated ace
    // is caught by rank order instead.
    assert_eq!(
        "A23456789TJQKA".parse::<Holding>(),
        Err(ParseHoldingError::InvalidRanks),
    );
    assert_eq!("A2345678910JQK".parse::<Holding>(), Ok(Holding::ALL),);
    assert_eq!(
        "A2345678910JQKA".parse::<Holding>(),
        Err(ParseHoldingError::TooManyCards),
    );
}

#[test]
fn hands() {
    let hand: Hand = "A23.456.789.T".parse().unwrap();
    assert_eq!(hand.to_string(), "A23.456.789.T");
    assert_eq!(hand.len(), 10);
    assert_eq!(hand[Suit::Clubs], "A23".parse().unwrap());
    assert_eq!(hand[Suit::Spades], "T".parse().unwrap());

    assert_eq!("-".parse(), Ok(Hand::EMPTY));
    assert_eq!(Hand::EMPTY.to_string(), "-");

    let uneven: Hand = "A23..9.QK".parse().unwrap();
    assert_eq!(uneven.to_string(), "A23..9.QK");
    assert!(uneven[Suit::Diamonds].is_empty());

    assert_eq!("...".parse::<Hand>().map(|h| h.len()), Ok(0));

    assert_eq!(
        "A23.456.789".parse::<Hand>(),
        Err(ParseHandError::NotFourSuits),
    );
    assert_eq!(
        "A23.456.789.T.J".parse::<Hand>(),
        Err(ParseHandError::NotFourSuits),
    );
    assert_eq!(
        "32A.456.789.T".parse::<Hand>(),
        Err(ParseHandError::Holding(ParseHoldingError::InvalidRanks)),
    );
}
