//! Run-level choices from what the player sees (`docs/specs/honesty.md`).
//!
//! The policy reads the deck, relics,
//! potions, HP and gold, the map, and the open screen (rewards, shop stock, rest options, event
//! text, ancient relics). Started from RLB5's heuristics (card tiers, deck size, path, rest and
//! upgrade, event deck guards); events and ancients are judged by their text keys and relic ids
//! through [`super::values`], never by applying an option to a copy of the run.

use sts2_core::content::cards::{CardKind, CardRarity, CardType};
use sts2_core::run::{
    CardSelect, MapCoord, MapPointType, RestOption, RestOptionKind, Reward, RunAction, RunCard,
    RunDecision, RunState, SelectPurpose, shop,
};

use super::values;

/// Rest instead of upgrading below this share of max HP (in percent), after act 1.
const REST_HP_PERCENT: i32 = 75;
/// Before the boss, rest below this share of max HP (in percent), after act 1.
const BOSS_REST_HP_PERCENT: i32 = 90;
/// The deck size up to which a card of pick value [`PICK_BAR`] is taken.
const PICK_FREE_DECK: i32 = 12;
/// The pick value a card reward needs while the deck is small.
const PICK_BAR: i32 = 3;
/// The pick bar rises by one per this many cards past [`PICK_FREE_DECK`].
const PICK_BAR_STEP: i32 = 6;
/// The pick value a shop card needs.
const SHOP_CARD_BAR: i32 = 8;
/// The gold a potion purchase leaves room for.
const POTION_GOLD: i32 = 250;

/// The best run-level choice among `legal` (non-empty).
#[must_use]
pub fn choose(s: &RunState, decision: &RunDecision<'_>, legal: &[RunAction]) -> RunAction {
    let pick = match decision {
        RunDecision::Ancient(room) => values::ancient_choice(s, room, legal),
        RunDecision::Event(room) => values::event_choice(s, room, legal),
        RunDecision::Map(_) => walk_choice(s, legal),
        RunDecision::Rewards(rewards) => reward_choice(s, rewards, legal),
        RunDecision::CardSelect(select) => card_select_choice(s, select, legal),
        RunDecision::Bundles(bundles) => bundles
            .iter()
            .enumerate()
            .filter(|(i, _)| legal.contains(&RunAction::SelectBundle(*i)))
            .max_by_key(|(_, cards)| cards.iter().map(|c| card_quality(*c)).sum::<i32>())
            .map(|(i, _)| RunAction::SelectBundle(i)),
        RunDecision::RestSite(options) => rest_choice(s, options, legal),
        RunDecision::Treasure(Some(_)) => Some(RunAction::TakeRelic),
        RunDecision::Shop(shop) => Some(shop_choice(s, shop, legal)),
        _ => None,
    };
    pick.filter(|a| legal.contains(a)).unwrap_or_else(|| {
        // Never drink a potion out of combat by accident.
        legal
            .iter()
            .find(|a| !matches!(a, RunAction::UsePotion(_) | RunAction::DiscardPotion(_)))
            .unwrap_or(&legal[0])
            .clone()
    })
}

/// RLB16: a shop's card removal spent on a (removable) curse, before anything else the run policy would
/// buy; `None` when there is no curse to remove or the removal is not for sale.
#[must_use]
pub fn curse_removal(
    s: &RunState,
    decision: &RunDecision<'_>,
    legal: &[RunAction],
) -> Option<RunAction> {
    let curse = |card: &RunCard| {
        card.is_removable()
            && card
                .kind()
                .is_some_and(|k| k.card_type() == CardType::Curse)
    };
    match decision {
        RunDecision::Shop(shop)
            if legal.contains(&RunAction::RemoveCard)
                && s.deck.iter().any(curse)
                && s.gold >= shop::price(s, shop.removal_base_cost) =>
        {
            Some(RunAction::RemoveCard)
        }
        RunDecision::CardSelect(select)
            if matches!(select.purpose, SelectPurpose::ShopRemoval { .. }) =>
        {
            (0..select.options.len())
                .filter(|i| !select.picked.contains(i))
                .filter(|i| legal.contains(&RunAction::SelectCard(*i)))
                .find(|&i| selected_card(s, select, i).is_some_and(|c| curse(&c)))
                .map(RunAction::SelectCard)
        }
        _ => None,
    }
}

