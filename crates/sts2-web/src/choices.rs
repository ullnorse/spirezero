//! What the player chose on each floor (GD26), for the run history: the card taken and the ones
//! passed over, relics, potions, gold, rest-site, event and shop choices. Noted from the state an
//! action is about to leave, so the names are the ones the player was reading. GD31 adds what the
//! action changed (see [`Before`]): potions used and discarded, gold spent or lost, max HP, enchants
//! and transforms.

use serde_json::{Value, json};
use sts2_core::combat::Action;
use sts2_core::content::ancients::AncientOption;
use sts2_core::content::enchantments::Enchantment;
use sts2_core::run::{Reward, Run, RunAction, RunCard, RunDecision, SelectPurpose};
use sts2_view::describe::Describer;
use sts2_view::{menu, names};

/// One choice, as the summary's `choices` carries it (`kind`, `text`, and the `skipped` card names
/// where cards were passed over).
fn choice(kind: &str, text: &str, skipped: &[String]) -> Value {
    json!({ "kind": kind, "text": text, "skipped": skipped })
}

/// The choice `action` makes in `run`'s pending decision, if it is one worth showing.
pub fn of(run: &Run, describer: &Describer, action: &RunAction) -> Option<Value> {
    let label = || {
        menu::items(run, describer, None)
            .into_iter()
            .find(|i| i.action == *action)
            .map(|i| i.label)
    };
    match (action, run.pending_decision()) {
        (RunAction::PickCard { reward, card }, RunDecision::Rewards(rewards)) => {
            let Some(Reward::Card(offer)) = rewards.get(*reward) else {
                return None;
            };
            let picked = names::run_card(*offer.cards.get(*card)?);
            let skipped: Vec<String> = offer
                .cards
                .iter()
                .enumerate()
                .filter(|(i, _)| i != card)
                .map(|(_, c)| names::run_card(*c))
                .collect();
            Some(choice("card", &picked, &skipped))
        }
        (RunAction::SkipReward(i), RunDecision::Rewards(rewards)) => {
            let Some(Reward::Card(offer)) = rewards.get(*i) else {
                return None;
            };
            let all: Vec<String> = offer.cards.iter().map(|c| names::run_card(*c)).collect();
            Some(choice("skip", "", &all))
        }
        (RunAction::ClaimReward(i), RunDecision::Rewards(rewards)) => {
            let (kind, text) = match rewards.get(*i)? {
                Reward::Gold(n) => ("gold", format!("{n} gold")),
                Reward::Potion(id) => ("potion", names::potion(id)),
                Reward::Relic(id) => ("relic", names::relic(id)),
                Reward::WaxRelic(id) => ("relic", format!("{} (wax)", names::relic(id))),
                Reward::SpecialCard(c) => ("card", names::run_card(*c)),
                Reward::Card(_) => return None,
            };
            Some(choice(kind, &text, &[]))
        }
        (RunAction::TakeRelic, RunDecision::Treasure(Some(id))) => {
            Some(choice("relic", &names::relic(id), &[]))
        }
        (RunAction::AncientOption(i), RunDecision::Ancient(room)) => {
            let option = room.options.get(*i)?;
            let name = match option {
                AncientOption::Modifier(m) => names::title_case(m.id()),
                _ => names::relic(option.name()),
            };
            Some(choice("ancient", &name, &[]))
        }
        (RunAction::EventOption(_), _) => Some(choice("event", &label()?, &[])),
        (RunAction::RestOption(_), _) => Some(choice("rest", &label()?, &[])),
        (RunAction::BuyCard(_) | RunAction::BuyRelic(_) | RunAction::BuyPotion(_), _) => {
            Some(choice("buy", &label()?, &[]))
        }
        (RunAction::SelectCard(n), RunDecision::CardSelect(select)) => {
            let card = *select.cards(&run.state().deck).get(*n)?;
            let kind = match select.purpose {
                SelectPurpose::Upgrade | SelectPurpose::Smith => "upgrade",
                SelectPurpose::Remove { .. }
                | SelectPurpose::ShopRemoval { .. }
                | SelectPurpose::Cook => "remove",
                SelectPurpose::AddToDeck => "card",
                SelectPurpose::Clone => "clone",
                _ => "select",
            };
            Some(choice(kind, &names::run_card(card), &[]))
        }
        _ => None,
    }
}

/// What an action can change on a floor, taken before it is applied (GD31): the belt, gold, max HP
/// and deck, to be compared with the state after it.
pub struct Before {
    potions: Vec<Option<&'static str>>,
    gold: i32,
    max_hp: i32,
    deck: Vec<RunCard>,
    /// A shop purchase pays gold; any other loss is a loss.
    pays: bool,
    /// The action is a card selection or an event option, the ones that can transform or enchant.
    changes_deck: bool,
    /// The action claims a gold reward (already noted as `gold`).
    claims: bool,
    used: Option<usize>,
    discarded: Option<usize>,
}

/// A deck compared with a later one: the enchants (the card before, its new enchantment) and
/// the transforms (went, came).
type DeckDiff = (Vec<(RunCard, Enchantment)>, Vec<(RunCard, RunCard)>);

