//! `FairyInABottle` (`Models.Potions/FairyInABottle.cs`): when the owner would die, the potion is
//! used automatically (`AfterPreventingDeath`) and heals 30% of max HP (at least 1).

use crate::combat::hooks::Hooks;
use crate::combat::{CombatState, CreatureId, Engine, Event, Flow, PotionId};
use crate::content::cards::TargetType;
use crate::content::potions::{self, PotionModel, PotionUsage};
use crate::decimal::Decimal;
use crate::run::{RunError, RunState};

/// `Math.Max(MaxHp * 0.3m, 1m)`.
const HEAL_PERCENT: Decimal = Decimal::new(3, 1);

pub struct FairyInABottle;

#[sts2_macros::hooks]
impl Hooks for FairyInABottle {
    type Me = PotionId;

    fn should_die(&self, s: &CombatState, _me: PotionId, creature: CreatureId) -> bool {
        // Only the owner's death is prevented.
        creature != s.player.creature
    }

    fn after_preventing_death(
        &self,
        eng: &mut Engine<'_>,
        me: PotionId,
        creature: CreatureId,
    ) -> Flow {
        // OnUseWrapper(new ThrowingPlayerChoiceContext(), creature), as in `use_potion_action`
        // but without the executor's `CheckWinCondition`.
        let slot = eng
            .s
            .player
            .potions
            .iter()
            .position(|p| p.is_some_and(|p| p.id == me))
            .expect("the preventer is in the belt");
        let potion = eng.s.player.potions[slot].expect("the slot holds the potion");
        eng.s.set_potion(slot, None);
        eng.emit(Event::PotionUsed {
            potion: potion.kind,
            target: Some(creature),
        });
        eng.before_potion_used(potion, Some(creature))?;
        eng.s.player.effect_depth += 1;
        let used = potions::on_use(eng, potion.kind, Some(creature));
        eng.s.player.effect_depth -= 1;
        used?;
        if eng.s.creature(eng.s.player.creature).is_alive() {
            eng.after_potion_used(potion, Some(creature))?;
            eng.check_for_empty_hand()?;
        }
        Ok(())
    }
}

impl PotionModel for FairyInABottle {
    fn name(&self) -> &'static str {
        "Fairy in a Bottle"
    }

    fn usage(&self) -> PotionUsage {
        PotionUsage::Automatic
    }

    fn target(&self) -> TargetType {
        // `TargetType.Self`.
        TargetType::Myself
    }

    fn on_use(&self, eng: &mut Engine<'_>, target: Option<CreatureId>) -> Flow {
        let target = target.expect("AssertValidForTargetedPotion");
        let max_hp = Decimal::from(eng.s.creature(target).max_hp);
        eng.heal(target, (max_hp * HEAL_PERCENT).max(Decimal::ONE))
    }

    fn on_use_out_of_combat(&self, s: &mut RunState) -> Result<(), RunError> {
        // Outside combat the run-level `Hook.AfterPreventingDeath` uses it (`run::hooks`).
        s.heal((Decimal::from(s.max_hp) * HEAL_PERCENT).max(Decimal::ONE));
        Ok(())
    }
}