/// What a run state is worth outside a fight: HP and max HP first, then relics, deck quality,
/// potions and gold. The event and ancient value tables are measured in this unit.
#[must_use]
pub fn run_value(s: &RunState) -> i64 {
    let deck: i64 = s.deck.iter().map(|c| i64::from(card_quality(*c)) - 2).sum();
    let relics: i64 = s.relics.iter().map(|r| 30 + relic_bonus(r.kind.id())).sum();
    let potions = i64::try_from(s.potions.iter().filter(|p| p.is_some()).count()).unwrap_or(0);
    3 * i64::from(s.max_hp) + i64::from(s.hp) + i64::from(s.gold) / 8 + relics + deck + 8 * potions
}

/// A crude quality of a run card: its pick value and upgrade; curses are strongly negative.
#[must_use]
pub fn card_quality(card: RunCard) -> i32 {
    let Some(kind) = card.kind() else {
        return 0;
    };
    if kind.card_type() == CardType::Curse {
        return -20;
    }
    if matches!(kind.card_type(), CardType::Status | CardType::Quest) {
        return -5;
    }
    card_base(kind) + i32::from(card.upgrade_level != 0) * (upgrade_gain(kind) + 1) / 2
}

/// The pick value of an unupgraded card: Ironclad's tier list, else rarity, type and cost.
#[must_use]
pub fn card_base(kind: CardKind) -> i32 {
    if let Some(value) = ironclad_card_base(kind.id()) {
        return value;
    }
    let rarity = match kind.rarity() {
        CardRarity::Rare | CardRarity::Ancient => 9,
        CardRarity::Uncommon => 4,
        CardRarity::Event => 3,
        _ => 0,
    };
    let kind_bonus = match kind.card_type() {
        CardType::Power => 4,
        CardType::Attack => 3,
        CardType::Skill => 2,
        _ => 0,
    };
    let cost = kind.cost();
    let cost_bonus = if cost <= 0 {
        1
    } else if cost >= 3 {
        1 - cost
    } else {
        0
    };
    rarity + kind_bonus + cost_bonus
}

/// Ironclad's card tier list (RLB5), 0 (dead weight) to 15 (a win condition).
fn ironclad_card_base(id: &str) -> Option<i32> {
    let value = match id {
        "STRIKE_IRONCLAD" | "DEFEND_IRONCLAD" => 0,
        "DEMON_FORM" => 15,
        "OFFERING" | "FIEND_FIRE" => 14,
        "CORRUPTION" | "BREAK" => 13,
        "BARRICADE" | "FEEL_NO_PAIN" | "DARK_EMBRACE" | "IMPERVIOUS" | "MANGLE" | "FEED"
        | "PYRE" | "CRIMSON_MANTLE" | "UNMOVABLE" => 12,
        "ASHEN_STRIKE" | "MAD_SCIENCE" => 11,
        "INFLAME" | "UPPERCUT" | "BATTLE_TRANCE" | "FORGOTTEN_RITUAL" | "HEMOKINESIS" => 10,
        "POMMEL_STRIKE" | "WHIRLWIND" | "TRUE_GRIT" | "SHRUG_IT_OFF" | "SECOND_WIND"
        | "BURNING_PACT" | "STONE_ARMOR" | "BLUDGEON" | "CONFLAGRATION" | "DOMINATE"
        | "INFERNO" => 9,
        "DRUM_OF_BATTLE" | "VICIOUS" | "AGGRESSION" | "STAMPEDE" | "COLOSSUS" | "HELLRAISER"
        | "CASCADE" | "PACTS_END" | "BLOODLETTING" | "FLAME_BARRIER" | "EXPECT_A_FIGHT"
        | "UNRELENTING" => 8,
        "SWORD_BOOMERANG" | "TWIN_STRIKE" | "HEADBUTT" | "DISMANTLE" | "RAMPAGE" | "SPITE"
        | "THUNDERCLAP" | "TREMBLE" | "ONE_TWO_PUNCH" | "PILLAGE" | "TAUNT" | "BODY_SLAM"
        | "TEAR_ASUNDER" | "THRASH" | "JUGGLING" | "NOT_YET" => 7,
        "ANGER" | "ARMAMENTS" | "HAVOC" | "CINDER" | "BLOOD_WALL" | "STOMP" | "RUPTURE" => 6,
        "IRON_WAVE" | "PERFECTED_STRIKE" | "BULLY" | "MOLTEN_FIST" | "BASH" | "STOKE" => 5,
        "SETUP_STRIKE" | "BREAKTHROUGH" | "FIGHT_ME" | "PRIMAL_FORCE" | "HOWL_FROM_BEYOND"
        | "INFERNAL_BLADE" | "RAGE" => 4,
        "BLAZE" | "DEMONIC_SHIELD" | "MIDNIGHT" | "OUTRAGE" | "TANK" => 3,
        _ => return None,
    };
    Some(value)
}

