//! Run-level hook dispatch (`Hooks/Hook.cs` over `RunState.IterateHookListeners(null)`).
//!
//! Outside combat the listeners are the deck cards (each followed by its enchantment), then the
//! relics in order, then the potions, modifiers and badges (`Runs/RunState.cs:545-575`). Among
//! the ported models only relics and three deck cards (`Guilty`, `Dowsing`, `LanternKey`) have
//! run-level hooks; the cards' pass comes before the relics'. Each relic's hook is a [`RelicModel`] method, called
//! with the relic's index.

use smallvec::SmallVec;

use super::RunCard;
use super::map::{ActMap, MapError, MapPointType};
use super::modifiers;
use super::rewards::{CardAlternatives, CardCreation, PendingReward, Reward};
use super::shop::Shop;
use super::state::{RoomType, RunState};
use super::{RestOption, RestOptionKind};
use crate::content::potions::PotionKind;
use crate::content::relics::{RelicKind, RelicModel, winged_boots};
use crate::decimal::Decimal;

/// `Player.IsActiveForHooks` outside combat: a player who has died (`CreatureCmd.Kill` →
/// `DeactivateHooks` once no `ShouldDie` prevents it, `Commands/CreatureCmd.cs:583`) contributes
/// no listeners (`Runs/RunState.cs:550,567`); a solo player never revives. Only the
/// `ShouldDie` pass itself runs at 0 HP with the player still active.
const fn hooks_active(s: &RunState) -> bool {
    s.hp > 0
}

/// The relics as listeners: index and model, as of the start of the pass; melted relics do not
/// listen (`RunState.IterateHookListeners`, `Runs/RunState.cs:569`), nor do a dead player's.
fn listeners(s: &RunState) -> SmallVec<[(usize, RelicKind); 16]> {
    if !hooks_active(s) {
        return SmallVec::new();
    }
    relic_listeners(s)
}

/// [`listeners`] whether or not the player is active (for `ShouldDie`).
fn relic_listeners(s: &RunState) -> SmallVec<[(usize, RelicKind); 16]> {
    s.relics
        .iter()
        .enumerate()
        .filter(|(_, r)| !r.melted)
        .map(|(me, r)| (me, r.kind))
        .collect()
}

