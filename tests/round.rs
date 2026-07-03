//! Scripted rounds exercising the full state machine

use gin_rummy::round::{DealError, RoundError};
use gin_rummy::{
    Card, Hand, Melds, Phase, Player, Rank, Round, RoundResult, Rules, Suit, best_melds, deadwood,
};

fn card(s: &str) -> Card {
    s.parse().unwrap()
}

fn hand(s: &str) -> Hand {
    s.parse().unwrap()
}

/// Deal with `Player::Two` as dealer, so `Player::One` gets the upcard
/// offer.  The stock is the 31 leftovers, reordered so that `top` lists the
/// first cards drawn, in order.
fn deal_with(rules: Rules, one: &str, two: &str, upcard: &str, top: &[&str]) -> Round {
    let hands = [hand(one), hand(two)];
    let upcard = card(upcard);
    let top: Vec<Card> = top.iter().map(|s| card(s)).collect();

    let mut stock: Vec<Card> = Hand::ALL
        .iter()
        .filter(|&c| !hands[0].contains(c) && !hands[1].contains(c) && c != upcard)
        .filter(|c| !top.contains(c))
        .collect();
    stock.extend(top.iter().rev());

    Round::from_deal(rules, Player::Two, hands, upcard, stock).unwrap()
}

fn deal(one: &str, two: &str, upcard: &str, top: &[&str]) -> Round {
    deal_with(Rules::default(), one, two, upcard, top)
}

#[test]
fn deal_validation() {
    let one = hand("A23.456.789.T");
    let two = hand("JQK.JQK.JQK.9");
    let upcard = card("♠2");
    let stock: Vec<Card> = Hand::ALL
        .iter()
        .filter(|&c| !one.contains(c) && !two.contains(c) && c != upcard)
        .collect();
    let rules = Rules::default();

    assert!(Round::from_deal(rules, Player::One, [one, two], upcard, stock.clone()).is_ok());

    assert_eq!(
        Round::from_deal(rules, Player::One, [one, one], upcard, stock.clone()).unwrap_err(),
        DealError::RepeatedCard,
    );
    assert_eq!(
        Round::from_deal(rules, Player::One, [one, two], card("♣A"), stock.clone()).unwrap_err(),
        DealError::RepeatedCard,
    );
    assert_eq!(
        Round::from_deal(
            rules,
            Player::One,
            [one, hand("JQK.JQK.JQK.")],
            upcard,
            stock.clone()
        )
        .unwrap_err(),
        DealError::WrongHandSize,
    );
    assert_eq!(
        Round::from_deal(rules, Player::One, [one, two], upcard, stock[1..].to_vec()).unwrap_err(),
        DealError::WrongStockSize,
    );

    let mut repeated = stock;
    repeated[0] = repeated[1];
    assert_eq!(
        Round::from_deal(rules, Player::One, [one, two], upcard, repeated).unwrap_err(),
        DealError::RepeatedCard,
    );
}

