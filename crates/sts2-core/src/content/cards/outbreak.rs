//! `Outbreak` (`Models.Cards/Outbreak.cs`): cost 3, 9 Poison (12 upgraded) to every hittable enemy, then each Poison triggers once.

use crate::combat::hooks::Hooks;
use crate::combat::hooks::PowerRef;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::{PowerKind, poison};

pub struct Outbreak;

#[sts2_macros::hooks]
impl Hooks for Outbreak {
    type Me = CardId;
}

impl CardModel for Outbreak {
    fn name(&self) -> &'static str {
        "Outbreak"
    }

    fn cost(&self) -> i32 {
        3
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Rare
    }

    fn target(&self) -> TargetType {
        TargetType::AllEnemies
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Power(PowerKind::Poison), 9).upgrade(3)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let amount = eng.s.card(play.card).var(Var::Power(PowerKind::Poison));
        let player = eng.s.player.creature;
        for enemy in eng.hooked().hittable_enemies() {
            eng.apply_power_from_card(
                Some(play.card),
                enemy,
                PowerKind::Poison,
                amount,
                Some(player),
            )?;
        }
        for enemy in eng.hooked().hittable_enemies() {
            // `GetPower<PoisonPower>()` is null for an enemy that has no Poison.
            let poison = eng
                .s
                .creature(enemy)
                .powers
                .iter()
                .find(|p| p.kind == PowerKind::Poison)
                .map(|p| PowerRef {
                    owner: enemy,
                    id: p.id,
                });
            if let Some(poison) = poison {
                poison::trigger(eng, poison)?;
            }
        }
        Ok(())
    }
}