/// Calls `f` for each relic listener still owned, in order.
fn each(s: &mut RunState, mut f: impl FnMut(&mut RunState, usize, &'static dyn RelicModel)) {
    for (me, kind) in listeners(s) {
        if s.relics.get(me).is_some_and(|r| r.kind == kind) {
            f(s, me, kind.model());
        }
    }
}

/// `Hook.AfterRoomEntered` for a room without a fight (`Hook.cs:1128`).
pub fn after_room_entered(s: &mut RunState, room: RoomType) {
    each(s, |s, me, m| {
        m.after_room_entered_out_of_combat(s, me, room);
    });
}

/// The merchant-room part of `Hook.AfterRoomEntered`, right after [`after_room_entered`]: the
/// relics that act on the shop's inventory (`LordsParasol`).
pub fn after_merchant_room_entered(s: &RunState, shop: &mut Shop) {
    for (me, kind) in listeners(s) {
        kind.model().after_merchant_room_entered(s, me, shop);
    }
}

/// The deck half of `Hook.AfterCombatEnd` (`Hook.cs:328`), once the fight is over: the deck
/// cards (`Guilty`), then the relics.
pub fn after_combat_end_on_deck(s: &mut RunState, room: RoomType) {
    for card in deck_listeners(s) {
        // `Guilty.AfterCombatEnd`: `CombatsSeen++`; at 5 it leaves the deck.
        if card.id == "GUILTY"
            && let Some(i) = s.deck.iter().position(|c| *c == card)
        {
            s.deck[i].counter += 1;
            if s.deck[i].counter >= GUILTY_COMBATS {
                s.remove_card(i);
            }
        }
    }
    each(s, |s, me, m| m.after_combat_end_on_deck(s, me, room));
}

/// `Guilty.maxCombats`.
const GUILTY_COMBATS: u32 = 5;
/// `Dowsing.maxRooms`.
const DOWSING_ROOMS: u32 = 5;

/// The deck cards as listeners, as of the start of the pass. A card is found again by value
/// (equal cards act alike, and the first equal one is the earlier listener). A dead player's
/// deck does not listen (see [`hooks_active`]).
fn deck_listeners(s: &RunState) -> Vec<RunCard> {
    if hooks_active(s) {
        s.deck.clone()
    } else {
        Vec::new()
    }
}

/// Whether a deck `LanternKey` acts: its two run hooks only act in Glory (`CurrentActIndex` 2)
/// and come before the relics'. Several keys give the same answer.
fn has_lantern_key_in_glory(s: &RunState) -> bool {
    s.act_index == LANTERN_KEY_ACT && deck_listeners(s).iter().any(|c| c.id == "LANTERN_KEY")
}

/// `LanternKey._gloryActIndex`.
const LANTERN_KEY_ACT: usize = 2;

/// `Hook.ShouldProcurePotion` outside combat (`Hook.cs:2416-2426`): no relic vetoes.
#[must_use]
pub fn should_procure_potion(s: &RunState) -> bool {
    listeners(s)
        .into_iter()
        .all(|(me, kind)| kind.model().should_procure_potion_out_of_combat(s, me))
}

/// `Hook.ShouldAllowFreeTravel` (`Hook.cs:2493`): any listener allows it; `WingedBoots` is the
/// only ported one.
#[must_use]
pub fn should_allow_free_travel(s: &RunState) -> bool {
    listeners(s)
        .into_iter()
        .any(|(me, kind)| kind == RelicKind::WingedBoots && winged_boots::allows_free_travel(s, me))
        || modifiers::allows_free_travel(s)
}

/// The part of `Hook.AfterRoomEntered` that reads how the room was reached, for any room
/// entered from the map: `WingedBoots` counts a free travel.
pub fn after_map_room_entered(s: &mut RunState) {
    for (me, kind) in listeners(s) {
        if kind == RelicKind::WingedBoots {
            winged_boots::after_room_entered(s, me);
        }
    }
}

/// `Hook.BeforeRoomEntered` (`Hook.cs`) for a room entered from the map (never a `MapRoom`, and
/// the only room on the stack): the deck cards (`Dowsing`); no ported relic listens.
pub fn before_room_entered(s: &mut RunState, point: MapPointType) {
    for card in deck_listeners(s) {
        // `Dowsing.BeforeRoomEntered`: a `?` point counts; at 5 the quest is complete and the
        // card becomes `Abundance` (`CardCmd.TransformTo`).
        if card.id == "DOWSING"
            && point == MapPointType::Unknown
            && let Some(i) = s.deck.iter().position(|c| *c == card)
        {
            s.deck[i].counter += 1;
            if s.deck[i].counter >= DOWSING_ROOMS {
                s.transform_to(i, RunCard::new("ABUNDANCE"));
            }
        }
    }
}

/// A `Hook.ShouldDie` preventer outside combat.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeathPreventer {
    /// The relic at this index.
    Relic(usize),
    /// The potion in this belt slot (`FairyInABottle`).
    Potion(usize),
}

/// `Hook.ShouldDie` (`Hook.cs:2237`) on the player outside combat: the preventer, if any.
///
/// Every listener's `ShouldDie`, then every listener's `ShouldDieLate`; the first that says no
/// prevents the death. Of the
/// ported run-level listeners only `FairyInABottle` (`ShouldDie`, after the deck and relics) and
/// the relics' `ShouldDieLate` (`LizardTail`) can.
#[must_use]
pub fn should_die(s: &RunState) -> Option<DeathPreventer> {
    let fairy = s
        .potions
        .iter()
        .position(|p| *p == Some("FAIRY_IN_A_BOTTLE"));
    fairy.map(DeathPreventer::Potion).or_else(|| {
        relic_listeners(s)
            .into_iter()
            .find(|&(me, kind)| !kind.model().should_die_late_out_of_combat(s, me))
            .map(|(me, _)| DeathPreventer::Relic(me))
    })
}

