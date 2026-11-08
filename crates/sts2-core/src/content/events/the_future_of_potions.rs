//! `TheFutureOfPotions` (`Models.Events/TheFutureOfPotions.cs`): trade one of the first three
//! potions for an upgraded card reward of the matching rarity and a rolled type.

use crate::content::cards::CardType;
use crate::content::events::{EventAction, EventCtx, EventData, EventModel, EventOption, Page};
use crate::run::odds::{CardRarity, PotionRarity};
use crate::run::potions;
use crate::run::rewards::{CardCreation, CardFilter, CardPool, CardReward, Reward};
use crate::run::{RunCard, RunError, RunState};

const OPTION_KEY: &str = "THE_FUTURE_OF_POTIONS.pages.INITIAL.options.POTION";
/// `new CardReward(options, 3, Owner)`.
const REWARD_CARDS: usize = 3;

pub struct TheFutureOfPotions;

/// The event potions (`EventPotionPool`), outside the character and shared pools: `Ambergris`,
/// `FoulPotion` and `GlowwaterPotion`. Their `PotionRarity` is Event, which both uses here treat
/// like Rare (`GetCardRarity` gives Rare; only Common and Token lose the Power type).
const EVENT_POTIONS: [&str; 3] = ["AMBERGRIS", "FOUL_POTION", "GLOWWATER_POTION"];

/// The token potion (`TokenPotionPool`), outside every pool: `PotionShapedRock`, which
/// `PetrifiedToad` puts in the belt at the start of a fight. Token behaves like Common here.
const TOKEN_POTIONS: [&str; 1] = ["POTION_SHAPED_ROCK"];

/// `PotionModel.Rarity` of a potion the run layer knows.
fn potion_rarity(id: &'static str) -> Result<PotionRarity, RunError> {
    if EVENT_POTIONS.contains(&id) {
        return Ok(PotionRarity::Rare);
    }
    if TOKEN_POTIONS.contains(&id) {
        return Ok(PotionRarity::Common);
    }
    potions::potion_rarity(id).ok_or(RunError::UnsupportedPotion(id))
}

/// `GetCardRarity(potion)`: Rare (and Event) to Rare, Uncommon to Uncommon, Common (and Token)
/// to Common.
const fn card_rarity(rarity: PotionRarity) -> CardRarity {
    match rarity {
        PotionRarity::Common => CardRarity::Common,
        PotionRarity::Uncommon => CardRarity::Uncommon,
        PotionRarity::Rare => CardRarity::Rare,
    }
}

impl EventModel for TheFutureOfPotions {
    /// At least two potions.
    fn is_allowed(&self, s: &RunState) -> bool {
        s.potions_held().count() >= 2
    }

    /// `BeforeEventStarted` only locks potion use (`CanUseOrRemovePotions`), which outside combat
    /// nothing reads. Building the first option fills `PotionToCardType`: one `NextItem` on the
    /// event `Rng` per held potion, from Attack/Skill/Power (no Power for Common and Token).
    fn begin(&self, ctx: &mut EventCtx<'_>) -> Result<Vec<EventOption>, RunError> {
        let held: Vec<(usize, &'static str)> = ctx.state().potions_held().collect();
        let mut types = Vec::with_capacity(held.len());
        for &(slot, id) in &held {
            let choices: &[CardType] = if potion_rarity(id)? == PotionRarity::Common {
                &[CardType::Attack, CardType::Skill]
            } else {
                &[CardType::Attack, CardType::Skill, CardType::Power]
            };
            let rolled = *ctx.rng.next_item(choices).expect("non-empty type list");
            types.push((slot, rolled));
        }
        *ctx.data = EventData::FutureOfPotions { types };
        Ok(held
            .iter()
            .take(3)
            .map(|&(slot, _)| EventOption::with_arg(OPTION_KEY, "POTION", slot))
            .collect())
    }

    /// `Trade(potion)`: discard it, then a `CardReward` of 3 uniform character cards of the
    /// potion's card rarity and rolled type, all upgraded (`AfterGenerated`), offered on its own.
    fn choose(&self, action: EventAction, ctx: &mut EventCtx<'_>) -> Result<Page, RunError> {
        let slot = action.arg;
        let EventData::FutureOfPotions { types } = &*ctx.data else {
            unreachable!("TheFutureOfPotions' types are set in begin");
        };
        let card_type = types
            .iter()
            .find(|&&(s, _)| s == slot)
            .map(|&(_, t)| t)
            .expect("every held potion has a type");
        let id = ctx.state().potions[slot].expect("the option's potion is held");
        let rarity = card_rarity(potion_rarity(id)?);
        ctx.state().discard_potion(slot);
        let options = CardCreation::non_combat_uniform(
            CardPool::Character,
            CardFilter::RarityAndType(rarity, card_type),
        )
        .without_pool_modifications()
        .as_card_reward();
        // `AfterGenerated += UpgradeCardsInReward`.
        let mut reward = CardReward::with_cards(ctx.create_cards(REWARD_CARDS, options), options);
        reward.cards.iter_mut().for_each(RunCard::upgrade);
        reward.upgrade_generated = true;
        ctx.offer_rewards(vec![Reward::Card(reward)]);
        Ok(Page::finished("THE_FUTURE_OF_POTIONS", "DONE"))
    }
}
