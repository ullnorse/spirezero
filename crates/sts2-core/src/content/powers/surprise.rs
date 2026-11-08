//! `SurprisePower` (`Models.Powers/SurprisePower.cs`): a buff (`Single`) on `GremlinMerc`. When the
//! merc dies, a `FatGremlin` is created in the `fat` slot and given a `HeistPower` for each
//! `ThieveryPower` the merc had (the gold it stole), a `SneakyGremlin` joins in the `sneaky` slot,
//! then the fat gremlin joins; if any gold was stolen, `GremlinMercNormal` is marked. Keeps combat
//! from ending meanwhile.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CombatState, CreatureId, Engine, Flow};
use crate::content::encounters::{Encounter, EncounterData};
use crate::content::monsters::{FatGremlin, MonsterKind, SneakyGremlin};
use crate::content::powers::{PowerKind, PowerModel, PowerType, heist, thievery};

pub struct Surprise;

impl PowerModel for Surprise {
    fn name(&self) -> &'static str {
        "Surprise"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }
}

#[sts2_macros::hooks]
impl Hooks for Surprise {
    type Me = PowerRef;

    fn after_death(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        creature: CreatureId,
        was_removal_prevented: bool,
    ) -> Flow {
        if was_removal_prevented || me.owner != creature {
            return Ok(());
        }
        let fat = eng.create_enemy(MonsterKind::FatGremlin(FatGremlin), Some("fat"));
        let thefts: Vec<thievery::Data> = eng
            .s
            .creature(me.owner)
            .powers
            .iter()
            .filter(|p| p.kind == PowerKind::Thievery)
            .map(|p| {
                thievery::data(
                    eng.s,
                    PowerRef {
                        owner: me.owner,
                        id: p.id,
                    },
                )
            })
            .collect();
        let mut total_stolen = 0;
        for theft in thefts {
            total_stolen += theft.gold;
            let target = theft.target.expect("Thievery has a target");
            heist::apply(eng, fat, target, theft.gold, me.owner)?;
        }
        eng.add_enemy(MonsterKind::SneakyGremlin(SneakyGremlin), Some("sneaky"))?;
        eng.add_created_enemy(fat)?;
        if total_stolen > 0 && eng.s.encounter == Encounter::GremlinMercNormal {
            // `GremlinMercNormal.MarkGoldStolen`.
            eng.s.encounter_data = EncounterData::GremlinMercNormal {
                gold_was_stolen: true,
            };
        }
        Ok(())
    }

    fn should_stop_combat_from_ending(&self, _s: &CombatState, _me: PowerRef) -> bool {
        true
    }
}