/// `Hook.AfterPreventingDeath` (`Hook.cs:1056`): only the preventer is called. A potion is used
/// (`FairyInABottle.AfterPreventingDeath` → `OnUseWrapper`: the slot empties, then `OnUse`).
///
/// # Panics
/// If the preventer is not one [`should_die`] returned.
pub fn after_preventing_death(s: &mut RunState, preventer: DeathPreventer) {
    match preventer {
        DeathPreventer::Relic(me) => {
            let kind = s.relics[me].kind;
            kind.model().after_preventing_death_out_of_combat(s, me);
        }
        DeathPreventer::Potion(slot) => {
            let id = s.potions[slot]
                .take()
                .expect("the preventer is in the belt");
            let kind = PotionKind::from_id(id).expect("a death preventer is ported");
            kind.use_out_of_combat(s)
                .expect("FairyInABottle's OnUse is ported");
        }
    }
}

/// `Hook.ShouldGenerateTreasure` (`Hook.cs:2324`): every listener must agree.
#[must_use]
pub fn should_generate_treasure(s: &RunState) -> bool {
    listeners(s)
        .into_iter()
        .all(|(me, kind)| kind.model().should_generate_treasure(s, me))
}

/// `Hook.ShouldForcePotionReward` (`Hook.cs:2480`): any listener forces it.
#[must_use]
pub fn should_force_potion_reward(s: &RunState, room: RoomType) -> bool {
    listeners(s)
        .into_iter()
        .any(|(me, kind)| kind.model().should_force_potion_reward(s, me, room))
}

/// `Hook.ModifyRewards` (`Hook.cs:1989`): `TryModifyRewards`, then `TryModifyRewardsLate`. Returns
/// the rewards the listeners added (unpopulated) and the listeners that modified.
pub fn modify_rewards(
    s: &mut RunState,
    room: Option<RoomType>,
    rewards: &mut [Reward],
) -> (Vec<PendingReward>, Vec<usize>) {
    let mut added = Vec::new();
    let mut modifiers = Vec::new();
    each(s, |s, me, m| {
        if m.try_modify_rewards(s, me, room, rewards, &mut added) {
            modifiers.push(me);
        }
    });
    each(s, |s, me, m| {
        if m.try_modify_rewards_late(s, me, room, rewards, &mut added) {
            modifiers.push(me);
        }
    });
    let mut pending: Vec<PendingReward> = std::mem::take(&mut added);
    modifiers::modify_rewards_late(s, room, rewards, &mut pending);
    added = pending;
    (added, modifiers)
}

/// `Hook.ModifyCardRewardAlternatives`: each listener may add to `alternatives`.
pub fn modify_card_reward_alternatives(s: &RunState, alternatives: &mut CardAlternatives) {
    for (me, kind) in listeners(s) {
        kind.model()
            .try_modify_card_reward_alternatives(s, me, alternatives);
    }
}

/// `Hook.AfterModifyingRewards` (`Hook.cs:841`): the modifiers, in listener order.
pub fn after_modifying_rewards(s: &mut RunState, modifiers: &[usize]) {
    each(s, |s, me, m| {
        if modifiers.contains(&me) {
            m.after_modifying_rewards(s, me);
        }
    });
}

/// `Hook.BeforeCombatRewardOffered` (`Hook.cs:340`).
pub fn before_combat_reward_offered(s: &mut RunState, rewards: &mut Vec<Reward>, room: RoomType) {
    each(s, |s, me, m| {
        m.before_combat_reward_offered(s, me, rewards, room);
    });
}

/// `Hook.ModifyCardRewardCreationOptions` (`Hook.cs:1438`): normal, then `Late`.
#[must_use]
pub fn modify_card_reward_creation_options(s: &RunState, options: CardCreation) -> CardCreation {
    let listeners = listeners(s);
    let options = listeners.iter().fold(options, |o, &(me, kind)| {
        kind.model().modify_card_reward_creation_options(s, me, o)
    });
    let options = modifiers::modify_card_reward_creation_options(s, options);
    listeners.iter().fold(options, |o, &(me, kind)| {
        kind.model()
            .modify_card_reward_creation_options_late(s, me, o)
    })
}

