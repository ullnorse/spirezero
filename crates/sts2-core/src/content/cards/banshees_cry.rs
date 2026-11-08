//! `BansheesCry` (`Models.Cards/BansheesCry.cs`): cost 9 (7 upgraded), 33 damage to all
//! enemies. Costs 2 less for the rest of the combat for each Ethereal card the player played,
//! including the ones played before it entered combat.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, TargetType, Var,
};

pub struct BansheesCry;

#[sts2_macros::hooks]
impl Hooks for BansheesCry {
    type Me = CardId;

    fn after_card_entered_combat(&self, eng: &mut Engine<'_>, me: CardId, card: CardId) -> Flow {
        if card != me || eng.s.card(me).is_clone() {
            return Ok(());
        }
        let ethereal_plays = eng.s.ethereal_card_plays_finished();
        let ethereal_plays = i32::try_from(ethereal_plays).expect("history size fits i32");
        let energy = eng.s.card(me).int_var(Var::Energy);
        eng.s
            .energy_cost_mut(me)
            .add_this_combat(-ethereal_plays * energy);
        Ok(())
    }

    fn after_card_played(&self, eng: &mut Engine<'_>, me: CardId, play: &CardPlay) -> Flow {
        if !eng.hooked().has_keyword(play.card, CardKeyword::Ethereal) {
            return Ok(());
        }
        let energy = eng.s.card(me).int_var(Var::Energy);
        eng.s.energy_cost_mut(me).add_this_combat(-energy);
        Ok(())
    }
}

impl CardModel for BansheesCry {
    fn name(&self) -> &'static str {
        "Banshee's Cry"
    }

    fn cost(&self) -> i32 {
        9
    }

    fn card_type(&self) -> CardType {
        CardType::Attack
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Rare
    }

    fn target(&self) -> TargetType {
        TargetType::AllEnemies
    }

    fn on_upgrade(&self, card: &mut Card) {
        card.energy_cost.upgrade_by(-2);
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Damage, 33), CardVar::new(Var::Energy, 2)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::AllOpponents,
            AttackDamage::Fixed(eng.s.card(play.card).var(Var::Damage)),
            Some(play.card),
        ))?;
        Ok(())
    }
}
