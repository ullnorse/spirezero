//! `KinglyKick` (`Models.Cards/KinglyKick.cs`): Regent uncommon attack, cost 4; deal 27 damage (35
//! upgraded). Each time the card is drawn its energy cost drops by 1 for the rest of the combat.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct KinglyKick;

#[sts2_macros::hooks]
impl Hooks for KinglyKick {
    type Me = CardId;

    fn after_card_drawn(
        &self,
        eng: &mut Engine<'_>,
        me: CardId,
        card: CardId,
        _from_hand_draw: bool,
    ) -> Flow {
        if card != me {
            return Ok(());
        }
        eng.s.energy_cost_mut(me).add_this_combat(-1);
        Ok(())
    }
}

impl CardModel for KinglyKick {
    fn name(&self) -> &'static str {
        "Kingly Kick"
    }

    fn cost(&self) -> i32 {
        4
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

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Damage, 27).upgrade(8)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("KinglyKick needs a target");
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