/// `Hook.TryModifyCardRewardOptions` (`Hook.cs:1454`) and, if any listener modified,
/// `AfterModifyingCardRewardOptions` (`Hook.cs:691`) for those listeners.
pub fn modify_card_reward_options(
    s: &mut RunState,
    cards: &mut Vec<RunCard>,
    options: CardCreation,
) {
    let listeners = listeners(s);
    let mut modifiers = Vec::new();
    for &(me, kind) in &listeners {
        if kind
            .model()
            .try_modify_card_reward_options(s, me, cards, options)
        {
            modifiers.push(me);
        }
    }
    for &(me, kind) in &listeners {
        if kind
            .model()
            .try_modify_card_reward_options_late(s, me, cards, options)
        {
            modifiers.push(me);
        }
    }
    if !modifiers.is_empty() {
        each(s, |s, me, m| {
            if modifiers.contains(&me) {
                m.after_modifying_card_reward_options(s, me);
            }
        });
    }
}

/// `CardReward.OnRelicObtained` (`Rewards/CardReward.cs:158-169`).
///
/// A relic obtained while a card reward is up modifies its cards at once
/// (`TryModifyCardRewardOptions`, then `Late`, each followed by the relic's
/// `AfterModifyingRewards` (not `AfterModifyingCardRewardOptions`) when it modified).
pub fn modify_card_reward_for_obtained_relic(
    s: &mut RunState,
    me: usize,
    cards: &mut Vec<RunCard>,
    options: CardCreation,
) {
    let model = s.relics[me].kind.model();
    if model.try_modify_card_reward_options(s, me, cards, options) {
        model.after_modifying_rewards(s, me);
    }
    if model.try_modify_card_reward_options_late(s, me, cards, options) {
        model.after_modifying_rewards(s, me);
    }
}

/// `Hook.ModifyMerchantCardCreationResults` (`Hook.cs:1791`).
pub fn modify_merchant_card_creation_results(s: &RunState, cards: &mut [RunCard]) {
    for (me, kind) in listeners(s) {
        kind.model()
            .modify_merchant_card_creation_results(s, me, cards);
    }
}

/// `Hook.ModifyRestSiteHealRewards`: the rewards a rest-site heal also offers, unpopulated.
#[must_use]
pub fn modify_rest_site_heal_rewards(s: &RunState, is_mimicked: bool) -> Vec<PendingReward> {
    let mut rewards = Vec::new();
    for (me, kind) in listeners(s) {
        kind.model()
            .try_modify_rest_site_heal_rewards(s, me, &mut rewards, is_mimicked);
    }
    rewards
}

/// The run-state part of `Hook.AfterRoomEntered` for a fight room entered from the map (see
/// [`RelicModel::after_map_combat_room_entered`]).
pub fn after_map_combat_room_entered(s: &mut RunState, point: MapPointType) {
    each(s, |s, me, m| m.after_map_combat_room_entered(s, me, point));
}

/// `Hook.ModifyCardBeingAddedToDeck` (`Hook.cs:1357`): normal, then `Late`; each replacement
/// feeds the next listener.
#[must_use]
pub fn modify_card_being_added_to_deck(s: &RunState, card: RunCard) -> RunCard {
    let listeners = listeners(s);
    let card = listeners.iter().fold(card, |c, &(me, kind)| {
        kind.model()
            .try_modify_card_being_added_to_deck(s, me, c)
            .unwrap_or(c)
    });
    listeners.iter().fold(card, |c, &(me, kind)| {
        kind.model()
            .try_modify_card_being_added_to_deck_late(s, me, c)
            .unwrap_or(c)
    })
}

/// `Hook.BeforeCardRemoved` (`Hook.cs:299`): a `SpoilsMap` leaving the deck in Act 2 with its
/// `SpoilsCoord` set takes its quest marker off the map (`SpoilsMap.cs:99-115`), then the relics.
pub fn before_card_removed(s: &mut RunState, card: RunCard) {
    if card.id == super::SPOILS_MAP && card.counter > 0 && s.act_index == SPOILS_ACT {
        s.spoils_quest = None;
    }
    each(s, |s, me, m| m.before_card_removed(s, me, card));
}

/// `Hook.AfterCardChangedPiles(card, oldPileType)` (`Hook.cs:167`) for a card that joined the
/// deck: after `CardPileCmd.Add` to the deck, and for a deck transform's replacement. No ported
/// deck card listens.
pub fn after_card_added_to_deck(s: &mut RunState, card: RunCard) {
    after_card_added_to_deck_by_relics(s, card);
    modifiers::after_card_added_to_deck(s, card);
}