/// How much an upgrade adds to a card's pick value, 0 to 5.
#[must_use]
pub fn upgrade_gain(kind: CardKind) -> i32 {
    match kind.id() {
        "WHIRLWIND" => 5,
        "BASH" | "DEMON_FORM" | "INFLAME" | "UPPERCUT" | "BARRICADE" | "CORRUPTION"
        | "FIEND_FIRE" | "IMPERVIOUS" | "FEEL_NO_PAIN" | "DARK_EMBRACE" | "OFFERING" | "FEED"
        | "MANGLE" | "CRIMSON_MANTLE" | "UNMOVABLE" | "MAD_SCIENCE" => 4,
        "SHRUG_IT_OFF" | "TRUE_GRIT" | "POMMEL_STRIKE" | "BLUDGEON" | "HEMOKINESIS"
        | "CONFLAGRATION" | "STONE_ARMOR" | "BATTLE_TRANCE" | "SECOND_WIND" | "BURNING_PACT"
        | "FLAME_BARRIER" | "DRUM_OF_BATTLE" | "FORGOTTEN_RITUAL" => 3,
        "STRIKE_IRONCLAD" | "DEFEND_IRONCLAD" => 1,
        _ => 2,
    }
}

/// What adding `card` to the deck is worth now: its tier, upgrade, copies held and a few
/// two-card engines.
fn pick_value(s: &RunState, card: RunCard) -> i32 {
    let Some(kind) = card.kind() else {
        return 0;
    };
    let mut value = card_base(kind) + 2 * i32::from(card.upgrade_level) + upgrade_gain(kind) / 2;
    let deck: Vec<&'static str> = s
        .deck
        .iter()
        .filter_map(|c| c.kind().map(CardKind::id))
        .collect();
    let has = |id: &str| deck.contains(&id);
    let copies = deck.iter().filter(|&&d| d == kind.id()).count();
    let synergy = match kind.id() {
        "FEEL_NO_PAIN" | "DARK_EMBRACE" | "CORRUPTION" => {
            ["FEEL_NO_PAIN", "DARK_EMBRACE", "CORRUPTION"]
                .iter()
                .map(|&id| i32::from(has(id)))
                .sum::<i32>()
                * 2
        }
        "BARRICADE" if has("BODY_SLAM") => 3,
        "BODY_SLAM" if has("BARRICADE") => 4,
        "INFLAME" | "DEMON_FORM"
            if ["WHIRLWIND", "TWIN_STRIKE", "SWORD_BOOMERANG", "FIEND_FIRE"]
                .iter()
                .any(|&id| has(id)) =>
        {
            2
        }
        "WHIRLWIND" | "TWIN_STRIKE" | "SWORD_BOOMERANG" | "FIEND_FIRE"
            if has("INFLAME") || has("DEMON_FORM") =>
        {
            2
        }
        "VICIOUS"
            if [
                "UPPERCUT",
                "THUNDERCLAP",
                "TREMBLE",
                "TAUNT",
                "DOMINATE",
                "MOLTEN_FIST",
                "BASH",
            ]
            .iter()
            .any(|&id| has(id)) =>
        {
            2
        }
        "RUPTURE" if has("BLOODLETTING") || has("HEMOKINESIS") || has("CRIMSON_MANTLE") => 2,
        _ => 0,
    };
    value += synergy;
    // A deck wants one or two copies of a card, not four; powers stack better than attacks.
    let power = kind.card_type() == CardType::Power;
    if copies >= 2 && !power {
        value -= 6;
    } else if copies == 1 && !power {
        value -= 2;
    }
    if copies >= 3 && power {
        value -= 4;
    }
    value
}

