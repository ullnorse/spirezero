//! The numbered choices of each out-of-combat screen, as labels over [`RunAction`]s. Whether a
//! choice can be taken is the engine's call (`Run::legal_actions`); this only words them.

use sts2_core::run::Modifier;
use sts2_core::run::modifiers::sealed_deck;
use sts2_core::run::{
    CardAlternative, CardSelect, CardSlot, MapCoord, RestOptionKind, Reward, Run, RunAction,
    RunCard, RunDecision, RunState, SelectPurpose, Shop, shop,
};

use sts2_core::content::ancients::AncientOption;
use sts2_core::content::events::EventData;
use sts2_core::decimal::Decimal;

use crate::describe::{Describer, EventText};
use crate::names;

/// One line of a screen's menu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuItem {
    pub label: String,
    /// What the choice is, in the game's text (markup), when the text is available.
    pub detail: Option<String>,
    pub action: RunAction,
    /// Whether the engine lists the action as legal; a disabled item is shown but has no number.
    pub enabled: bool,
    /// What is lost for good if the choice is taken, when that is worth a second key press.
    pub warning: Option<String>,
    /// The card reward (its index among the rewards) this row opens as a screen of its own
    /// instead of taking `action`.
    pub open: Option<usize>,
    /// The shop section this row opens (with the items left in it) at the shop's first level;
    /// its `action` is a placeholder that is never applied.
    pub section: Option<(ShopSection, usize)>,
    /// How many identical cards a deck selection's row stands for (`Strike ×5`); its action
    /// takes the first of them. 1 for every other row.
    pub copies: usize,
}

/// A part of the shop: the second level of its screen lists one section's items.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShopSection {
    Cards,
    Colorless,
    Relics,
    Potions,
}

impl ShopSection {
    /// The sections in the order the shop lists them.
    pub const ALL: [Self; 4] = [Self::Cards, Self::Colorless, Self::Relics, Self::Potions];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Cards => "Cards",
            Self::Colorless => "Colorless cards",
            Self::Relics => "Relics",
            Self::Potions => "Potions",
        }
    }

    /// The count with its unit, `5 cards`, `1 relic`.
    #[must_use]
    pub fn count_label(self, n: usize) -> String {
        let (one, many) = match self {
            Self::Cards | Self::Colorless => ("card", "cards"),
            Self::Relics => ("relic", "relics"),
            Self::Potions => ("potion", "potions"),
        };
        format!("{n} {}", if n == 1 { one } else { many })
    }
}

/// The section a buy action belongs to, if the item is still on sale.
fn section_of(shop: &Shop, action: &RunAction) -> Option<ShopSection> {
    match *action {
        RunAction::BuyCard(i) => {
            let card = shop.cards.get(i).filter(|c| c.card.is_some())?;
            Some(match card.slot {
                CardSlot::Character(_) => ShopSection::Cards,
                CardSlot::Colorless(_) => ShopSection::Colorless,
            })
        }
        RunAction::BuyRelic(i) => shop
            .relics
            .get(i)
            .filter(|r| r.id.is_some())
            .map(|_| ShopSection::Relics),
        RunAction::BuyPotion(i) => shop
            .potions
            .get(i)
            .filter(|p| p.id.is_some())
            .map(|_| ShopSection::Potions),
        _ => None,
    }
}

/// The section of the shop a buy action is in, if the item is still on sale.
#[must_use]
pub fn buy_section(run: &Run, action: &RunAction) -> Option<ShopSection> {
    match run.pending_decision() {
        RunDecision::Shop(shop) => section_of(shop, action),
        _ => None,
    }
}

/// How many items `section` still has on sale.
#[must_use]
pub fn section_count(run: &Run, section: ShopSection) -> usize {
    let RunDecision::Shop(shop) = run.pending_decision() else {
        return 0;
    };
    let count = |n: usize, make: fn(usize) -> RunAction| {
        (0..n)
            .filter(|&i| section_of(shop, &make(i)) == Some(section))
            .count()
    };
    count(shop.cards.len(), RunAction::BuyCard)
        + count(shop.relics.len(), RunAction::BuyRelic)
        + count(shop.potions.len(), RunAction::BuyPotion)
}

