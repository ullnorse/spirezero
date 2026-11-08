//! `EndlessConveyor` (`Models.Events/EndlessConveyor.cs`): grab the dish on the belt for 40 gold,
//! as often as the player can pay, or watch the chef and upgrade a card.

use crate::combat::PromptText;
use crate::content::events::{EventAction, EventCtx, EventData, EventModel, EventOption, Page};
use crate::decimal::Decimal;
use crate::rng::PlayerRngType;
use crate::run::odds::roll_weighted;
use crate::run::potions::potion_options;
use crate::run::rewards::{CardCreation, CardPool};
use crate::run::{Reward, RunCard, RunError, RunState};

/// `GoldVar(40)`: what a dish costs.
const COST: i32 = 40;
/// `GoldVar("GoldenFyshGold", 75)`.
const GOLDEN_FYSH_GOLD: i32 = 75;
/// `HealVar("ClamRollHeal", 10m)`.
const CLAM_ROLL_HEAL: i32 = 10;
/// `MaxHpVar("CaviarMaxHp", 4m)`.
const CAVIAR_MAX_HP: i32 = 4;
/// `IsAllowed`: the player needs at least this much gold.
const MIN_GOLD: i32 = 120;
/// Every this many grabs the dish is the Seapunk Salad.
const SALAD_EVERY: u32 = 5;
/// `CardSelectorPrefs(TransformSelectionPrompt, 1)`.
const TRANSFORMS: usize = 1;

pub struct EndlessConveyor;

/// The dish `Dish(id, ..)` ids; the option's `TextKey` is `ENDLESS_CONVEYOR.pages.ALL.options.<id>`.
mod dish {
    pub const CAVIAR: &str = "CAVIAR";
    pub const SPICY_SNAPPY: &str = "SPICY_SNAPPY";
    pub const JELLY_LIVER: &str = "JELLY_LIVER";
    pub const FRIED_EEL: &str = "FRIED_EEL";
    pub const SUSPICIOUS_CONDIMENT: &str = "SUSPICIOUS_CONDIMENT";
    pub const CLAM_ROLL: &str = "CLAM_ROLL";
    pub const GOLDEN_FYSH: &str = "GOLDEN_FYSH";
    pub const SEAPUNK_SALAD: &str = "SEAPUNK_SALAD";
}

/// `_numOfGrabs`, `_lastDishId` and `_currentDish`.
fn state(ctx: &EventCtx<'_>) -> (u32, &'static str, &'static str) {
    let EventData::EndlessConveyor {
        grabs,
        last,
        current,
    } = *ctx.data
    else {
        unreachable!("EndlessConveyor's dishes are set in begin");
    };
    (grabs, last, current)
}

/// `RollDish`: the next dish, from the event's `Rng`.
fn roll_dish(ctx: &mut EventCtx<'_>) {
    let (grabs, mut last, mut current) = state(ctx);
    let grabs = grabs + 1;
    if grabs % SALAD_EVERY == 0 {
        last = dish::SEAPUNK_SALAD;
        current = dish::SEAPUNK_SALAD;
    } else {
        let s = ctx.state();
        let mut dishes = vec![
            (dish::CAVIAR, 6),
            (dish::SPICY_SNAPPY, 3),
            (dish::JELLY_LIVER, 3),
            (dish::FRIED_EEL, 3),
        ];
        if s.potions.iter().any(Option::is_none) {
            dishes.push((dish::SUSPICIOUS_CONDIMENT, 3));
        }
        if s.hp != s.max_hp {
            dishes.push((dish::CLAM_ROLL, 6));
        }
        if grabs > 1 {
            dishes.push((dish::GOLDEN_FYSH, 1));
        }
        dishes.retain(|&(id, _)| id != last);
        let weights: Vec<u8> = dishes.iter().map(|&(_, weight)| weight).collect();
        if let Some(i) = roll_weighted(&weights, ctx.rng) {
            last = dishes[i].0;
            current = last;
        }
    }
    *ctx.data = EventData::EndlessConveyor {
        grabs,
        last,
        current,
    };
}

/// `GenerateGrabSomethingOffTheBeltOption`: the current dish, or a locked option without the gold.
fn grab_option(ctx: &mut EventCtx<'_>) -> EventOption {
    if ctx.state().gold >= COST {
        let key = format!("ENDLESS_CONVEYOR.pages.ALL.options.{}", state(ctx).2);
        EventOption::new(key, "GRAB")
    } else {
        EventOption::locked("ENDLESS_CONVEYOR.pages.ALL.options.LOCKED")
    }
}

