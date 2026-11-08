//! `Wither` (`Models.Cards/Wither.cs`): status, unplayable. When the turn ends with it in hand it
//! deals 3 unpowered damage to its owner. `Aeonglass` fake-upgrades it (`FakeUpgrade`: +3 damage
//! each, without changing the upgrade level; the title and portrait follow, which are visual).

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, CombatState, Engine, Flow, ValueProps};
use crate::content::cards::{
    CardData, CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};
use crate::decimal::Decimal;

pub struct Wither;

/// `Wither`'s field.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `_fakeUpgradeLevel`.
    pub fake_upgrade_level: u32,
}

/// `FakeUpgradeLevel`.
#[must_use]
pub const fn fake_upgrade_level(card: &Card) -> u32 {
    match card.data {
        CardData::Wither(data) => data.fake_upgrade_level,
        _ => 0,
    }
}

/// The damage each `FakeUpgrade` adds (`DynamicVars.Damage.UpgradeValueBy(3m)`).
const FAKE_UPGRADE_DAMAGE: i32 = 3;

/// `FakeUpgrade`: one more fake upgrade level, +3 damage.
pub fn fake_upgrade(s: &mut CombatState, card: CardId) {
    let level = fake_upgrade_level(s.card(card)) + 1;
    s.card_mut(card).data = CardData::Wither(Data {
        fake_upgrade_level: level,
    });
}

#[sts2_macros::hooks]
impl Hooks for Wither {
    type Me = CardId;
}

impl CardModel for Wither {
    fn name(&self) -> &'static str {
        "Wither"
    }

    fn cost(&self) -> i32 {
        -1
    }

    fn card_type(&self) -> CardType {
        CardType::Status
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Status
    }

    fn target(&self) -> TargetType {
        TargetType::None
    }

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Unplayable)
    }

    fn max_upgrade_level(&self) -> u8 {
        0
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] =
            &[CardVar::new(Var::Damage, 3).props(ValueProps::UNPOWERED.with(ValueProps::MOVE))];
        VARS
    }

    fn instance_var(&self, card: &Card, name: Var, value: Decimal) -> Decimal {
        match name {
            Var::Damage => {
                let level = i32::try_from(fake_upgrade_level(card)).unwrap_or(i32::MAX);
                value + Decimal::from(FAKE_UPGRADE_DAMAGE * level)
            }
            _ => value,
        }
    }

    fn has_turn_end_in_hand_effect(&self) -> bool {
        true
    }

    /// `CreatureCmd.Damage(Owner.Creature, DynamicVars.Damage, this, null)`: the dealer is the
    /// card's owner (`cardSource?.Owner.Creature`).
    fn on_turn_end_in_hand(&self, eng: &mut Engine<'_>, me: CardId) -> Flow {
        let owner = eng.s.player.creature;
        eng.damage(
            &[owner],
            eng.s.card(me).var(Var::Damage),
            ValueProps::UNPOWERED.with(ValueProps::MOVE),
            Some(owner),
            Some(me),
        )?;
        Ok(())
    }

    fn on_play(&self, _eng: &mut Engine<'_>, _play: &CardPlay) -> Flow {
        Ok(())
    }
}