#[test]
fn upcard_take_and_knock_with_layoffs() {
    // One runs A23♣ 456♦ 789♥ with ♠T deadwood; the ♠J upcard tests the
    // just-taken rule.
    let mut round = deal("A23.456.789.T", "45.89J.JQK.23", "♠J", &[]);

    assert_eq!(round.phase(), Phase::Upcard);
    assert_eq!(round.dealer(), Player::Two);
    assert_eq!(round.non_dealer(), Player::One);
    assert_eq!(round.turn(), Some(Player::One));
    assert_eq!(round.knock_limit(), 10);
    assert_eq!(round.discard_pile(), [card("♠J")]);
    assert_eq!(round.stock().len(), 31);
    assert_eq!(round.knocker(), None);
    assert_eq!(round.spread().count(), 0);
    assert_eq!(round.laid_off(), Hand::EMPTY);

    // Wrong-phase actions are rejected up front.
    assert_eq!(
        round.draw_stock().unwrap_err(),
        RoundError::WrongPhase(Phase::Upcard),
    );
    assert_eq!(
        round.discard(card("♠T")).unwrap_err(),
        RoundError::WrongPhase(Phase::Upcard),
    );
    assert_eq!(
        round.finish_layoffs().unwrap_err(),
        RoundError::WrongPhase(Phase::Upcard),
    );

    assert_eq!(round.take_discard().unwrap(), card("♠J"));
    assert_eq!(round.phase(), Phase::Discard);
    assert_eq!(round.hand(Player::One).len(), 11);

    // The card taken from the pile may not come right back.
    assert_eq!(
        round.discard(card("♠J")).unwrap_err(),
        RoundError::DiscardJustTaken(card("♠J")),
    );
    let melds = best_melds(round.hand(Player::One) - card("♠J").into());
    assert_eq!(
        round.knock(card("♠J"), melds).unwrap_err(),
        RoundError::DiscardJustTaken(card("♠J")),
    );

    // Knock at exactly the limit: spread the three runs, keep ♠J (10).
    let melds = best_melds(round.hand(Player::One) - card("♠T").into());
    assert_eq!(melds.deadwood(), 10);
    round.knock(card("♠T"), melds).unwrap();

    assert_eq!(round.phase(), Phase::Layoff);
    assert_eq!(round.knocker(), Some(Player::One));
    assert_eq!(round.turn(), Some(Player::Two));
    assert_eq!(round.spread().count(), 3);

    let club_run = round.spread().next().unwrap();
    assert_eq!(club_run.suit(), Some(Suit::Clubs));

    // ♦8 extends nothing at index 1; ♦7 is not even in the defender's hand;
    // ♣4 does not fit the heart run at index 2.
    assert_eq!(
        round.lay_off(card("♦8"), 1).unwrap_err(),
        RoundError::CannotLayOff(card("♦8")),
    );
    assert_eq!(
        round.lay_off(card("♦7"), 1).unwrap_err(),
        RoundError::NotInHand(card("♦7")),
    );
    assert_eq!(
        round.lay_off(card("♣4"), 2).unwrap_err(),
        RoundError::CannotLayOff(card("♣4")),
    );

    // Chained layoff: ♣4 then ♣5 onto A-2-3 of clubs — but not ♣5 first.
    assert_eq!(
        round.lay_off(card("♣5"), 0).unwrap_err(),
        RoundError::CannotLayOff(card("♣5")),
    );
    round.lay_off(card("♣4"), 0).unwrap();
    round.lay_off(card("♣5"), 0).unwrap();
    assert_eq!(round.laid_off(), hand("45..."));
    assert_eq!(round.spread().next().unwrap().high(), Some(Rank::new(5)));

    // Defender keeps ♥JQK melded; deadwood is ♦8 ♦9 ♦J ♠2 ♠3 = 32.
    let result = round.finish_layoffs().unwrap();
    assert_eq!(
        result,
        RoundResult::Knock {
            winner: Player::One,
            margin: 22,
        },
    );
    assert_eq!(round.phase(), Phase::Finished);
    assert_eq!(round.turn(), None);
    assert_eq!(round.result(), Some(result));
    assert_eq!(result.winner(), Some(Player::One));
    assert_eq!(result.points(round.rules()), 22);

    // The round is over: every action now reports the finished phase.
    assert_eq!(
        round.pass().unwrap_err(),
        RoundError::WrongPhase(Phase::Finished),
    );
    assert_eq!(
        round.draw_stock().unwrap_err(),
        RoundError::WrongPhase(Phase::Finished),
    );
}

#[test]
fn pass_pass_forces_stock_draw_and_tie_undercuts() {
    let mut round = deal("A23.456.789.J", "JQK.JQK.JQK.T", "♠2", &["♠K", "♠Q", "♠4"]);

    round.pass().unwrap();
    assert_eq!(round.phase(), Phase::Upcard);
    assert_eq!(round.turn(), Some(Player::Two));
    round.pass().unwrap();
    assert_eq!(round.phase(), Phase::Draw);
    assert_eq!(round.turn(), Some(Player::One));

    // Both passed: the first draw must hit the stock.
    assert_eq!(
        round.take_discard().unwrap_err(),
        RoundError::MustDrawFromStock,
    );
    assert_eq!(round.draw_stock().unwrap(), card("♠K"));
    round.discard(card("♠K")).unwrap();

    // The forced draw applied only to that first draw: Two may now take
    // from the pile, though here Two draws the ♠Q from the stock instead.
    let drawn = round.draw_stock().unwrap();
    assert_eq!(drawn, card("♠Q"));
    round.discard(drawn).unwrap();

    let drawn = round.draw_stock().unwrap();
    assert_eq!(drawn, card("♠4"));

    // One knocks with 10 deadwood (♠J) while Two melds JJJ QQQ KKK around
    // the ♠T, also 10: a tie, undercut by default.
    let melds = best_melds(round.hand(Player::One) - drawn.into());
    round.knock(drawn, melds).unwrap();
    assert_eq!(deadwood(round.hand(Player::Two)), 10);
    let result = round.finish_layoffs().unwrap();
    assert_eq!(
        result,
        RoundResult::Undercut {
            winner: Player::Two,
            margin: 0,
        },
    );
    assert_eq!(result.points(round.rules()), 25);
}