/// `SetEventState` after a grab: the next dish and `Leave`.
fn belt_page(ctx: &mut EventCtx<'_>) -> Page {
    Page::Next(vec![
        grab_option(ctx),
        EventOption::new(
            "ENDLESS_CONVEYOR.pages.GRAB_SOMETHING_OFF_THE_BELT.options.LEAVE",
            "LEAVE",
        ),
    ])
}

/// `CardCmd.Upgrade(Rng.NextItem(upgradable deck cards))`, if there are any.
fn upgrade_random(ctx: &mut EventCtx<'_>) {
    let upgradable = ctx.state().upgradable_cards();
    if let Some(&i) = ctx.rng.next_item(&upgradable) {
        ctx.state().deck[i].upgrade();
    }
}

/// `SuspiciousCondiment`: one potion of the character's and the shared pools, on the player's
/// `rewards` stream (as `Wellspring`), offered as a reward.
fn suspicious_condiment(ctx: &mut EventCtx<'_>) {
    let potions: Vec<&'static str> = potion_options(ctx.state().character)
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    let rng = &mut ctx.state().player_rng[PlayerRngType::Rewards];
    if let Some(&potion) = rng.next_item(&potions) {
        ctx.offer_rewards(vec![Reward::Potion(potion)]);
    }
}

impl EventModel for EndlessConveyor {
    fn is_allowed(&self, s: &RunState) -> bool {
        s.gold >= MIN_GOLD
    }

    /// `CalculateVars` rolls the first dish.
    fn begin(&self, ctx: &mut EventCtx<'_>) -> Result<Vec<EventOption>, RunError> {
        *ctx.data = EventData::EndlessConveyor {
            grabs: 0,
            last: "",
            current: "",
        };
        roll_dish(ctx);
        Ok(vec![
            grab_option(ctx),
            EventOption::new(
                "ENDLESS_CONVEYOR.pages.INITIAL.options.OBSERVE_CHEF",
                "OBSERVE_CHEF",
            ),
        ])
    }

    fn choose(&self, action: EventAction, ctx: &mut EventCtx<'_>) -> Result<Page, RunError> {
        match action.tag {
            "GRAB" => {
                let (_, _, current) = state(ctx);
                if current != dish::GOLDEN_FYSH {
                    ctx.state().lose_gold(COST);
                }
                match current {
                    dish::CAVIAR => ctx.state().gain_max_hp(CAVIAR_MAX_HP),
                    dish::SPICY_SNAPPY => upgrade_random(ctx),
                    dish::JELLY_LIVER => {
                        let options = ctx.state().transformable_cards();
                        ctx.select_deck_cards(action, PromptText::Transform, options, TRANSFORMS)?;
                    }
                    dish::FRIED_EEL => {
                        let options = CardCreation::non_combat_default(CardPool::Colorless);
                        let card = ctx.create_cards(1, options)[0];
                        ctx.state().add_card(card);
                    }
                    dish::SUSPICIOUS_CONDIMENT => suspicious_condiment(ctx),
                    dish::CLAM_ROLL => ctx.state().heal(Decimal::from(CLAM_ROLL_HEAL)),
                    dish::GOLDEN_FYSH => ctx.state().gain_gold(Decimal::from(GOLDEN_FYSH_GOLD)),
                    dish::SEAPUNK_SALAD => ctx.state().add_card(RunCard::new("FEEDING_FRENZY")),
                    _ => unreachable!("{current} is not a dish"),
                }
                // `RollDish` and the next page wait for the dish's prompt.
                ctx.then(action);
                Ok(Page::Next(Vec::new()))
            }
            "OBSERVE_CHEF" => {
                upgrade_random(ctx);
                Ok(Page::finished("ENDLESS_CONVEYOR", "OBSERVE_CHEF"))
            }
            _ => Ok(Page::finished("ENDLESS_CONVEYOR", "LEAVE")),
        }
    }

    /// `JellyLiver` after `FromDeckForTransformation`: `TransformToRandom` on the event's `Rng`.
    fn on_cards_selected(
        &self,
        _action: EventAction,
        ctx: &mut EventCtx<'_>,
        cards: &[usize],
    ) -> Result<(), RunError> {
        for &i in cards {
            ctx.transform_to_random(i)?;
        }
        Ok(())
    }

    /// The end of `GrabSomethingOffTheBelt`: `RollDish`, then the next page.
    fn after_prompt(&self, _action: EventAction, ctx: &mut EventCtx<'_>) -> Page {
        roll_dish(ctx);
        belt_page(ctx)
    }
}
