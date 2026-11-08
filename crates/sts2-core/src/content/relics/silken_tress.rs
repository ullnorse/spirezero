//! `SilkenTress` (`Models.Relics/SilkenTress.cs`): `AfterObtained` loses all gold; the first card
//! reward after it offers its cards enchanted with `Glam` (those that can take it), then the
//! relic is used up.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::enchantments::{Enchantment, EnchantmentKind};
use crate::content::relics::{RelicData, RelicModel};
use crate::run::{CardCreation, RunCard, RunCtx, RunError, RunState};

/// `CardCmd.Enchant<Glam>(card, 1m)`.
const GLAM: i32 = 1;

pub struct SilkenTress;

/// `SilkenTress`'s fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `IsUsed`.
    pub is_used: bool,
}

fn data(s: &RunState, me: usize) -> Data {
    match s.relics[me].data {
        RelicData::SilkenTress(data) => data,
        _ => unreachable!("a SilkenTress has SilkenTress data"),
    }
}

impl RelicModel for SilkenTress {
    fn name(&self) -> &'static str {
        "Silken Tress"
    }

    fn init_data(&self) -> RelicData {
        RelicData::SilkenTress(Data::default())
    }

    /// `IsUsedUp => IsUsed`.
    fn is_used_up(&self, data: &RelicData) -> bool {
        matches!(data, RelicData::SilkenTress(d) if d.is_used)
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        let gold = ctx.state().gold;
        ctx.state().lose_gold(gold);
        Ok(())
    }

    /// `TryModifyCardRewardOptionsLate`: on a card reward while unused, each card `Glam` can
    /// enchant is replaced by an enchanted clone (`ModifyCard`). A card the engine does not port
    /// is left alone (its keywords and cost are unknown; it cannot be taken either).
    fn try_modify_card_reward_options_late(
        &self,
        s: &mut RunState,
        me: usize,
        cards: &mut Vec<RunCard>,
        options: CardCreation,
    ) -> bool {
        if !options.is_card_reward || data(s, me).is_used {
            return false;
        }
        for card in cards.iter_mut() {
            if card.can_enchant(EnchantmentKind::Glam).unwrap_or(false) {
                card.enchantment = Some(Enchantment::new(EnchantmentKind::Glam, GLAM));
            }
        }
        true
    }

    fn after_modifying_card_reward_options(&self, s: &mut RunState, me: usize) {
        if let RelicData::SilkenTress(data) = &mut s.relics[me].data {
            data.is_used = true;
        }
    }
}

#[sts2_macros::hooks]
impl Hooks for SilkenTress {
    type Me = RelicId;
}