/// A value beyond the generic relic count for relics whose payoff is not in the immediate state.
#[must_use]
pub fn relic_bonus(id: &str) -> i64 {
    match id {
        "SOZU" | "ECTOPLASM" | "PHILOSOPHERS_STONE" | "VELVET_CHOKER" | "WHISPERING_EARRING" => 120,
        "SNECKO_EYE" | "RUNIC_PYRAMID" => 100,
        "BLACK_STAR" => 80,
        "CALLING_BELL" | "ASTROLABE" | "EMPTY_CAGE" | "JEWELED_MASK" => 50,
        "TOY_BOX" => 140,
        "GOLDEN_COMPASS" => 70,
        "PANDORAS_BOX" | "ORRERY" => 40,
        "VAJRA" | "VAMBRACE" | "PLANISPHERE" => 35,
        "STRAWBERRY" | "LASTING_CANDY" | "MEAL_TICKET" | "LEES_WAFFLE" => 25,
        "REPTILE_TRINKET" | "PANTOGRAPH" | "POTION_BELT" | "BOOK_OF_FIVE_RINGS" => 20,
        _ => 0,
    }
}

/// A rewards screen: claim everything valuable, pick a good card or skip it, then leave.
fn reward_choice(s: &RunState, rewards: &[Reward], legal: &[RunAction]) -> Option<RunAction> {
    let has_slot = s.potions.contains(&None);
    let skip_or_proceed = |i: usize| {
        Some(RunAction::SkipReward(i))
            .filter(|a| legal.contains(a))
            .or(Some(RunAction::Proceed))
    };
    for (i, reward) in rewards.iter().enumerate() {
        match reward {
            Reward::Card(cards) => {
                let best = cards
                    .cards
                    .iter()
                    .enumerate()
                    .filter(|(card, _)| {
                        legal.contains(&RunAction::PickCard {
                            reward: i,
                            card: *card,
                        })
                    })
                    .max_by_key(|(_, c)| pick_value(s, **c));
                // Every card past a small deck dilutes the draws, so the bar rises with size.
                let size = i32::try_from(s.deck.len()).unwrap_or(i32::MAX);
                let bar = PICK_BAR + (size - PICK_FREE_DECK).max(0) / PICK_BAR_STEP;
                if let Some((card, c)) = best
                    && pick_value(s, *c) >= bar
                {
                    return Some(RunAction::PickCard { reward: i, card });
                }
                return skip_or_proceed(i);
            }
            Reward::Potion(_) if !has_slot => return skip_or_proceed(i),
            _ => {
                let claim = RunAction::ClaimReward(i);
                if legal.contains(&claim) {
                    return Some(claim);
                }
            }
        }
    }
    Some(RunAction::Proceed)
}

