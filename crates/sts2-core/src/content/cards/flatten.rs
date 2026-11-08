//! `Flatten` (`Models.Cards/Flatten.cs`): cost 2, Osty hits for 12 (16 upgraded). Costs 0 for
//! the rest of the turn once Osty has attacked this turn. Does nothing without a living Osty.

use crate::combat::commands::damage::DamageResult;
use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, CombatState, Engine, Flow, HistoryKind, HistoryTag};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct Flatten;

#[sts2_macros::hooks]
impl Hooks for Flatten {
    type Me = CardId;

    fn after_card_entered_combat(&self, eng: &mut Engine<'_>, me: CardId, card: CardId) -> Flow {
        if card == me && has_osty_attacked_this_turn(eng.s) {
            reduce_cost(eng, me);
        }
        Ok(())
    }

    fn after_attack(
        &self,
        eng: &mut Engine<'_>,
        me: CardId,
        attack: &Attack,
        _results: &[DamageResult],
    ) -> Flow {
        if Some(attack.attacker) == eng.s.osty() {
            reduce_cost(eng, me);
        }
        Ok(())
    }
}

impl CardModel for Flatten {
    fn name(&self) -> &'static str {
        "Flatten"
    }

    fn cost(&self) -> i32 {
        2
    }

    fn card_type(&self) -> CardType {
        CardType::Attack
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Common
    }

    fn target(&self) -> TargetType {
        TargetType::AnyEnemy
    }

    fn is_osty_attack(&self) -> bool {
        true
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::OstyDamage, 12).upgrade(4)];
        VARS
    }

    /// `ShouldGlowGoldInternal`: `HasOstyAttackedThisTurn`.
    fn glows_gold(&self, s: &CombatState, _me: CardId) -> bool {
        has_osty_attacked_this_turn(s)
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Flatten needs a target");
        // Osty.CheckMissingWithAnim.
        if !eng.s.is_osty_alive() {
            return Ok(());
        }
        let damage = eng.s.card(play.card).var(Var::OstyDamage);
        let osty = eng.s.osty().expect("Osty is alive");
        eng.attack(&Attack::new(
            osty,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        Ok(())
    }
}

/// `HasOstyAttackedThisTurn`: a `CreatureAttackedEntry` by Osty this turn.
fn has_osty_attacked_this_turn(s: &CombatState) -> bool {
    let Some(osty) = s.osty() else {
        return false;
    };
    s.history_this_turn(HistoryTag::CreatureAttacked)
        .iter()
        .any(|e| e.actor == osty && matches!(e.kind, HistoryKind::CreatureAttacked))
}

/// `ReduceCost`: `EnergyCost.SetThisTurn(0)`.
fn reduce_cost(eng: &mut Engine<'_>, me: CardId) {
    eng.s.energy_cost_mut(me).set_this_turn(0);
}
