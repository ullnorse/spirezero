//! `DampenPower` (`Models.Powers/DampenPower.cs`): a debuff (`StackType.None`) `MagiKnight` puts on
//! the player. When applied, every upgraded card in combat is downgraded and remembered; when the
//! last of its casters dies the power goes, and the remembered cards are upgraded back.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::state::Power;
use crate::combat::{CardId, CombatState, CreatureId, Engine, Flow, Unsupported};
use crate::content::powers::tangled::all_cards;
use crate::content::powers::{PowerData, PowerKind, PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct Dampen;

/// How many card ids [`Data::downgraded`] holds.
const CARD_CAPACITY: u32 = 256;

/// `DampenPower.Data`. `PowerData` is `Copy`, so both sets are bitsets: `casters` over
/// `CombatId` (`HashSet<Creature>`), `downgraded` over `CardId`
/// (`downgradedCardsToOldUpgradeLevels`: every card's `MaxUpgradeLevel` is 0 or 1, so the old
/// level of an upgraded card is always 1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    casters: u64,
    downgraded: [u64; 4],
}

impl Data {
    /// `casters.Count`.
    #[must_use]
    pub const fn casters_count(&self) -> u32 {
        self.casters.count_ones()
    }

    /// `downgradedCardsToOldUpgradeLevels.Count`.
    #[must_use]
    pub const fn downgraded_count(&self) -> u32 {
        self.downgraded[0].count_ones()
            + self.downgraded[1].count_ones()
            + self.downgraded[2].count_ones()
            + self.downgraded[3].count_ones()
    }

    const fn has_card(&self, card: CardId) -> bool {
        card.0 < CARD_CAPACITY && self.downgraded[card.index() / 64] & (1 << (card.0 % 64)) != 0
    }
}

fn data(s: &CombatState, me: PowerRef) -> Data {
    match *s.power_data(me.id) {
        PowerData::Dampen(data) => data,
        _ => unreachable!("a Dampen instance holds Dampen data"),
    }
}

fn set_data(eng: &mut Engine<'_>, me: PowerRef, data: Data) {
    eng.s.set_power_data(me.id, PowerData::Dampen(data));
}

/// `1 << CombatId` in [`Data::casters`].
fn caster_bit(caster: CreatureId) -> u64 {
    assert!(caster.0 < 64, "a Dampen caster's CombatId fits the bitset");
    1 << caster.0
}

/// `MagiKnight.DampenMove` for one target: the existing `Dampen` gets `caster` (`AddCaster`);
/// otherwise a new one with `caster` is applied (amount 1).
pub fn apply_from_caster(eng: &mut Engine<'_>, target: CreatureId, caster: CreatureId) -> Flow {
    let existing = eng
        .s
        .creature(target)
        .powers
        .iter()
        .find(|p| p.kind == PowerKind::Dampen)
        .map(|p| PowerRef {
            owner: target,
            id: p.id,
        });
    let me = match existing {
        Some(me) => Some(me),
        None => eng.apply_power_instance(target, PowerKind::Dampen, Decimal::ONE, Some(caster))?,
    };
    // The game adds the caster before applying; nothing in between reads it.
    if let Some(me) = me {
        let mut d = data(eng.s, me);
        d.casters |= caster_bit(caster);
        set_data(eng, me, d);
    }
    Ok(())
}

impl PowerModel for Dampen {
    fn name(&self) -> &'static str {
        "Dampen"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Debuff
    }

    fn init_internal_data(&self) -> PowerData {
        PowerData::Dampen(Data::default())
    }

    fn after_applied(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        _applier: Option<CreatureId>,
    ) -> Flow {
        let upgraded: Vec<CardId> = all_cards(eng.s)
            .into_iter()
            .filter(|&c| eng.s.card(c).upgrade_level > 0)
            .collect();
        for card in upgraded {
            if card.0 >= CARD_CAPACITY {
                // More combat cards than the bitset holds.
                return Unsupported::card(eng.s.card(card).kind.id()).stop();
            }
            let mut d = data(eng.s, me);
            d.downgraded[card.index() / 64] |= 1 << (card.0 % 64);
            set_data(eng, me, d);
            eng.downgrade(card);
        }
        Ok(())
    }

    fn after_removed(&self, eng: &mut Engine<'_>, power: &Power, _old_owner: CreatureId) -> Flow {
        let PowerData::Dampen(d) = *eng.s.power_data(power.id) else {
            unreachable!("a Dampen instance holds Dampen data")
        };
        // Card id order, not the dictionary's: `CardCmd.Upgrade` runs no hooks.
        for card in (0..CARD_CAPACITY).map(CardId).filter(|&c| d.has_card(c)) {
            eng.upgrade_card(card);
        }
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for Dampen {
    type Me = PowerRef;

    fn after_death(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        creature: CreatureId,
        was_removal_prevented: bool,
    ) -> Flow {
        if was_removal_prevented || creature.0 >= 64 {
            return Ok(());
        }
        let mut d = data(eng.s, me);
        if d.casters & (1 << creature.0) != 0 {
            d.casters &= !(1 << creature.0);
            set_data(eng, me, d);
            if d.casters == 0 {
                eng.remove_power(me.owner, me.id)?;
            }
        }
        Ok(())
    }
}