/// A deck card selection: upgrade the best card, remove the worst, otherwise take the best.
fn card_select_choice(s: &RunState, select: &CardSelect, legal: &[RunAction]) -> Option<RunAction> {
    let remove = matches!(
        select.purpose,
        SelectPurpose::Remove { .. } | SelectPurpose::Cook | SelectPurpose::ShopRemoval { .. }
    );
    let upgrade = matches!(
        select.purpose,
        SelectPurpose::Upgrade | SelectPurpose::Smith | SelectPurpose::Enchant { .. }
    );
    let chosen = select
        .options
        .iter()
        .enumerate()
        .filter(|(i, _)| !select.picked.contains(i))
        .filter(|(i, _)| legal.contains(&RunAction::SelectCard(*i)))
        .filter_map(|(i, _)| {
            let card = selected_card(s, select, i)?;
            let kind = card.kind()?;
            let mut quality = card_quality(card);
            if upgrade && card.upgrade_level == 0 {
                quality += 3 * upgrade_gain(kind);
            }
            Some((i, quality))
        })
        .max_by_key(|(_, quality)| if remove { -*quality } else { *quality });
    if let Some((i, _)) = chosen {
        return Some(RunAction::SelectCard(i));
    }
    [
        RunAction::ConfirmSelect,
        RunAction::CancelSelect,
        RunAction::Proceed,
    ]
    .into_iter()
    .find(|a| legal.contains(a))
}

/// The card a selection option stands for: a deck index or an index into `offered`.
fn selected_card(s: &RunState, select: &CardSelect, option: usize) -> Option<RunCard> {
    let index = *select.options.get(option)?;
    if select.offered.is_empty() {
        s.deck.get(index).copied()
    } else {
        select.offered.get(index).copied()
    }
}

/// The rest site: heal when hurt, hatch an egg when there is one, otherwise smith.
fn rest_choice(s: &RunState, options: &[RestOption], legal: &[RunAction]) -> Option<RunAction> {
    let before_boss = s
        .coord()
        .is_some_and(|c| c.row + 1 == s.map.boss().coord.row);
    let (rest_percent, boss_percent) = match s.act_index {
        0 => (50, 65),
        _ => (REST_HP_PERCENT, BOSS_REST_HP_PERCENT),
    };
    let threshold = if before_boss {
        boss_percent
    } else {
        rest_percent
    };
    let hurt = s.hp * 100 < s.max_hp * threshold;
    let choose = |kind: RestOptionKind| {
        options
            .iter()
            .position(|o| o.kind == kind && o.enabled)
            .map(RunAction::RestOption)
            .filter(|a| legal.contains(a))
    };
    if hurt && let Some(a) = choose(RestOptionKind::Heal) {
        return Some(a);
    }
    choose(RestOptionKind::Hatch)
        .or_else(|| choose(RestOptionKind::Smith))
        .or_else(|| choose(RestOptionKind::Dig))
        .or_else(|| choose(RestOptionKind::Lift))
        .or_else(|| choose(RestOptionKind::Heal))
        .or_else(|| Some(RunAction::Proceed).filter(|a| legal.contains(a)))
}

/// A shop: buy relics, then a removal, good cards and potions; otherwise leave.
fn shop_choice(s: &RunState, shop: &shop::Shop, legal: &[RunAction]) -> RunAction {
    if let Some(i) = shop
        .relics
        .iter()
        .enumerate()
        .filter(|(i, r)| r.id.is_some() && legal.contains(&RunAction::BuyRelic(*i)))
        .max_by_key(|(_, r)| r.id.map_or(0, relic_bonus) + 10)
        .map(|(i, _)| i)
    {
        return RunAction::BuyRelic(i);
    }
    if legal.contains(&RunAction::RemoveCard)
        && s.deck
            .iter()
            .any(|c| c.is_basic() || c.kind().is_some_and(|k| k.card_type() == CardType::Curse))
        && s.gold > shop::price(s, shop.removal_base_cost)
    {
        return RunAction::RemoveCard;
    }
    if let Some(i) = shop
        .cards
        .iter()
        .enumerate()
        .filter(|(i, c)| c.card.is_some() && legal.contains(&RunAction::BuyCard(*i)))
        .filter(|(_, c)| {
            c.card
                .is_some_and(|card| pick_value(s, card) >= SHOP_CARD_BAR)
        })
        .max_by_key(|(_, c)| c.card.map_or(0, |card| pick_value(s, card)))
        .map(|(i, _)| i)
    {
        return RunAction::BuyCard(i);
    }
    if s.gold > POTION_GOLD
        && s.potions.contains(&None)
        && let Some(i) = shop
            .potions
            .iter()
            .enumerate()
            .filter(|(i, p)| p.id.is_some() && legal.contains(&RunAction::BuyPotion(*i)))
            .max_by_key(|(_, p)| p.base_cost)
            .map(|(i, _)| i)
    {
        return RunAction::BuyPotion(i);
    }
    RunAction::Proceed
}

