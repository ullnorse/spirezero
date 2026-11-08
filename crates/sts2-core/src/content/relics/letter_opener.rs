//! `LetterOpener` (`Models.Relics/LetterOpener.cs`): every 3rd Skill the owner plays in a turn
//! deals 5 damage to every hittable enemy.

use crate::combat::hooks::Hooks;
use crate::combat::{CardPlay, CombatState, CreatureId, Engine, Flow, RelicId, Side, ValueProps};
use crate::content::cards::CardType;
use crate::content::relics::{RelicData, RelicModel};
use crate::decimal::Decimal;

/// `CardsVar(3)`.
const CARDS: i32 = 3;
/// `DamageVar(5m, ValueProp.Unpowered)`.
const DAMAGE: i32 = 5;

pub struct LetterOpener;

/// `LetterOpener`'s fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `SkillsPlayedThisTurn`.
    pub skills_played_this_turn: i32,
}

fn data_mut(s: &mut CombatState, me: RelicId) -> &mut Data {
    let RelicData::LetterOpener(data) = &mut s.relic_mut(me).data else {
        unreachable!("a LetterOpener has LetterOpener data");
    };
    data
}

impl RelicModel for LetterOpener {
    fn name(&self) -> &'static str {
        "Letter Opener"
    }

    fn init_data(&self) -> RelicData {
        RelicData::LetterOpener(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for LetterOpener {
    type Me = RelicId;

    fn before_combat_start(&self, eng: &mut Engine<'_>, me: RelicId) -> Flow {
        data_mut(eng.s, me).skills_played_this_turn = 0;
        Ok(())
    }

    fn after_side_turn_start(
        &self,
        eng: &mut Engine<'_>,
        me: RelicId,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if participants.contains(&eng.s.player.creature) && eng.s.player.turn_number != 1 {
            data_mut(eng.s, me).skills_played_this_turn = 0;
        }
        Ok(())
    }

    fn after_card_played(&self, eng: &mut Engine<'_>, me: RelicId, play: &CardPlay) -> Flow {
        if !eng.s.in_progress || eng.s.card(play.card).card_type() != CardType::Skill {
            return Ok(());
        }
        let data = data_mut(eng.s, me);
        data.skills_played_this_turn += 1;
        if data.skills_played_this_turn % CARDS == 0 {
            let owner = eng.s.player.creature;
            let targets = eng.hooked().hittable_enemies();
            eng.damage(
                &targets,
                Decimal::from(DAMAGE),
                ValueProps::UNPOWERED,
                Some(owner),
                None,
            )?;
        }
        Ok(())
    }
}