/// [`after_card_added_to_deck`] for the relics only (`Hoarder`'s clones skip the modifier).
pub fn after_card_added_to_deck_by_relics(s: &mut RunState, card: RunCard) {
    each(s, |s, me, m| m.after_card_added_to_deck(s, me, card));
}

/// `Hook.ModifyRestSiteOptions` (`Hook.cs:1957`): the deck cards (each `ByrdonisEgg` adds a
/// Hatch option), then the relics.
pub fn modify_rest_site_options(s: &RunState, options: &mut Vec<RestOption>) {
    for card in deck_listeners(s) {
        if card.id == "BYRDONIS_EGG" {
            options.push(RestOption {
                kind: RestOptionKind::Hatch,
                enabled: true,
            });
        }
    }
    for (me, kind) in listeners(s) {
        kind.model().try_modify_rest_site_options(s, me, options);
    }
    modifiers::modify_rest_site_options(s, options);
}

/// `Hook.ShouldDisableRemainingRestSiteOptions` (`Hook.cs:2262`): every listener must agree.
#[must_use]
pub fn should_disable_remaining_rest_site_options(s: &RunState) -> bool {
    listeners(s).into_iter().all(|(me, kind)| {
        kind.model()
            .should_disable_remaining_rest_site_options(s, me)
    })
}

/// `Hook.ModifyRestSiteHealAmount` (`Hook.cs:1944`).
#[must_use]
pub fn modify_rest_site_heal_amount(s: &RunState, amount: Decimal) -> Decimal {
    let amount = listeners(s).into_iter().fold(amount, |a, (me, kind)| {
        kind.model().modify_rest_site_heal_amount(s, me, a)
    });
    modifiers::modify_rest_site_heal_amount(s, amount)
}

/// `Hook.ModifyHpLost` in both phases for damage to the player outside combat: only the
/// `AfterOsty` phase has a ported listener there (`TungstenRod`); `AfterModifyingHpLost*` only
/// flashes.
#[must_use]
pub fn modify_hp_lost_out_of_combat(s: &RunState, amount: Decimal) -> Decimal {
    listeners(s).into_iter().fold(amount, |a, (me, kind)| {
        kind.model().modify_hp_lost_out_of_combat(s, me, a)
    })
}

/// `Hook.AfterRestSiteHeal` (`Hook.cs:1080`).
pub fn after_rest_site_heal(s: &mut RunState, is_mimicked: bool) {
    each(s, |s, me, m| m.after_rest_site_heal(s, me, is_mimicked));
    modifiers::after_rest_site_heal(s, is_mimicked);
}

/// `Hook.AfterRestSiteSmith` (`Hook.cs:1092`).
pub fn after_rest_site_smith(s: &mut RunState) {
    each(s, |s, me, m| m.after_rest_site_smith(s, me));
}

/// `Hook.ModifyGoldGained` (`Hook.cs:1634`) outside combat: the folded amount and the
/// listeners that changed `(int)` gold.
#[must_use]
pub fn modify_gold_gained(s: &RunState, amount: Decimal) -> (Decimal, Vec<usize>) {
    let mut modifiers = Vec::new();
    let mut gold = amount;
    for (me, kind) in listeners(s) {
        let before = gold;
        gold = kind.model().modify_gold_gained_out_of_combat(s, me, gold);
        if before.to_int_truncating() != gold.to_int_truncating() {
            modifiers.push(me);
        }
    }
    (gold, modifiers)
}

/// `Hook.AfterModifyingGoldGained` (`Hook.cs:736`): the modifiers, in listener order.
pub fn after_modifying_gold_gained(s: &mut RunState, modifiers: &[usize], amount: Decimal) {
    each(s, |s, me, m| {
        if modifiers.contains(&me) {
            m.after_modifying_gold_gained_out_of_combat(s, me, amount);
        }
    });
}

/// `Hook.AfterGoldGained` (`Hook.cs:481`).
pub fn after_gold_gained(s: &mut RunState) {
    each(s, |s, me, m| m.after_gold_gained_out_of_combat(s, me));
}

/// `Hook.AfterItemPurchased` (`Hook.cs:637`).
pub fn after_item_purchased(s: &mut RunState, gold_spent: i32) {
    each(s, |s, me, m| m.after_item_purchased(s, me, gold_spent));
}