#[test]
fn taking_from_the_pile_after_a_forced_draw() {
    let mut round = deal("A23.456.789.T", "45.89J.JQK.23", "♠J", &["♠K"]);
    round.pass().unwrap();
    round.pass().unwrap();

    // One is forced into the stock and throws the draw back.
    assert_eq!(round.draw_stock().unwrap(), card("♠K"));
    round.discard(card("♠K")).unwrap();

    // Two takes the fresh ♠K from the pile: the forced draw is spent.
    assert_eq!(round.turn(), Some(Player::Two));
    assert_eq!(round.take_discard().unwrap(), card("♠K"));
    assert!(round.hand(Player::Two).contains(card("♠K")));
    assert_eq!(
        round.discard(card("♠K")).unwrap_err(),
        RoundError::DiscardJustTaken(card("♠K")),
    );
    round.discard(card("♠2")).unwrap();
    assert_eq!(round.turn(), Some(Player::One));
}

#[test]
fn tie_without_undercut_goes_to_the_knocker() {
    let mut rules = Rules::default();
    rules.undercut_on_tie = false;

    let mut round = deal_with(
        rules,
        "A23.456.789.J",
        "JQK.JQK.JQK.T",
        "♠2",
        &["♠K", "♠Q", "♠4"],
    );

    round.pass().unwrap();
    round.pass().unwrap();
    let drawn = round.draw_stock().unwrap();
    round.discard(drawn).unwrap();
    let drawn = round.draw_stock().unwrap();
    round.discard(drawn).unwrap();

    let drawn = round.draw_stock().unwrap();
    assert_eq!(drawn, card("♠4"));
    let melds = best_melds(round.hand(Player::One) - drawn.into());
    round.knock(drawn, melds).unwrap();

    // Both sides keep 10 deadwood, but ties no longer undercut.
    assert_eq!(
        round.finish_layoffs().unwrap(),
        RoundResult::Knock {
            winner: Player::One,
            margin: 0,
        },
    );
}

#[test]
fn gin_skips_layoffs() {
    // Taking the upcard and knocking it right back is illegal.
    let mut round = deal("A234.567.9TJ.", "78.89J.QK.235", "♠A", &["♠K"]);
    round.take_discard().unwrap();
    let melds = best_melds(round.hand(Player::One) - card("♠A").into());
    assert_eq!(
        round.knock(card("♠A"), melds).unwrap_err(),
        RoundError::DiscardJustTaken(card("♠A")),
    );

    // Fresh round: draw ♠K from the stock and knock it away for gin.
    let mut round = deal("A234.567.9TJ.", "78.89J.QK.235", "♠A", &["♠K"]);
    round.pass().unwrap();
    round.pass().unwrap();
    let drawn = round.draw_stock().unwrap();
    assert_eq!(drawn, card("♠K"));

    let melds = best_melds(round.hand(Player::One) - drawn.into());
    assert_eq!(melds.deadwood(), 0);
    round.knock(drawn, melds).unwrap();

    // Gin finishes immediately: no layoff phase.  The defender has no meld
    // at all: ♣7♣8 + ♦8♦9♦J + ♥Q♥K + ♠2♠3♠5 = 72.
    assert_eq!(round.phase(), Phase::Finished);
    assert_eq!(round.knocker(), Some(Player::One));
    assert_eq!(
        round.result(),
        Some(RoundResult::Gin {
            winner: Player::One,
            deadwood: 72,
        }),
    );
    assert_eq!(round.result().unwrap().points(round.rules()), 97);
    assert_eq!(
        round.lay_off(card("♣4"), 0).unwrap_err(),
        RoundError::WrongPhase(Phase::Finished),
    );
}

