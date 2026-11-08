//! `LastingCandy` (`Models.Relics/LastingCandy.cs`): every second combat card reward (counting
//! from the first one it sees) offers one more card, a Power. `IsAllowed`: before the Act 3 treasure.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::cards::CardType;
use crate::content::characters::Character;
use crate::content::relics::{RelicData, RelicModel};
use crate::run::relic_bag::is_before_act3_treasure_chest;
use crate::run::rewards::{pick_card, possible_cards};
use crate::run::{CardCreation, CardOdds, CreationFlags, Reward, RoomType, RunCard, RunState};

pub struct LastingCandy;

/// `LastingCandy`'s fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `CombatRewardsSeen`.
    pub combat_rewards_seen: i32,
}

fn data(s: &RunState, me: usize) -> Data {
    match s.relics[me].data {
        RelicData::LastingCandy(data) => data,
        _ => unreachable!("a LastingCandy has LastingCandy data"),
    }
}

/// `CardPoolFilter`: a Power, and (unless `allow_dupes`) not already among the offered cards.
fn card_pool_filter(id: &str, card_type: CardType, offered: &[RunCard], allow_dupes: bool) -> bool {
    card_type == CardType::Power && (allow_dupes || offered.iter().all(|o| o.id != id))
}

impl RelicModel for LastingCandy {
    /// `IsAllowed` (`LastingCandy.cs:82-100`): never on an Ironclad's first run, else before the
    /// Act 3 treasure.
    fn is_allowed(&self, s: &RunState) -> bool {
        !(s.character == Character::Ironclad && s.unlock.is_first_run())
            && is_before_act3_treasure_chest(s)
    }

    fn name(&self) -> &'static str {
        "Lasting Candy"
    }

    fn init_data(&self) -> RelicData {
        RelicData::LastingCandy(Data::default())
    }

    fn try_modify_card_reward_options(
        &self,
        s: &mut RunState,
        me: usize,
        cards: &mut Vec<RunCard>,
        options: CardCreation,
    ) -> bool {
        let seen = data(s, me).combat_rewards_seen;
        // `IsInTriggeringCombat`.
        let triggering = seen > 0 && seen % 2 == 1;
        let CardOdds::Encounter(kind) = options.odds else {
            return false;
        };
        if !triggering
            || !options.is_card_reward
            || !options.flags.has(CreationFlags::IS_FROM_COMBAT)
        {
            return false;
        }
        let possible = possible_cards(s.character, options);
        let candidates = |allow_dupes| -> Vec<_> {
            possible
                .iter()
                .copied()
                .filter(|c| card_pool_filter(c.id, c.card_type, cards, allow_dupes))
                .collect()
        };
        let mut pool = candidates(false);
        if pool.is_empty() {
            pool = candidates(true);
        }
        if pool.is_empty() {
            return false;
        }
        // `CreateForReward(Owner, 1, options)` with `Source.Other`, `NoModifyHooks` and
        // `NoCardPoolModifications`.
        let card = pick_card(s, pool, CardOdds::Base(kind), true);
        cards.push(card);
        true
    }

    fn before_combat_reward_offered(
        &self,
        s: &mut RunState,
        me: usize,
        rewards: &mut Vec<Reward>,
        _room: RoomType,
    ) {
        if rewards.iter().any(|r| matches!(r, Reward::Card(_)))
            && let RelicData::LastingCandy(data) = &mut s.relics[me].data
        {
            data.combat_rewards_seen += 1;
        }
    }
}

#[sts2_macros::hooks]
impl Hooks for LastingCandy {
    type Me = RelicId;
}
