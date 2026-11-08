//! `RainbowRing` (`Models.Relics/RainbowRing.cs`): the first turn the owner has played an
//! Attack, a Skill and a Power, it gains 1 Strength and 1 Dexterity.

use crate::combat::hooks::Hooks;
use crate::combat::{CardPlay, CombatState, CreatureId, Engine, Flow, RelicId, Side};
use crate::content::cards::CardType;
use crate::content::powers::PowerKind;
use crate::content::relics::{RelicData, RelicModel};
use crate::decimal::Decimal;

/// `PowerVar<StrengthPower>(1m)`.
const STRENGTH: i32 = 1;
/// `PowerVar<DexterityPower>(1m)`.
const DEXTERITY: i32 = 1;

pub struct RainbowRing;

/// `RainbowRing`'s fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `AttacksPlayedThisTurn`.
    pub attacks: i32,
    /// `SkillsPlayedThisTurn`.
    pub skills: i32,
    /// `PowersPlayedThisTurn`.
    pub powers: i32,
    /// `ActivationCountThisTurn`.
    pub activations: i32,
}

fn data_mut(s: &mut CombatState, me: RelicId) -> &mut Data {
    let RelicData::RainbowRing(data) = &mut s.relic_mut(me).data else {
        unreachable!("a RainbowRing has RainbowRing data");
    };
    data
}

impl RelicModel for RainbowRing {
    fn name(&self) -> &'static str {
        "Rainbow Ring"
    }

    fn init_data(&self) -> RelicData {
        RelicData::RainbowRing(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for RainbowRing {
    type Me = RelicId;

    fn before_side_turn_start(
        &self,
        eng: &mut Engine<'_>,
        me: RelicId,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if participants.contains(&eng.s.player.creature) {
            *data_mut(eng.s, me) = Data::default();
        }
        Ok(())
    }

    fn after_card_played(&self, eng: &mut Engine<'_>, me: RelicId, play: &CardPlay) -> Flow {
        if !eng.s.in_progress || data_mut(eng.s, me).activations >= 1 {
            return Ok(());
        }
        let card_type = eng.s.card(play.card).card_type();
        let data = data_mut(eng.s, me);
        match card_type {
            CardType::Attack => data.attacks += 1,
            CardType::Skill => data.skills += 1,
            CardType::Power => data.powers += 1,
            _ => {}
        }
        if data.attacks > 0 && data.skills > 0 && data.powers > 0 {
            let owner = eng.s.player.creature;
            eng.apply_power(
                owner,
                PowerKind::Strength,
                Decimal::from(STRENGTH),
                Some(owner),
            )?;
            eng.apply_power(
                owner,
                PowerKind::Dexterity,
                Decimal::from(DEXTERITY),
                Some(owner),
            )?;
            data_mut(eng.s, me).activations += 1;
        }
        Ok(())
    }

    fn after_combat_end(&self, eng: &mut Engine<'_>, me: RelicId) -> Flow {
        *data_mut(eng.s, me) = Data::default();
        Ok(())
    }
}