/// `Hook.ModifyGeneratedMap` (`Hook.cs:1666-1675`) over the map of act `act_index`.
///
/// Then [`modify_generated_map_late`]. The deck cards come first: a `SpoilsMap` (its
/// `SpoilsActIndex` is 1, the only act it can be taken in) swaps in the hourglass
/// `SpoilsActMap` (`SpoilsMap.cs:49-61`).
///
/// # Errors
/// Where a replacement map's generation throws.
pub fn modify_generated_map(
    s: &mut RunState,
    map: &mut ActMap,
    act_index: usize,
) -> Result<(), MapError> {
    for card in deck_listeners(s) {
        if card.id == super::SPOILS_MAP && act_index == SPOILS_ACT {
            *map = ActMap::generate_spoils(&s.rng, act_index, s.ascension)?;
        }
    }
    for (me, kind) in listeners(s) {
        if s.relics.get(me).is_some_and(|r| r.kind == kind) {
            kind.model().modify_generated_map(s, me, map, act_index)?;
        }
    }
    if let Some(replacement) = modifiers::modify_generated_map(s, map, act_index)? {
        *map = replacement;
    }
    modify_generated_map_late(s, map, act_index)
}

/// `SpoilsMap.SpoilsActIndex`.
const SPOILS_ACT: usize = 1;

/// `Hook.ModifyGeneratedMapLate` (`Hook.cs:1679-1687`).
///
/// The deck cards first: a `SpoilsMap` in Act 2 sets its `SpoilsCoord` to the map's first treasure point (`SpoilsMap.cs:63-80`; the
/// deck card's counter is 1 once it is set), where `AfterMapGenerated` puts its quest marker
/// ([`RunState::spoils_quest`]).
///
/// # Errors
/// As [`modify_generated_map`].
pub fn modify_generated_map_late(
    s: &mut RunState,
    map: &mut ActMap,
    act_index: usize,
) -> Result<(), MapError> {
    if act_index == SPOILS_ACT
        && let Some(treasure) = map
            .points()
            .iter()
            .find(|p| p.kind == MapPointType::Treasure)
    {
        for card in s.deck.iter_mut().filter(|c| c.id == super::SPOILS_MAP) {
            card.counter = 1;
            s.spoils_quest = Some(treasure.coord);
        }
    }
    for (me, kind) in listeners(s) {
        if s.relics.get(me).is_some_and(|r| r.kind == kind) {
            kind.model()
                .modify_generated_map_late(s, me, map, act_index)?;
        }
    }
    Ok(())
}

/// `Hook.ModifyUnknownMapPointRoomTypes` (`Hook.cs:2069`).
pub fn modify_unknown_map_point_room_types(s: &RunState, types: &mut Vec<RoomType>) {
    if has_lantern_key_in_glory(s) {
        // `LanternKey.ModifyUnknownMapPointRoomTypes`: only events.
        *types = vec![RoomType::Event];
    }
    for (me, kind) in listeners(s) {
        kind.model()
            .modify_unknown_map_point_room_types(s, me, types);
    }
}

/// `Hook.ModifyNextEvent` (`Hook.cs:1838`).
#[must_use]
pub fn modify_next_event(s: &RunState, event: &'static str) -> &'static str {
    // `LanternKey.ModifyNextEvent`: `WarHistorianRepy`.
    let event = if has_lantern_key_in_glory(s) {
        "WAR_HISTORIAN_REPY"
    } else {
        event
    };
    listeners(s).into_iter().fold(event, |e, (me, kind)| {
        kind.model().modify_next_event(s, me, e)
    })
}

/// `Hook.ModifyMerchantPrice` (`Hook.cs:1826`).
#[must_use]
pub fn modify_merchant_price(s: &RunState, price: Decimal) -> Decimal {
    listeners(s).into_iter().fold(price, |p, (me, kind)| {
        kind.model().modify_merchant_price(s, me, p)
    })
}

/// `Hook.ShouldRefillMerchantEntry` (`Hook.cs:2431`): any listener refills.
#[must_use]
pub fn should_refill_merchant_entry(s: &RunState) -> bool {
    listeners(s)
        .into_iter()
        .any(|(me, kind)| kind.model().should_refill_merchant_entry(s, me))
}
