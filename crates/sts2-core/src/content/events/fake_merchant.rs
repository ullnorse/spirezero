//! `FakeMerchant` (`Models.Events/FakeMerchant.cs`): a merchant selling 6 of 9 `Fake*` relics.
//! Throwing a `FoulPotion` at him starts `FakeMerchantEventEncounter` instead, whose rewards add
//! `FakeMerchantsRug` and every relic still on sale.
//!
//! The event has a custom layout (`NFakeMerchant`, a merchant inventory screen) and no options of
//! its own (`GenerateInitialOptions` is empty), so its screen is modelled as options with
//! engine-made text keys `FAKE_MERCHANT.pages.INITIAL.options.<X>`: one per relic on sale (locked
//! without enough gold: `PurchaseStatus.FailureGold`), one per `FoulPotion` in the belt
//! (`FoulPotion.OnUse` in this room calls `FoulPotionThrown`), and `LEAVE`.
//! `OnEventFinished` only fills the run history (not modelled).

use crate::content::events::{EventAction, EventCtx, EventData, EventModel, EventOption, Page};
use crate::rng::PlayerRngType;
use crate::run::odds::merchant_relic_cost;
use crate::run::rewards::{PendingReward, Reward};
use crate::run::{RunError, RunState, hooks};
use crate::shuffle::unstable_shuffle;

/// `_inventoryRelics`, in order (`FakeMerchant.cs:29-40`; never `FakeMerchantsRug`).
const INVENTORY_RELICS: [&str; 9] = [
    "FAKE_ANCHOR",
    "FAKE_BLOOD_VIAL",
    "FAKE_HAPPY_FLOWER",
    "FAKE_LEES_WAFFLE",
    "FAKE_MANGO",
    "FAKE_ORICHALCUM",
    "FAKE_SNECKO_EYE",
    "FAKE_STRIKE_DUMMY",
    "FAKE_VENERABLE_TEA_SET",
];
/// `Take(6)`.
const ON_SALE: usize = 6;
/// Every `Fake*` relic's `MerchantCost` (`FakeMerchant.relicCost`).
const RELIC_COST: i32 = 50;
/// `IsAllowed`: the gold a player needs without a `FoulPotion`.
const MIN_GOLD: i32 = 100;
const FOUL_POTION: &str = "FOUL_POTION";
/// The fight's own relic reward.
const RUG: &str = "FAKE_MERCHANTS_RUG";
const ENCOUNTER: &str = "FAKE_MERCHANT_EVENT_ENCOUNTER";

pub struct FakeMerchant;

fn option_key(name: &str) -> String {
    format!("FAKE_MERCHANT.pages.INITIAL.options.{name}")
}

fn relics(data: &EventData) -> &[(Option<&'static str>, i32)] {
    let EventData::FakeMerchant { relics } = data else {
        unreachable!("FakeMerchant's inventory is set in begin");
    };
    relics
}

/// The merchant screen: the relics on sale, a throw per `FoulPotion`, and leaving.
fn options(s: &RunState, data: &EventData) -> Vec<EventOption> {
    let mut options = Vec::new();
    for (i, &(relic, cost)) in relics(data).iter().enumerate() {
        let Some(relic) = relic else { continue };
        // `MerchantEntry.EnoughGold` (`Cost` is the base cost outside a `MerchantRoom`).
        options.push(if cost <= s.gold {
            EventOption::with_arg(option_key(relic), "BUY", i)
        } else {
            EventOption::locked(option_key(relic))
        });
    }
    for (slot, potion) in s.potions.iter().enumerate() {
        if *potion == Some(FOUL_POTION) {
            options.push(EventOption::with_arg(
                option_key(FOUL_POTION),
                "FOUL_POTION",
                slot,
            ));
        }
    }
    options.push(EventOption::new(option_key("LEAVE"), "LEAVE"));
    options
}

impl EventModel for FakeMerchant {
    /// Act 2 on, single player, and 100 gold or a `FoulPotion`.
    fn is_allowed(&self, s: &RunState) -> bool {
        s.act_index >= 1 && (s.gold >= MIN_GOLD || s.potions.contains(&Some(FOUL_POTION)))
    }

    /// `BeforeEventStarted`: `UnstableShuffle(Rng)` of the 9 relics, the first 6 go on sale, each
    /// `MerchantRelicEntry` drawing its cost on `PlayerRng.Shops` in that order (`CalcCost`).
    fn begin(&self, ctx: &mut EventCtx<'_>) -> Result<Vec<EventOption>, RunError> {
        let mut pool = INVENTORY_RELICS;
        unstable_shuffle(&mut pool, ctx.rng);
        let shops = &mut ctx.state().player_rng[PlayerRngType::Shops];
        let relics = pool[..ON_SALE]
            .iter()
            .map(|&relic| (Some(relic), merchant_relic_cost(RELIC_COST, shops)))
            .collect();
        let data = EventData::FakeMerchant { relics };
        let options = options(ctx.state(), &data);
        *ctx.data = data;
        Ok(options)
    }

    fn choose(&self, action: EventAction, ctx: &mut EventCtx<'_>) -> Result<Page, RunError> {
        match action.tag {
            "BUY" => {
                // `MerchantRelicEntry.OnTryPurchase`: `LoseGold`, `RelicCmd.Obtain`; then the
                // entry is cleared (no restock outside a `MerchantRoom`) and `AfterItemPurchased`.
                let (relic, cost) = relics(ctx.data)[action.arg];
                let relic = relic.expect("only stocked relics are offered");
                ctx.state().lose_gold(cost);
                ctx.obtain_relic(relic)?;
                let EventData::FakeMerchant { relics } = &mut *ctx.data else {
                    unreachable!("FakeMerchant's inventory is set in begin");
                };
                relics[action.arg].0 = None;
                hooks::after_item_purchased(ctx.state(), cost);
                let data = ctx.data.clone();
                Ok(Page::Next(options(ctx.state(), &data)))
            }
            "FOUL_POTION" => {
                // `FoulPotion.OnUse` → `FoulPotionThrown`: the rewards are the rug, then every
                // stocked relic, and the fight does not come back to the event.
                ctx.state().discard_potion(action.arg);
                let mut extra = vec![PendingReward::Ready(Reward::Relic(RUG))];
                extra.extend(
                    relics(ctx.data)
                        .iter()
                        .filter_map(|&(relic, _)| relic)
                        .map(|relic| PendingReward::Ready(Reward::Relic(relic))),
                );
                ctx.enter_combat(ENCOUNTER, extra, false)?;
                Ok(Page::left())
            }
            _ => Ok(Page::left()),
        }
    }
}
