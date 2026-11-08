//! `FoulPotion` (`Models.Potions/FoulPotion.cs`): in combat, 12 unpowered damage to every creature
//! that is not a pet, the player included; outside combat it is thrown at a merchant for 100
//! gold. Event rarity (`PotionCourier`), usable any time.
//!
//! Outside combat `PassesCustomUsabilityCheck` needs a `MerchantRoom` or the `FakeMerchant`
//! event, with the inventory screen closed; the engine allows it anywhere in a shop room. In
//! `FakeMerchant` the throw (`FoulPotionThrown`) is that event's own option per belt slot
//! (`events/fake_merchant.rs`), so `RunAction::UsePotion` is not offered there. The headless
//! referee has no `NMerchantRoom`, so it never offers the shop throw.

use crate::combat::hooks::Hooks;
use crate::combat::{CreatureId, Engine, Flow, PotionId, ValueProps};
use crate::content::cards::TargetType;
use crate::content::potions::{PotionModel, PotionUsage};
use crate::decimal::Decimal;
use crate::run::{Room, RunError, RunState};

/// `DamageVar(12m, ValueProp.Unpowered)`.
const DAMAGE: i32 = 12;
/// `GoldVar(100)`.
const GOLD: i32 = 100;

pub struct FoulPotion;

#[sts2_macros::hooks]
impl Hooks for FoulPotion {
    type Me = PotionId;
}

impl PotionModel for FoulPotion {
    fn name(&self) -> &'static str {
        "Foul Potion"
    }

    fn usage(&self) -> PotionUsage {
        PotionUsage::AnyTime
    }

    /// `AllEnemies` in combat (`TargetedNoCreature` outside it).
    fn target(&self) -> TargetType {
        TargetType::AllEnemies
    }

    /// `CreatureCmd.Damage(CombatState.Creatures.Where(!IsPet), 12, Unpowered, Owner.Creature)`:
    /// the allies, then the enemies.
    fn on_use(&self, eng: &mut Engine<'_>, _target: Option<CreatureId>) -> Flow {
        let player = eng.s.player.creature;
        let targets: Vec<CreatureId> = eng
            .s
            .allies
            .iter()
            .chain(&eng.s.enemies)
            .copied()
            .filter(|&c| eng.s.creature(c).pet_owner.is_none())
            .collect();
        eng.damage(
            &targets,
            Decimal::from(DAMAGE),
            ValueProps::UNPOWERED,
            Some(player),
            None,
        )?;
        Ok(())
    }

    /// `PassesCustomUsabilityCheck` outside combat: a merchant room.
    fn usable_out_of_combat(&self, s: &RunState) -> bool {
        s.room == Room::Shop
    }

    /// `OnUse` in a `MerchantRoom`: `PlayerCmd.GainGold(100)`.
    fn on_use_out_of_combat(&self, s: &mut RunState) -> Result<(), RunError> {
        s.gain_gold(Decimal::from(GOLD));
        Ok(())
    }
}
