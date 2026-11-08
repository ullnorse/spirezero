//! `RocketPunch` (`Models.Cards/RocketPunch.cs`): Defect uncommon attack, cost 2; 13 damage (14
//! upgraded), then draw 1 card (2 upgraded). Every Status the owner generates in combat makes
//! *this* card cost 1 less until played (`AfterCardGeneratedForCombat` calls `EnergyCost.AddUntilPlayed`
//! on the listener itself, not on the generated card).

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, CreatureId, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct RocketPunch;

#[sts2_macros::hooks]
impl Hooks for RocketPunch {
    type Me = CardId;

    fn after_card_generated_for_combat(
        &self,
        eng: &mut Engine<'_>,
        me: CardId,
        card: CardId,
        creator: Option<CreatureId>,
    ) -> Flow {
        // `creator != Owner` / `card.Owner != Owner`: the only player owns every card.
        if creator != Some(eng.s.player.creature) {
            return Ok(());
        }
        if eng.s.card(card).card_type() == CardType::Status {
            eng.s.energy_cost_mut(me).add_until_played(-1);
        }
        Ok(())
    }
}

impl CardModel for RocketPunch {
    fn name(&self) -> &'static str {
        "Rocket Punch"
    }

    fn cost(&self) -> i32 {
        2
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
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Damage, 13).upgrade(1),
            CardVar::new(Var::Cards, 1).upgrade(1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("RocketPunch needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        eng.draw(eng.s.card(play.card).var(Var::Cards), false)?;
        Ok(())
    }
}