/// The shop's items at one of its two levels: with no `section` its sections (with their
/// counts) then Remove a card and Leave; with one, only that section's items. Other decisions'
/// items pass through.
#[must_use]
pub fn shop_level(run: &Run, items: Vec<MenuItem>, section: Option<ShopSection>) -> Vec<MenuItem> {
    let RunDecision::Shop(shop) = run.pending_decision() else {
        return items;
    };
    if let Some(open) = section {
        return items
            .into_iter()
            .filter(|i| section_of(shop, &i.action) == Some(open))
            .collect();
    }
    let rows = ShopSection::all_rows(run);
    rows.into_iter()
        .chain(items.into_iter().filter(|i| {
            !matches!(
                i.action,
                RunAction::BuyCard(_) | RunAction::BuyRelic(_) | RunAction::BuyPotion(_)
            )
        }))
        .collect()
}

/// The whole shop on one level: every item still on sale, then Remove a card and Leave (sold
/// items go). Other decisions' items pass through.
#[must_use]
pub fn shop_flat(run: &Run, items: Vec<MenuItem>) -> Vec<MenuItem> {
    let RunDecision::Shop(shop) = run.pending_decision() else {
        return items;
    };
    items
        .into_iter()
        .filter(|i| {
            section_of(shop, &i.action).is_some()
                || !matches!(
                    i.action,
                    RunAction::BuyCard(_) | RunAction::BuyRelic(_) | RunAction::BuyPotion(_)
                )
        })
        .collect()
}

/// The shop's items as a client lays them out: on one level when `flat`, else at the level
/// `section` names ([`shop_level`]).
#[must_use]
pub fn shop_items_for(
    run: &Run,
    items: Vec<MenuItem>,
    section: Option<ShopSection>,
    flat: bool,
) -> Vec<MenuItem> {
    if flat {
        shop_flat(run, items)
    } else {
        shop_level(run, items, section)
    }
}

impl ShopSection {
    /// The first level's section rows.
    fn all_rows(run: &Run) -> Vec<MenuItem> {
        Self::ALL
            .into_iter()
            .map(|s| {
                let n = section_count(run, s);
                MenuItem {
                    label: s.label().to_owned(),
                    detail: None,
                    action: RunAction::CancelSelect,
                    enabled: n > 0,
                    warning: None,
                    open: None,
                    section: Some((s, n)),
                    copies: 1,
                }
            })
            .collect()
    }
}

/// The items so far, and which actions are legal.
struct Builder<'a> {
    legal: &'a [RunAction],
    text: &'a Describer,
    items: Vec<MenuItem>,
}

impl Builder<'_> {
    fn add(&mut self, label: String, action: RunAction) {
        self.add_detail(label, None, action);
    }

    fn add_detail(&mut self, label: String, detail: Option<String>, action: RunAction) {
        let enabled = self.legal.contains(&action);
        self.items.push(MenuItem {
            label,
            detail,
            action,
            enabled,
            warning: None,
            open: None,
            section: None,
            copies: 1,
        });
    }

    /// Makes the last item ask before it is taken, with `warning` as its question.
    fn warn(&mut self, warning: String) {
        if let Some(last) = self.items.last_mut() {
            last.warning = Some(warning);
        }
    }
}

