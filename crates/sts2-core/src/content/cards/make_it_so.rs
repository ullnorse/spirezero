//! `MakeItSo` (`Models.Cards/MakeItSo.cs`): Regent rare attack, cost 0; deal 6 damage (9
//! upgraded). Outside the hand, it returns to the hand whenever the number of Skills the player
//! finished this turn is a multiple of `Cards` (3).

use crate::combat::commands::{Attack, AttackDamage, AttackTargets, Position};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, HistoryKind, HistoryTag, PileType};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct MakeItSo;

#[sts2_macros::hooks]
impl Hooks for MakeItSo {
    type Me = CardId;

    fn after_card_played_late(&self, eng: &mut Engine<'_>, me: CardId, play: &CardPlay) -> Flow {
        // `cardPlay.Card.Owner == Owner`: every card belongs to the player.
        if eng.s.card(play.card).card_type() != CardType::Skill
            || eng.s.pile_of(me) == Some(PileType::Hand)
        {
            return Ok(());
        }
        // `CardPlaysFinished` of the player's Skills this turn.
        let skills = eng
            .s
            .history_this_turn(HistoryTag::CardPlayFinished)
            .iter()
            .filter(|e| {
                matches!(&e.kind, HistoryKind::CardPlayFinished { play, .. }
                    if eng.s.card(play.card).card_type() == CardType::Skill)
            })
            .count();
        let cards = eng.s.card(me).int_var(Var::Cards);
        if i32::try_from(skills).expect("a count fits i32") % cards == 0 {
            eng.add_to_pile(me, PileType::Hand, Position::Bottom)?;
        }
        Ok(())
    }
}

impl CardModel for MakeItSo {
    fn name(&self) -> &'static str {
        "Make It So"
    }

    fn cost(&self) -> i32 {
        0
    }

    fn card_type(&self) -> CardType {
        CardType::Attack
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Rare
    }

    fn target(&self) -> TargetType {
        TargetType::AnyEnemy
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Damage, 6).upgrade(3),
            CardVar::new(Var::Cards, 3),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("MakeItSo needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        Ok(())
    }
}
