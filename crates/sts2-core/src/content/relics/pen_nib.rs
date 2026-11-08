//! `PenNib` (`Models.Relics/PenNib.cs`): every 10th Attack the owner plays deals double damage.
//! The count (`[SavedProperty] AttacksPlayed`) carries over between combats.

use crate::combat::hooks::{DamageQuery, Hooks};
use crate::combat::{CardId, CardPlay, CombatState, Engine, Flow, PileType, RelicId};
use crate::content::cards::CardType;
use crate::content::relics::{RelicData, RelicModel};
use crate::decimal::Decimal;

/// `_attacksThreshold`.
const ATTACKS_THRESHOLD: i32 = 10;

pub struct PenNib;

/// `PenNib`'s fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `AttacksPlayed`.
    pub attacks_played: i32,
    /// `AttackToDouble`.
    pub attack_to_double: Option<CardId>,
}

fn data_mut(s: &mut CombatState, me: RelicId) -> &mut Data {
    let RelicData::PenNib(data) = &mut s.relic_mut(me).data else {
        unreachable!("a PenNib has PenNib data");
    };
    data
}

impl RelicModel for PenNib {
    fn name(&self) -> &'static str {
        "Pen Nib"
    }

    fn init_data(&self) -> RelicData {
        RelicData::PenNib(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for PenNib {
    type Me = RelicId;

    fn modify_damage_multiplicative(
        &self,
        s: &CombatState,
        me: RelicId,
        query: DamageQuery,
        _amount: Decimal,
    ) -> Decimal {
        let Some(card) = query.card else {
            return Decimal::ONE;
        };
        if !query.props.is_powered_attack()
            || (query.dealer != Some(s.player.creature) && query.dealer != s.osty())
        {
            return Decimal::ONE;
        }
        let RelicData::PenNib(data) = s.relic(me).data else {
            unreachable!("a PenNib has PenNib data");
        };
        let double = data.attack_to_double.map_or_else(
            || {
                s.pile_of(card) != Some(PileType::Play)
                    && data.attacks_played == ATTACKS_THRESHOLD - 1
            },
            |attack| attack == card,
        );
        if double {
            Decimal::from(2)
        } else {
            Decimal::ONE
        }
    }

    fn before_card_played(&self, eng: &mut Engine<'_>, me: RelicId, play: &CardPlay) -> Flow {
        if eng.s.card(play.card).card_type() != CardType::Attack {
            return Ok(());
        }
        let data = data_mut(eng.s, me);
        data.attacks_played = (data.attacks_played + 1) % ATTACKS_THRESHOLD;
        if data.attacks_played == 0 {
            data.attack_to_double = Some(play.card);
        }
        Ok(())
    }

    fn after_card_played(&self, eng: &mut Engine<'_>, me: RelicId, play: &CardPlay) -> Flow {
        let data = data_mut(eng.s, me);
        if data.attack_to_double == Some(play.card) {
            data.attack_to_double = None;
        }
        Ok(())
    }

    fn after_combat_end(&self, eng: &mut Engine<'_>, me: RelicId) -> Flow {
        // A card of this fight means nothing in the next one.
        data_mut(eng.s, me).attack_to_double = None;
        Ok(())
    }
}
