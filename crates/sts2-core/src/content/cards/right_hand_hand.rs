//! `RightHandHand` (`Models.Cards/RightHandHand.cs`): cost 0, Osty hits for 4 (6 upgraded).
//! Whenever a card costing at least 2 energy is played while this card is in the discard pile,
//! it returns to the hand.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets, Position};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, PileType};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct RightHandHand;

#[sts2_macros::hooks]
impl Hooks for RightHandHand {
    type Me = CardId;

    fn after_card_played_late(&self, eng: &mut Engine<'_>, me: CardId, play: &CardPlay) -> Flow {
        // `cardPlay.Card.Owner == Owner`: every card belongs to the player.
        if play.energy_value >= eng.s.card(me).int_var(Var::Energy)
            && eng.s.pile_of(me) == Some(PileType::Discard)
        {
            eng.add_to_pile(me, PileType::Hand, Position::Bottom)?;
        }
        Ok(())
    }
}

impl CardModel for RightHandHand {
    fn name(&self) -> &'static str {
        "Right Hand Hand"
    }

    fn cost(&self) -> i32 {
        0
    }

    fn card_type(&self) -> CardType {
        CardType::Attack
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Uncommon
    }

    fn target(&self) -> TargetType {
        TargetType::AnyEnemy
    }

    fn is_osty_attack(&self) -> bool {
        true
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::OstyDamage, 4).upgrade(2),
            CardVar::new(Var::Energy, 2),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("RightHandHand needs a target");
        // Osty.CheckMissingWithAnim.
        if eng.s.is_osty_alive() {
            let damage = eng.s.card(play.card).var(Var::OstyDamage);
            let osty = eng.s.osty().expect("Osty is alive");
            eng.attack(&Attack::new(
                osty,
                AttackTargets::Single(target),
                AttackDamage::Fixed(damage),
                Some(play.card),
            ))?;
        }
        Ok(())
    }
}