#[test]
fn big_gin_and_its_fallback() {
    // Drawing ♣5 completes an 11-card melded hand: A2345♣ 567♦ 9TJ♥.
    let mut round = deal("A234.567.9TJ.", "78.89J.QK.235", "♠A", &["♣5"]);
    round.pass().unwrap();
    round.pass().unwrap();
    round.draw_stock().unwrap();

    let eleven = round.hand(Player::One);
    assert_eq!(deadwood(eleven), 0);

    round.declare_big_gin(best_melds(eleven)).unwrap();
    assert_eq!(
        round.result(),
        Some(RoundResult::BigGin {
            winner: Player::One,
            deadwood: 72,
        }),
    );
    assert_eq!(round.result().unwrap().points(round.rules()), 72 + 31);

    // Classic rules disable big gin, but plain gin remains reachable by
    // shedding a card from the 4-card club run.
    let mut round = deal_with(
        Rules::classic(),
        "A234.567.9TJ.",
        "78.89J.QK.235",
        "♠A",
        &["♣5"],
    );
    round.pass().unwrap();
    round.pass().unwrap();
    round.draw_stock().unwrap();

    let melds = best_melds(round.hand(Player::One));
    assert_eq!(
        round.declare_big_gin(melds).unwrap_err(),
        RoundError::BigGinDisabled,
    );

    let gin = best_melds(round.hand(Player::One) - card("♣A").into());
    assert_eq!(gin.deadwood(), 0);
    round.knock(card("♣A"), gin).unwrap();
    assert_eq!(
        round.result(),
        Some(RoundResult::Gin {
            winner: Player::One,
            deadwood: 72,
        }),
    );

    // Under classic rules a gin is worth 20 + deadwood.
    assert_eq!(round.result().unwrap().points(round.rules()), 92);
}

#[test]
fn knock_validation() {
    let mut round = deal("A23.456.789.J", "JQK.JQK.JQK.T", "♠2", &["♠K"]);
    round.pass().unwrap();
    round.pass().unwrap();
    round.draw_stock().unwrap();

    let hand_one = round.hand(Player::One);

    // An arrangement of the wrong cards is rejected.
    let wrong = best_melds(hand_one - card("♠K").into());
    assert_eq!(
        round.knock(card("♠J"), wrong).unwrap_err(),
        RoundError::MeldsMismatch,
    );

    // A suboptimal arrangement can exceed the limit even when the optimal
    // one would not: spreading no melds leaves all 55 points as deadwood.
    let bare = Melds::try_new(hand_one - card("♠K").into(), &[]).unwrap();
    assert_eq!(
        round.knock(card("♠K"), bare).unwrap_err(),
        RoundError::TooMuchDeadwood {
            deadwood: 55,
            limit: 10,
        },
    );

    // Big gin needs a zero-deadwood arrangement of all 11 cards.
    assert_eq!(
        round.declare_big_gin(best_melds(hand_one)).unwrap_err(),
        RoundError::NotBigGin,
    );

    // Cards outside the hand cannot be knocked away.
    let melds = best_melds(hand_one - card("♠K").into());
    assert_eq!(
        round.knock(card("♠5"), melds).unwrap_err(),
        RoundError::NotInHand(card("♠5")),
    );
}

#[test]
fn layoff_indices() {
    let mut round = deal("A23.456.789.T", "45.89J.JQK.23", "♠J", &[]);
    round.take_discard().unwrap();
    let melds = best_melds(round.hand(Player::One) - card("♠T").into());
    round.knock(card("♠T"), melds).unwrap();

    assert_eq!(
        round.lay_off(card("♣4"), 3).unwrap_err(),
        RoundError::NoSuchMeld(3),
    );
    assert_eq!(
        round.lay_off(card("♣4"), usize::MAX).unwrap_err(),
        RoundError::NoSuchMeld(usize::MAX),
    );
    round.lay_off(card("♣4"), 0).unwrap();
}

#[test]
fn dead_hand_at_two_stock_cards() {
    let mut round = deal("A23.456.789.J", "JQK.JQK.JQK.T", "♠2", &[]);
    round.pass().unwrap();
    round.pass().unwrap();

    // Both players draw from the stock and shed the drawn card until the
    // stock runs down to two cards.
    let result = loop {
        let drawn = round.draw_stock().unwrap();
        round.discard(drawn).unwrap();
        if let Some(result) = round.result() {
            break result;
        }
    };

    assert_eq!(result, RoundResult::Dead);
    assert_eq!(round.stock().len(), 2);
    assert_eq!(round.phase(), Phase::Finished);
    assert_eq!(result.winner(), None);
    assert_eq!(result.points(round.rules()), 0);

    // Hands are intact after the draw-and-shed turns.
    assert_eq!(round.hand(Player::One), hand("A23.456.789.J"));
    assert_eq!(round.hand(Player::Two), hand("JQK.JQK.JQK.T"));
}
