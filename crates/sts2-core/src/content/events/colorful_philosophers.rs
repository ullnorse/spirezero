//! `ColorfulPhilosophers` (`Models.Events/ColorfulPhilosophers.cs`): pick another character's
//! colour and take a Common, an Uncommon and a Rare card reward (3 cards each) from its pool.
//!
//! `IsAllowed` (more than one unlocked character pool) always holds: every character is unlocked.
//! `NoRarityModification` only matters to a modifier (`BigGameHunter`), which is not modeled.

use crate::content::characters::Character;
use crate::content::events::{EventAction, EventCtx, EventModel, EventOption, Page};
use crate::run::odds::CardRarity;
use crate::run::{
    CardCreation, CardFilter, CardOdds, CardPool, CreationFlags, PendingReward, RunError,
};

/// `CardPoolColorOrder` with each pool's `EnergyColorName.ToUpperInvariant()`.
const COLOR_ORDER: [(Character, &str); 5] = [
    (Character::Necrobinder, "NECROBINDER"),
    (Character::Ironclad, "IRONCLAD"),
    (Character::Regent, "REGENT"),
    (Character::Silent, "SILENT"),
    (Character::Defect, "DEFECT"),
];
/// At most this many colours are offered (`Mathf.Min(3, list.Count)`).
const MAX_OPTIONS: usize = 3;
/// `CardsVar(3)`: the cards of each reward.
const CARDS: usize = 3;

pub struct ColorfulPhilosophers;

impl EventModel for ColorfulPhilosophers {
    /// The other characters' colours in `CardPoolColorOrder`, then `RemoveAt(Rng.NextInt(count))`
    /// until 3 are left.
    fn begin(&self, ctx: &mut EventCtx<'_>) -> Result<Vec<EventOption>, RunError> {
        let own = ctx.state().character;
        let mut options: Vec<EventOption> = (0..COLOR_ORDER.len())
            .filter(|&i| COLOR_ORDER[i].0 != own)
            .map(|i| {
                let key = format!(
                    "COLORFUL_PHILOSOPHERS.pages.INITIAL.options.{}",
                    COLOR_ORDER[i].1
                );
                EventOption::with_arg(key, "OFFER_REWARDS", i)
            })
            .collect();
        while options.len() > MAX_OPTIONS {
            let count = i32::try_from(options.len()).expect("a handful of options");
            let index = usize::try_from(ctx.rng.next_int(count)).expect("NextInt is not negative");
            options.remove(index);
        }
        Ok(options)
    }

    /// `OfferRewards(pool)`: three `CardReward`s (Uniform odds, `NoCardPoolModifications`), one per
    /// rarity, through `RewardsCmd.OfferCustom`.
    fn choose(&self, action: EventAction, ctx: &mut EventCtx<'_>) -> Result<Page, RunError> {
        let pool = CardPool::Of(COLOR_ORDER[action.arg].0);
        let rewards = [CardRarity::Common, CardRarity::Uncommon, CardRarity::Rare]
            .into_iter()
            .map(|rarity| {
                let options = CardCreation {
                    pool,
                    odds: CardOdds::Uniform,
                    filter: CardFilter::Rarity(rarity),
                    upgrade_roll: true,
                    is_card_reward: true,
                    flags: CreationFlags::NO_CARD_POOL_MODIFICATIONS,
                };
                PendingReward::Card(options, CARDS)
            })
            .collect();
        ctx.offer_pending_rewards(rewards);
        Ok(Page::finished("COLORFUL_PHILOSOPHERS", "DONE"))
    }
}