/// A walk along a map path: the HP and gold it expects to have, and the rewards it collects.
#[derive(Clone, Copy, Debug)]
struct Walk {
    hp: i32,
    gold: i32,
    value: i32,
}

/// The most map points one path choice may visit (a guard; act maps have far fewer paths).
const WALK_BUDGET: usize = 200_000;

/// The map: the child on the best path to the boss, by a walk to the boss: every path from each next point is walked with the
/// expected HP loss and reward of its rooms (`?` by its type only), and the child whose best path
/// ends richest, with the most HP at the boss, is taken.
fn walk_choice(s: &RunState, legal: &[RunAction]) -> Option<RunAction> {
    let start = Walk {
        hp: s.hp,
        gold: s.gold,
        value: 0,
    };
    let mut budget = WALK_BUDGET;
    legal
        .iter()
        .filter_map(|a| match a {
            RunAction::ChooseNode(c) => Some((walk(s, *c, start, &mut budget), a)),
            _ => None,
        })
        .rev()
        .max_by_key(|(v, _)| *v)
        .map(|(_, a)| a.clone())
}

/// The best end value of the paths from `coord` on, having arrived as `w`.
fn walk(s: &RunState, coord: MapCoord, w: Walk, budget: &mut usize) -> i32 {
    let Some(p) = s.map.point(coord) else {
        return end_value(s, w);
    };
    if p.kind == MapPointType::Boss || *budget == 0 {
        return end_value(s, w);
    }
    *budget -= 1;
    let w = room(s, p.kind, w);
    if w.hp <= 0 {
        return w.value - 10_000;
    }
    p.children
        .iter()
        .map(|c| walk(s, c, w, budget))
        .max()
        .unwrap_or_else(|| end_value(s, w))
}

/// A walk's value at the boss: its rewards and its HP, with HP below half of max counting double.
fn end_value(s: &RunState, w: Walk) -> i32 {
    let low = (s.max_hp / 2 - w.hp).max(0);
    w.value + w.hp - low
}

/// One room's expected HP loss and reward.
fn room(s: &RunState, kind: MapPointType, mut w: Walk) -> Walk {
    let act = i32::try_from(s.act_index.min(2)).unwrap_or(2);
    match kind {
        MapPointType::Monster => {
            w.hp -= 8 + 4 * act;
            w.gold += 15;
            w.value += 6;
        }
        MapPointType::Elite => {
            w.hp -= 18 + 8 * act;
            w.gold += 30;
            w.value += 30;
        }
        MapPointType::RestSite => {
            if w.hp * 100 < s.max_hp * 60 {
                w.hp = (w.hp + s.max_hp * 3 / 10).min(s.max_hp);
            } else {
                w.value += 10;
            }
        }
        MapPointType::Shop => {
            w.value += w.gold.min(300) / 10;
            w.gold = 0;
        }
        MapPointType::Unknown => {
            w.hp -= 3;
            w.value += 7;
        }
        MapPointType::Treasure => w.value += 20,
        _ => {}
    }
    w
}
