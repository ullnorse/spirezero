//! `Kusarigama` (`Models.Relics/Kusarigama.cs`): every 3rd Attack the owner plays in a turn
//! deals 6 damage to a random hittable enemy.

use crate::combat::hooks::Hooks;
use crate::combat::{CardPlay, CombatState, CreatureId, Engine, Flow, RelicId, Side, ValueProps};
use crate::content::cards::CardType;
use crate::content::relics::{RelicData, RelicModel};
use crate::decimal::Decimal;
use crate::rng::RunRngType;

/// `CardsVar(3)`.
const CARDS: i32 = 3;
/// `DamageVar(6m, ValueProp.Unpowered)`.
const DAMAGE: i32 = 6;

pub struct Kusarigama;

/// `Kusarigama`'s fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `AttacksPlayedThisTurn`.
    pub attacks_played_this_turn: i32,
}

fn data_mut(s: &mut CombatState, me: RelicId) -> &mut Data {
    let RelicData::Kusarigama(data) = &mut s.relic_mut(me).data else {
        unreachable!("a Kusarigama has Kusarigama data");
    };
    data
}

impl RelicModel for Kusarigama {
    fn name(&self) -> &'static str {
        "Kusarigama"
    }

    fn init_data(&self) -> RelicData {
        RelicData::Kusarigama(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for Kusarigama {
    type Me = RelicId;

    fn before_combat_start(&self, eng: &mut Engine<'_>, me: RelicId) -> Flow {
        data_mut(eng.s, me).attacks_played_this_turn = 0;
        Ok(())
    }

    fn after_side_turn_end(
        &self,
        eng: &mut Engine<'_>,
        me: RelicId,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if participants.contains(&eng.s.player.creature) {
            data_mut(eng.s, me).attacks_played_this_turn = 0;
        }
        Ok(())
    }

    fn after_card_played(&self, eng: &mut Engine<'_>, me: RelicId, play: &CardPlay) -> Flow {
        if !eng.s.in_progress || eng.s.card(play.card).card_type() != CardType::Attack {
            return Ok(());
        }
        let data = data_mut(eng.s, me);
        data.attacks_played_this_turn += 1;
        if data.attacks_played_this_turn % CARDS == 0 {
            let enemies = eng.hooked().hittable_enemies();
            if let Some(&target) = eng.s.rng[RunRngType::CombatTargets].next_item(&enemies) {
                let owner = eng.s.player.creature;
                eng.damage(
                    &[target],
                    Decimal::from(DAMAGE),
                    ValueProps::UNPOWERED,
                    Some(owner),
                    None,
                )?;
            }
        }
        Ok(())
    }
}
