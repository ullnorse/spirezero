//! `FranticEscape` (`Models.Cards/FranticEscape.cs`): status, cost 1, targets itself, never
//! upgraded. `TheInsatiable`'s Liquify shuffles six in. Playing it raises the `SandpitPower` that
//! targets its owner by 1 (on the first enemy holding a Sandpit), then costs 1 more this combat.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, TargetType};
use crate::content::powers::{PowerData, PowerKind};
use crate::decimal::Decimal;

/// Status card added by `TheInsatiable`.
pub struct FranticEscape;

#[sts2_macros::hooks]
impl Hooks for FranticEscape {
    type Me = CardId;
}

impl CardModel for FranticEscape {
    fn name(&self) -> &'static str {
        "Frantic Escape"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Status
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Status
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn max_upgrade_level(&self) -> u8 {
        0
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let owner = eng.s.player.creature;
        // `GetSandpitEnemy`: the first enemy with a `SandpitPower`; then its first one targeting
        // the card's owner.
        let enemy = eng
            .s
            .enemies
            .iter()
            .copied()
            .find(|&e| eng.s.creature(e).has_power(PowerKind::Sandpit));
        let sandpit = enemy.and_then(|e| {
            eng.s
                .creature(e)
                .powers
                .iter()
                .find_map(|p| match *eng.s.power_data(p.id) {
                    PowerData::Sandpit(d) if d.target == Some(owner) => Some((e, p.id)),
                    _ => None,
                })
        });
        if let Some((enemy, power)) = sandpit {
            eng.modify_power_amount_from_card(
                Some(play.card),
                enemy,
                power,
                Decimal::ONE,
                Some(enemy),
            )?;
        }
        eng.s.energy_cost_mut(play.card).add_this_combat(1);
        Ok(())
    }
}
