//! `WingCharm` (`Models.Relics/WingCharm.cs`): one random card of each card reward that `Swift`
//! can enchant (`Rng.Niche`) is offered enchanted with `Swift` 1
//! (`TryModifyCardRewardOptionsLate`).

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::enchantments::{Enchantment, EnchantmentKind};
use crate::content::relics::RelicModel;
use crate::rng::RunRngType;
use crate::run::{CardCreation, RunCard, RunState};

/// `DynamicVar("SwiftAmount", 1m)`.
const SWIFT_AMOUNT: i32 = 1;

pub struct WingCharm;

impl RelicModel for WingCharm {
    fn name(&self) -> &'static str {
        "Wing Charm"
    }

    fn try_modify_card_reward_options_late(
        &self,
        s: &mut RunState,
        _me: usize,
        cards: &mut Vec<RunCard>,
        _options: CardCreation,
    ) -> bool {
        // A card the engine does not port cannot be taken; leave it out of the draw.
        let enchantable: Vec<usize> = (0..cards.len())
            .filter(|&i| {
                cards[i]
                    .can_enchant(EnchantmentKind::Swift)
                    .unwrap_or(false)
            })
            .collect();
        let Some(&i) = s.rng[RunRngType::Niche].next_item(&enchantable) else {
            return false;
        };
        cards[i].enchantment = Some(Enchantment::new(EnchantmentKind::Swift, SWIFT_AMOUNT));
        true
    }
}

#[sts2_macros::hooks]
impl Hooks for WingCharm {
    type Me = RelicId;
}