impl Before {
    pub fn of(run: &Run, action: &RunAction) -> Self {
        let state = run.state();
        let shop_removal = matches!(
            (action, run.pending_decision()),
            (RunAction::SelectCard(_), RunDecision::CardSelect(s))
                if matches!(s.purpose, SelectPurpose::ShopRemoval { .. })
        );
        Self {
            potions: run.potions(),
            gold: state.gold,
            max_hp: state.max_hp,
            deck: state.deck.clone(),
            pays: shop_removal
                || matches!(
                    action,
                    RunAction::BuyCard(_) | RunAction::BuyRelic(_) | RunAction::BuyPotion(_)
                ),
            changes_deck: matches!(action, RunAction::SelectCard(_) | RunAction::EventOption(_)),
            claims: matches!(action, RunAction::ClaimReward(_)),
            used: match action {
                RunAction::UsePotion(slot) | RunAction::Combat(Action::UsePotion { slot, .. }) => {
                    Some(*slot)
                }
                _ => None,
            },
            discarded: match action {
                RunAction::DiscardPotion(slot) => Some(*slot),
                _ => None,
            },
        }
    }

    /// The gold held before the action.
    pub const fn gold(&self) -> i32 {
        self.gold
    }

    /// The changes `run` shows now, as choices. Combat actions other than a potion are not looked at.
    pub fn changes(&self, run: &Run, in_combat_action: bool) -> Vec<Value> {
        let mut out = Vec::new();
        let potion = |slot: usize| self.potions.get(slot).copied().flatten().map(names::potion);
        if let Some(name) = self.used.and_then(potion) {
            out.push(choice("potion_used", &name, &[]));
        }
        if let Some(name) = self.discarded.and_then(potion) {
            out.push(choice("potion_discarded", &name, &[]));
        }
        if in_combat_action {
            return out;
        }
        let state = run.state();
        let gold = state.gold - self.gold;
        if gold < 0 {
            let kind = if self.pays { "gold_spent" } else { "gold_lost" };
            out.push(choice(kind, &format!("{} gold", -gold), &[]));
        } else if gold > 0 && !self.claims {
            out.push(choice("gold", &format!("{gold} gold"), &[]));
        }
        let hp = state.max_hp - self.max_hp;
        if hp != 0 {
            out.push(choice("max_hp", &format!("{hp:+}"), &[]));
        }
        if self.changes_deck {
            self.deck_changes(&state.deck, &mut out);
        }
        out
    }

    /// Enchants and transforms: the deck cards that went and came, paired by what the game words.
    fn deck_changes(&self, after: &[RunCard], out: &mut Vec<Value>) {
        let (enchants, transforms) = self.deck_diff(after);
        for (from, e) in enchants {
            let text = format!("{}|{}", names::run_card(from), e.kind.name());
            out.push(choice("enchant", &text, &[]));
        }
        for (from, to) in transforms {
            let text = format!("{}|{}", names::run_card(from), names::run_card(to));
            out.push(choice("transform", &text, &[]));
        }
    }

    /// The deck cards `after` shows transformed, each as the card that went and the one that came
    /// (`CardCmd.Transform`), for the page's reveal.
    pub fn transforms(&self, after: &[RunCard]) -> Vec<(RunCard, RunCard)> {
        self.deck_diff(after).1
    }

    /// The deck's enchants (the card before, its new enchantment) and transforms (went, came).
    fn deck_diff(&self, after: &[RunCard]) -> DeckDiff {
        let mut gone: Vec<RunCard> = self.deck.clone();
        let mut came: Vec<RunCard> = Vec::new();
        for card in after {
            match gone.iter().position(|c| same(*c, *card)) {
                Some(i) => {
                    gone.remove(i);
                }
                None => came.push(*card),
            }
        }
        // An enchanted card is the same card (id, level) with an enchantment it did not have.
        let mut enchants = Vec::new();
        let mut i = 0;
        while i < came.len() {
            let c = came[i];
            let pair = c.enchantment.and_then(|e| {
                gone.iter()
                    .position(|g| {
                        g.id == c.id
                            && g.upgrade_level == c.upgrade_level
                            && g.enchantment.is_none()
                    })
                    .map(|j| (j, e))
            });
            if let Some((j, e)) = pair {
                enchants.push((gone.remove(j), e));
                came.remove(i);
            } else {
                i += 1;
            }
        }
        // The rest that left and came in equal numbers, with a different card id, was transformed.
        gone.retain(|g| !came.iter().any(|c| c.id == g.id));
        came.retain(|c| !self.deck.iter().any(|g| g.id == c.id));
        let transforms = if !gone.is_empty() && gone.len() == came.len() {
            gone.into_iter().zip(came).collect()
        } else {
            Vec::new()
        };
        (enchants, transforms)
    }
}

fn same(a: RunCard, b: RunCard) -> bool {
    a.id == b.id && a.upgrade_level == b.upgrade_level && a.enchantment == b.enchantment
}