/// A rest-site option in plain words when the game's text is not installed, with the
/// engine's numbers (`RunState::rest_option_vars`): `Rest (heal 24 HP)`.
fn rest_label(kind: RestOptionKind, vars: &[(&'static str, Decimal)]) -> String {
    let var = |name: &str| {
        vars.iter().find(|&&(n, _)| n == name).map_or_else(
            || "?".to_owned(),
            |&(_, v)| v.to_int_truncating().to_string(),
        )
    };
    match kind {
        RestOptionKind::Heal => format!("Rest (heal {} HP)", var("Heal")),
        RestOptionKind::Smith => format!("Smith (upgrade {} card)", var("Count")),
        RestOptionKind::Lift => format!("Lift (Girya: gain Strength, {} left)", var("LiftsLeft")),
        RestOptionKind::Dig => "Dig (Shovel: find a relic)".to_owned(),
        RestOptionKind::Hatch => "Hatch (Byrdonis Egg: get Byrdpip)".to_owned(),
        RestOptionKind::Kindle => format!(
            "Kindle (Pumpkin Candle: +{} combats)",
            var("RekindleAmount")
        ),
        RestOptionKind::Clone => "Clone (copy every Clone card)".to_owned(),
        RestOptionKind::Cook => format!(
            "Cook (Meat Cleaver: remove {} cards, +{} max HP)",
            var("Cards"),
            var("MaxHp")
        ),
    }
}

/// An event's options in the game's text. `TheFutureOfPotions` fills each option's potion,
/// rarity and card type from its belt slot.
fn event_items(menu: &mut Builder<'_>, run: &Run, room: &sts2_core::run::EventRoom) {
    let state = run.state();
    let text = menu.text;
    let vars = room.vars(state);
    let event = EventText {
        vars: &vars,
        character: Some(state.character),
        extra: &[],
    };
    for (i, option) in room.options.iter().enumerate() {
        let extra = match (&room.data, option.action) {
            (EventData::FutureOfPotions { types }, Some(action)) => state
                .potions_held()
                .find(|&(slot, _)| slot == action.arg)
                .zip(types.iter().find(|&&(slot, _)| slot == action.arg))
                .map(|((_, id), &(_, kind))| text.future_of_potions_vars(id, kind))
                .unwrap_or_default(),
            _ => Vec::new(),
        };
        let event = EventText {
            extra: &extra,
            ..event
        };
        let (label, detail) = text
            .event_option(&option.text_key, &event)
            .unwrap_or_else(|| (names::event_option(&option.text_key), None));
        menu.add_detail(label, detail, RunAction::EventOption(i));
    }
}

/// The menu of the run's pending decision (empty in combat and on the end screens).
///
/// Potions are not part of it: they have their own key. `open` is the card reward open as its
/// own screen, if one is: the menu is then its cards.
#[must_use]
pub fn items(run: &Run, text: &Describer, open: Option<usize>) -> Vec<MenuItem> {
    let legal = run.legal_actions();
    let state = run.state();
    let mut menu = Builder {
        legal: &legal,
        text,
        items: Vec::new(),
    };
    match run.pending_decision() {
        RunDecision::Ancient(room) => {
            for (i, option) in room.options.iter().enumerate() {
                if let AncientOption::Modifier(modifier) = *option {
                    menu.add_detail(
                        text.modifier_title(modifier),
                        text.modifier_description(modifier),
                        RunAction::AncientOption(i),
                    );
                    continue;
                }
                let id = option.name();
                menu.add_detail(
                    names::relic(id),
                    text.relic(id),
                    RunAction::AncientOption(i),
                );
            }
        }
        RunDecision::Event(room) => event_items(&mut menu, run, room),
        RunDecision::Map(nodes) => map_items(&mut menu, state, &nodes),
        RunDecision::Rewards(rewards) => reward_items(&mut menu, run, rewards, open),
        RunDecision::CardSelect(select) => card_select_items(&mut menu, state, select),
        RunDecision::Bundles(bundles) => {
            for (i, bundle) in bundles.iter().enumerate() {
                let cards: Vec<String> = bundle.iter().map(|c| names::run_card(*c)).collect();
                let detail: Vec<String> = bundle
                    .iter()
                    .filter_map(|c| {
                        let text = text.run_card(*c)?;
                        Some(format!("[gold]{}[/gold]: {text}", names::run_card(*c)))
                    })
                    .collect();
                let detail = (!detail.is_empty()).then(|| detail.join("\n"));
                menu.add_detail(cards.join(", "), detail, RunAction::SelectBundle(i));
            }
        }
        RunDecision::RestSite(options) => {
            for (i, option) in options.iter().enumerate() {
                let vars = state.rest_option_vars(option.kind);
                let (label, detail) = text
                    .rest_option(option.kind, &vars, option.enabled)
                    .map_or_else(
                        || (rest_label(option.kind, &vars), None),
                        |(name, detail)| (name, Some(detail)),
                    );
                menu.add_detail(label, detail, RunAction::RestOption(i));
            }
            // The game offers Proceed only once you have rested.
            if legal.contains(&RunAction::Proceed) || !text.game_wording() {
                menu.add("Leave".to_owned(), RunAction::Proceed);
            }
        }
        RunDecision::Treasure(relic) => {
            if let Some(id) = relic {
                menu.add_detail(
                    format!("Take the relic: {}", names::relic(id)),
                    text.relic(id),
                    RunAction::TakeRelic,
                );
            }
            menu.add("Leave".to_owned(), RunAction::Proceed);
        }
        RunDecision::Shop(inventory) => shop_items(&mut menu, state, inventory),
        RunDecision::Unsupported(_) => menu.add("Leave this room".to_owned(), RunAction::Proceed),
        RunDecision::Combat(_)
        | RunDecision::Stopped(_)
        | RunDecision::GameOver
        | RunDecision::Victory => {}
    }
    menu.items
}

/// The card a menu entry takes or buys (a card reward's card, a shop card), if it is one.
#[must_use]
pub fn item_card(run: &Run, action: &RunAction) -> Option<RunCard> {
    match (action, run.pending_decision()) {
        (RunAction::PickCard { reward, card }, RunDecision::Rewards(rewards)) => {
            match rewards.get(*reward)? {
                Reward::Card(cards) => cards.cards.get(*card).copied(),
                _ => None,
            }
        }
        (RunAction::ClaimReward(i), RunDecision::Rewards(rewards)) => match rewards.get(*i)? {
            Reward::SpecialCard(c) => Some(*c),
            _ => None,
        },
        (RunAction::BuyCard(i), RunDecision::Shop(shop)) => shop.cards.get(*i)?.card,
        (
            RunAction::SelectCard(n) | RunAction::DeselectCard(n),
            RunDecision::CardSelect(select),
        ) => select.cards(&run.state().deck).get(*n).copied(),
        _ => None,
    }
}

fn map_items(menu: &mut Builder<'_>, state: &RunState, nodes: &[MapCoord]) {
    for &coord in nodes {
        let kind = state
            .map
            .point(coord)
            .map_or_else(|| "?".to_owned(), |p| names::point_type(state, p.kind));
        menu.add(
            format!("{kind} (col {}, row {})", coord.col, coord.row),
            RunAction::ChooseNode(coord),
        );
    }
}

/// A reward in a few words.
#[must_use]
pub fn reward_name(reward: &Reward) -> String {
    match reward {
        Reward::Gold(n) => format!("{n} gold"),
        Reward::Potion(id) => names::potion(id),
        Reward::Relic(id) => names::relic(id),
        Reward::WaxRelic(id) => format!("{} (wax)", names::relic(id)),
        Reward::SpecialCard(c) => names::run_card(*c),
        Reward::Card(_) => "a card".to_owned(),
    }
}

/// The cards of card reward `i` (its screen): each card, the alternatives the engine lists
/// (a reroll, a relic's trade), then Skip.
fn card_reward_items(menu: &mut Builder<'_>, run: &Run, i: usize, cards: &[RunCard]) {
    for (card, c) in cards.iter().enumerate() {
        menu.add_detail(
            format!("Card: {}", names::run_card_detail(*c)),
            menu.text.run_card(*c),
            RunAction::PickCard { reward: i, card },
        );
    }
    for (alternative, kind) in run.card_reward_alternatives(i).into_iter().enumerate() {
        let label = match kind {
            // The Skip row below.
            CardAlternative::Skip => continue,
            CardAlternative::Reroll => "Reroll the cards".to_owned(),
            CardAlternative::Relic { relic, option } => format!(
                "{} ({})",
                names::event_option(option),
                names::relic(state_relic_id(run, relic))
            ),
        };
        menu.add(
            label,
            RunAction::CardAlternative {
                reward: i,
                alternative,
            },
        );
    }
    menu.add("Skip".to_owned(), RunAction::SkipReward(i));
    menu.warn("Skip the card reward".to_owned());
}

/// One row per reward left, no skip rows (Leave skips what is left); a card reward is one row
/// that opens its cards as a screen of their own (`open`).
fn reward_items(menu: &mut Builder<'_>, run: &Run, rewards: &[Reward], open: Option<usize>) {
    if let Some(i) = open
        && let Some(Reward::Card(cards)) = rewards.get(i)
    {
        card_reward_items(menu, run, i, &cards.cards);
        return;
    }
    for (i, reward) in rewards.iter().enumerate() {
        match reward {
            Reward::Gold(n) => menu.add(format!("Gold: {n}"), RunAction::ClaimReward(i)),
            Reward::Potion(id) => menu.add_detail(
                format!("Potion: {}", names::potion(id)),
                menu.text.potion(id),
                RunAction::ClaimReward(i),
            ),
            Reward::WaxRelic(id) => menu.add_detail(
                format!("Wax relic: {}", names::relic(id)),
                menu.text.relic(id),
                RunAction::ClaimReward(i),
            ),
            Reward::Relic(id) => menu.add_detail(
                format!("Relic: {}", names::relic(id)),
                menu.text.relic(id),
                RunAction::ClaimReward(i),
            ),
            Reward::SpecialCard(c) => menu.add_detail(
                format!("Card: {}", names::run_card_detail(*c)),
                menu.text.run_card(*c),
                RunAction::ClaimReward(i),
            ),
            Reward::Card(cards) => {
                // Skipping is legal exactly while the reward can be answered.
                menu.add(
                    format!("Card reward ({} cards)", cards.cards.len()),
                    RunAction::SkipReward(i),
                );
                // The Draft's rewards cannot be skipped but can be opened: the row is live while
                // any of its cards can be picked.
                let pickable = menu
                    .legal
                    .iter()
                    .any(|a| matches!(a, RunAction::PickCard { reward, .. } if *reward == i));
                if let Some(row) = menu.items.last_mut() {
                    row.open = Some(i);
                    row.enabled |= pickable;
                }
            }
        }
    }
    let left: Vec<String> = rewards.iter().map(reward_name).collect();
    menu.add("Leave".to_owned(), RunAction::Proceed);
    if !left.is_empty() {
        menu.warn(format!("Leave without taking {}", left.join(", ")));
    }
}

/// The id of the relic at `index`.
fn state_relic_id(run: &Run, index: usize) -> &'static str {
    run.state().relics[index].kind.id()
}

/// A deck selection's cards: identical cards not yet picked share one row (`Strike ×5`) that
/// takes the first of them, so five Strikes do not fill a page. Identical is every field of the
/// card (upgrade, enchantment, counter), so whichever copy is taken, the run is the same as if
/// the player had pointed at another.
fn card_select_items(menu: &mut Builder<'_>, state: &RunState, select: &CardSelect) {
    let cards = select.cards(&state.deck);
    let mut counted = vec![false; cards.len()];
    // The `SealedDeck` grid is sorted by rarity, then title (`SealedDeck.CompareCards`).
    let order = if select.purpose == SelectPurpose::AddToDeck
        && select.count == sealed_deck::KEPT
        && state.modifiers.contains(&Modifier::SealedDeck)
    {
        sealed_deck::display_order(&cards, names::run_card)
    } else {
        (0..cards.len()).collect()
    };
    for &n in &order {
        let card = cards[n];
        let label = names::run_card_detail(card);
        let detail = menu.text.run_card(card);
        if select.picked.contains(&n) {
            menu.items.push(MenuItem {
                label: format!("{label}  [picked]"),
                detail,
                action: RunAction::DeselectCard(n),
                enabled: true,
                warning: None,
                open: None,
                section: None,
                copies: 1,
            });
            continue;
        }
        if counted[n] {
            continue;
        }
        let mut copies = 1;
        for m in n + 1..cards.len() {
            if !counted[m] && !select.picked.contains(&m) && cards[m] == card {
                counted[m] = true;
                copies += 1;
            }
        }
        menu.add_detail(label, detail, RunAction::SelectCard(n));
        if let Some(row) = menu.items.last_mut() {
            row.copies = copies;
        }
    }
    // A free card has its one Skip row below, not a second way to pick none.
    let free_card = menu.text.game_wording()
        && matches!(select.purpose, SelectPurpose::ChooseACard { .. })
        && select.purpose.is_cancelable()
        && select.picked.is_empty();
    if !free_card && (select.min() < select.count || select.manual_confirm()) {
        let label = if select.picked.is_empty() {
            "Done (pick none)"
        } else {
            "Done"
        };
        menu.add(label.to_owned(), RunAction::ConfirmSelect);
    }
    // Cancelling is Esc (`App::back`); only a free card keeps a Skip row, as a reward does.
    if matches!(select.purpose, SelectPurpose::ChooseACard { .. }) && select.purpose.is_cancelable()
    {
        menu.add("Skip".to_owned(), RunAction::CancelSelect);
    }
}

fn shop_items(menu: &mut Builder<'_>, state: &RunState, inventory: &Shop) {
    let price = |base| shop::price(state, base);
    let entry = |kind: &str, name: String, base| format!("{kind}: {name} - {}g", price(base));
    for (i, card) in inventory.cards.iter().enumerate() {
        let detail = card.card.and_then(|c| menu.text.run_card(c));
        let label = card.card.map_or_else(
            || "(sold)".to_owned(),
            |c| {
                let kind = match card.slot {
                    CardSlot::Character(_) => "Card",
                    CardSlot::Colorless(_) => "Colorless",
                };
                let sale = if card.on_sale { "  SALE" } else { "" };
                format!(
                    "{}{sale}",
                    entry(kind, names::run_card_detail(c), card.base_cost)
                )
            },
        );
        menu.add_detail(label, detail, RunAction::BuyCard(i));
        menu.warn(format!("Buy for {}", price(card.base_cost)));
    }
    for (i, relic) in inventory.relics.iter().enumerate() {
        let label = relic.id.map_or_else(
            || "(sold)".to_owned(),
            |id| entry("Relic", names::relic(id), relic.base_cost),
        );
        let detail = relic.id.and_then(|id| menu.text.relic(id));
        menu.add_detail(label, detail, RunAction::BuyRelic(i));
        menu.warn(format!("Buy for {}", price(relic.base_cost)));
    }
    for (i, potion) in inventory.potions.iter().enumerate() {
        let label = potion.id.map_or_else(
            || "(sold)".to_owned(),
            |id| entry("Potion", names::potion(id), potion.base_cost),
        );
        let detail = potion.id.and_then(|id| menu.text.potion(id));
        menu.add_detail(label, detail, RunAction::BuyPotion(i));
        menu.warn(format!("Buy for {}", price(potion.base_cost)));
    }
    let removal = if inventory.removal_used {
        "Remove a card (used)".to_owned()
    } else {
        format!("Remove a card - {}g", price(inventory.removal_base_cost))
    };
    menu.add(removal, RunAction::RemoveCard);
    menu.warn(format!(
        "Remove a card for {}",
        price(inventory.removal_base_cost)
    ));
    menu.add("Leave the shop".to_owned(), RunAction::Proceed);
}
