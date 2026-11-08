//! `WelcomeToWongos` (`Models.Events/WelcomeToWongos.cs`): buy a Common relic (100 gold), the
//! featured Rare relic (200) or `WongosMysteryTicket` (300), or leave and lose an upgrade. Shared,
//! Act 2 only.
//!
//! Every purchase earns Wongo points, kept in the profile (`SaveManager.Progress.WongoPoints`);
//! `WongoCustomerAppreciationBadge` comes with each 2000. The engine has no profile and counts
//! from 0, so a single purchase (32 points at most) never earns the badge.

use crate::content::events::{EventAction, EventCtx, EventData, EventModel, EventOption, Page};
use crate::decimal::Decimal;
use crate::run::relic_bag::RelicRarity;
use crate::run::rewards::pull_relic_of_rarity;
use crate::run::shop::NOT_IN_SHOPS;
use crate::run::{RunError, RunState};

/// `DynamicVar("BargainBinCost", 100m)`.
const BARGAIN_BIN_COST: i32 = 100;
/// `DynamicVar("FeaturedItemCost", 200m)`.
const FEATURED_ITEM_COST: i32 = 200;
/// `DynamicVar("MysteryBoxCost", 300m)`.
const MYSTERY_BOX_COST: i32 = 300;
/// `_wongoPointsForBadge`.
const POINTS_FOR_BADGE: i32 = 2000;
/// The Wongo points of a Bargain Bin, Featured Item and Mystery Box purchase.
const BARGAIN_BIN_POINTS: i32 = 32;
const FEATURED_ITEM_POINTS: i32 = 16;
const MYSTERY_BOX_POINTS: i32 = 8;
/// `SaveManager.Instance.Progress.WongoPoints`: the engine has no profile.
const PROFILE_WONGO_POINTS: i32 = 0;

pub struct WelcomeToWongos;

/// `PullNextRelicFromFront(owner, rarity, r => r.IsAllowedInShops)`.
fn pull_shop_relic(s: &mut RunState, rarity: RelicRarity) -> &'static str {
    pull_relic_of_rarity(s, rarity, |id| !NOT_IN_SHOPS.contains(&id))
}

/// An option, or its `_LOCKED` twin when the player has less than `cost` gold.
fn priced(gold: i32, cost: i32, key: &str, tag: &'static str) -> EventOption {
    let key = format!("WELCOME_TO_WONGOS.pages.INITIAL.options.{key}");
    if gold >= cost {
        EventOption::new(key, tag)
    } else {
        EventOption::locked(format!("{key}_LOCKED"))
    }
}

/// `CheckObtainWongoBadge(pointsEarned)`: the badge once the points since the last badge reach
/// 2000; returns the page the purchase finishes on.
fn check_obtain_wongo_badge(
    ctx: &mut EventCtx<'_>,
    points_earned: i32,
) -> Result<&'static str, RunError> {
    // `Owner.ExtraFields.WongoPoints = pointsEarned`; the page's vars come from it.
    *ctx.data = EventData::Amount(Decimal::from(points_earned));
    if PROFILE_WONGO_POINTS % POINTS_FOR_BADGE + points_earned >= POINTS_FOR_BADGE {
        ctx.obtain_relic("WONGO_CUSTOMER_APPRECIATION_BADGE")?;
        return Ok("AFTER_BUY_RECEIVE_BADGE");
    }
    if (PROFILE_WONGO_POINTS + points_earned) / POINTS_FOR_BADGE > 0 {
        return Ok("AFTER_BUY_BADGE_COUNTER");
    }
    Ok("AFTER_BUY")
}

/// `CheckObtainWongoBadge`'s `WongoPointAmount`, `RemainingWongoPointAmount` and
/// `TotalWongoBadgeAmount` after earning `points_earned`.
pub(super) fn point_vars(points_earned: i32) -> [(&'static str, Decimal); 3] {
    let points = PROFILE_WONGO_POINTS % POINTS_FOR_BADGE + points_earned;
    [
        ("WongoPointAmount", Decimal::from(points)),
        (
            "RemainingWongoPointAmount",
            Decimal::from(POINTS_FOR_BADGE - points),
        ),
        (
            "TotalWongoBadgeAmount",
            Decimal::from((PROFILE_WONGO_POINTS + points_earned) / POINTS_FOR_BADGE),
        ),
    ]
}

impl EventModel for WelcomeToWongos {
    /// `CurrentActIndex == 1` and 100 gold (`WelcomeToWongos.cs:68-75`).
    fn is_allowed(&self, s: &RunState) -> bool {
        s.act_index == 1 && s.gold >= BARGAIN_BIN_COST
    }

    /// `FeaturedItem`: the front Rare relic allowed in shops, pulled whether or not it is
    /// affordable.
    fn begin(&self, ctx: &mut EventCtx<'_>) -> Result<Vec<EventOption>, RunError> {
        let featured = pull_shop_relic(ctx.state(), RelicRarity::Rare);
        *ctx.data = EventData::Relic(featured);
        let gold = ctx.state().gold;
        Ok(vec![
            priced(gold, BARGAIN_BIN_COST, "BARGAIN_BIN", "BARGAIN_BIN"),
            priced(gold, FEATURED_ITEM_COST, "FEATURED_ITEM", "FEATURED_ITEM"),
            priced(gold, MYSTERY_BOX_COST, "MYSTERY_BOX", "MYSTERY_BOX"),
            EventOption::new("WELCOME_TO_WONGOS.pages.INITIAL.options.LEAVE", "LEAVE"),
        ])
    }

    fn choose(&self, action: EventAction, ctx: &mut EventCtx<'_>) -> Result<Page, RunError> {
        let page = match action.tag {
            "BARGAIN_BIN" => {
                ctx.state().lose_gold(BARGAIN_BIN_COST);
                let relic = pull_shop_relic(ctx.state(), RelicRarity::Common);
                ctx.obtain_relic(relic)?;
                check_obtain_wongo_badge(ctx, BARGAIN_BIN_POINTS)?
            }
            "MYSTERY_BOX" => {
                ctx.state().lose_gold(MYSTERY_BOX_COST);
                ctx.obtain_relic("WONGOS_MYSTERY_TICKET")?;
                check_obtain_wongo_badge(ctx, MYSTERY_BOX_POINTS)?
            }
            "FEATURED_ITEM" => {
                let EventData::Relic(featured) = *ctx.data else {
                    unreachable!("the featured relic is set in begin");
                };
                ctx.state().lose_gold(FEATURED_ITEM_COST);
                ctx.obtain_relic(featured)?;
                check_obtain_wongo_badge(ctx, FEATURED_ITEM_POINTS)?
            }
            _ => {
                // `Rng.NextItem` of the upgraded deck cards, then `CardCmd.Downgrade`.
                let s = ctx.state();
                let upgraded: Vec<usize> = (0..s.deck.len())
                    .filter(|&i| s.deck[i].upgrade_level > 0)
                    .collect();
                if let Some(&i) = ctx.rng.next_item(&upgraded) {
                    ctx.state().downgrade(i);
                }
                "LEAVE"
            }
        };
        Ok(Page::finished("WELCOME_TO_WONGOS", page))
    }
}
