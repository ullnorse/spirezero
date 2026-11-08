//! The fight as a [`Screen`]: a pure read of a [`Combat`] and the client's selection.
//!
//! The builder never draws and never decides: legal cards, numbers, intents and previews come
//! from the engine (`Combat::unplayable_reason`, `Combat::incoming`, `CombatState::intents`); the
//! text comes from [`Describer`] and [`names`]. A renderer (the TUI now, `SwiftUI` later) only
//! places the blocks.

use sts2_core::combat::{
    CardId, Combat, CombatState, CreatureId, CreatureOutcome, Decision, DecisionRequest,
    ExtraReward, IncomingPreview, Intent, IntentType, PlayOutcome, Prompt, Side, Status,
};
use sts2_core::content::cards::{CardData, CardKind, CardRarity, CardType};
use sts2_core::content::monsters::MonsterKind;
use sts2_core::content::powers::{PowerData, PowerKind, PowerType};
use sts2_core::{Action, Character, Decimal};

use crate::describe::Describer;
use crate::model::{
    BladeRow, Block, Button, ButtonState, CardFace, ChoiceSheet, EnemyRow, Hit, IncomingRow, Key,
    OrbRow, PetRow, PowerRow, Preview, Screen, Sheet, YouRow,
};
use crate::names;
use crate::text::{self, Rich, RichSpan, Role};
use crate::ui::{Item, Pending, ids};

/// What the client has selected in the fight, if anything.
#[derive(Clone, Debug, Default)]
pub struct Selection {
    /// The card whose effect the enemies preview: the card waiting for a target or dragged,
    /// else the card the focus rests on.
    pub selected: Option<CardId>,
    /// The selected card is waiting (all it does is previewed), not just in focus (only what
    /// is certain is).
    pub waiting: bool,
    /// What waits for a target.
    pub pending: Option<Pending>,
    /// A card without a target is being dragged: it waits for a drop on the battlefield.
    pub armed: Option<CardId>,
    /// The enemy the selected card is aimed at (its text reads against it).
    pub target: Option<CreatureId>,
    /// What the keyboard focus rests on.
    pub focus: Option<Item>,
    /// What the incoming line showed before the turn's last play: shown, stale, for a card
    /// choice (the engine has no preview while one is open).
    pub kept_incoming: Option<IncomingPreview>,
}

/// The belt keys the TUI prints (`z c b n g`); other clients may print them too.
pub const POTION_KEYS: [char; 5] = ['z', 'c', 'b', 'n', 'g'];

/// The fight as a screen.
#[must_use]
pub fn screen(combat: &Combat, describer: &Describer, sel: &Selection) -> Screen {
    let state = combat.state();
    let legal = combat.legal_actions();
    let previews = previews(combat, &legal, sel);
    let decision = combat.status() == Status::Decision;
    let mut blocks = combatants(combat, describer, &legal, &previews, sel);
    if let Some(incoming) = incoming_row(combat, sel) {
        blocks.insert(enemy_end(&blocks), Block::Incoming(incoming));
    }
    if let Some(line) = crate::last::line(state, combat.events()) {
        blocks.push(Block::Last(spans([(Role::Muted, line)])));
    }
    let sheet = if decision {
        choice_sheet(combat, describer, sel)
    } else {
        let hand = hand_buttons(combat, describer, &legal, sel);
        if !hand.is_empty() {
            blocks.push(Block::List {
                frame: true,
                rows: hand,
                page: None,
            });
        }
        None
    };
    let counts = format!(
        "Draw {} · Discard {} · Exhaust {}",
        state.player.draw.len(),
        state.player.discard.len(),
        state.player.exhaust.len()
    );
    blocks.push(Block::Text(vec![spans([(Role::Muted, counts)])]));
    Screen {
        top: None,
        title: targeting_title(state, sel),
        aside: None,
        blocks,
        primary: main_button(combat, sel),
        sheet,
        status: None,
    }
}

/// Where the player's side starts in `blocks`: after the enemies.
fn enemy_end(blocks: &[Block]) -> usize {
    blocks
        .iter()
        .position(|b| matches!(b, Block::You(_)))
        .map_or(blocks.len(), |at| at + 1)
}

/// The enemies in the engine's order: the encounter's slot order
/// (`CombatState.SortEnemiesBySlotName`), so a summon stands where the game places it (Fogmog's
/// Eye with Teeth, slot "illusion", left of Fogmog). An escaped enemy keeps its place, shown as
/// gone: it goes back in by the same key (slot index, then creature id, which follows spawn
/// order).
fn enemy_order(state: &CombatState) -> Vec<CreatureId> {
    let slots = state.encounter.slots();
    let key = |c: CreatureId| {
        let slot = state
            .creature(c)
            .slot
            .and_then(|name| slots.iter().position(|&s| s == name));
        (slot, c.0)
    };
    let mut order: Vec<CreatureId> = state.enemies.clone();
    for &gone in &state.escaped {
        let at = order
            .iter()
            .position(|&c| key(c) > key(gone))
            .unwrap_or(order.len());
        order.insert(at, gone);
    }
    order
}

/// The row of one enemy; `index` is its place on the screen and `escaped_thieves` counts the
/// thieves gone before it, which hold the lost cards in turn.
fn enemy_row(
    combat: &Combat,
    describer: &Describer,
    (legal, previews, sel): (&[Action], &[(CreatureId, Preview)], &Selection),
    (index, enemy): (usize, CreatureId),
    (lost, escaped_thieves): (&[CardId], &mut usize),
) -> EnemyRow {
    let state = combat.state();
    let creature = state.creature(enemy);
    let escaped = state.escaped.contains(&enemy);
    let alive = creature.is_alive() && !escaped;
    let stolen = if escaped {
        let is_thief = creature
            .monster
            .as_ref()
            .is_some_and(|m| matches!(m.kind, MonsterKind::ThievingHopper(_)));
        if is_thief {
            *escaped_thieves += 1;
            lost.get(*escaped_thieves - 1).copied()
        } else {
            None
        }
    } else {
        creature
            .powers
            .iter()
            .find_map(|p| match state.power_data(p.id) {
                PowerData::Swipe(d) => d.stolen,
                _ => None,
            })
    }
    .map(|card| card_face(combat, describer, card));
    EnemyRow {
        id: enemy.0,
        art: creature
            .monster
            .as_ref()
            .and_then(|m| MonsterKind::IDS.get(m.kind.ordinal()))
            .map_or_else(String::new, |id| (*id).to_owned()),
        name: names::creature(state, enemy),
        hp: creature.hp,
        max_hp: creature.max_hp,
        block: creature.block,
        alive,
        dead_state: !alive
            && creature
                .monster
                .as_ref()
                .is_some_and(|m| m.next_move_name() == "RESPAWN_MOVE"),
        intent: alive.then(|| intent_rich(state, enemy, false)).flatten(),
        intent_long: alive.then(|| intent_rich(state, enemy, true)).flatten(),
        powers: powers(state, describer, enemy, false),
        escaped,
        stolen,
        preview: previews.iter().find(|(c, _)| *c == enemy).map(|&(_, p)| p),
        target: can_target(legal, sel, enemy).then(|| {
            let mut button = Button::row(ids::creature(enemy).0, Rich::default());
            button.key = Some(Key::Digit(u8::try_from((index + 1) % 10).unwrap_or(0)));
            if sel.focus == Some(Item::Creature(enemy)) {
                button.state = ButtonState::Selected;
            }
            button
        }),
    }
}

/// The player's row.
fn you_row(combat: &Combat, describer: &Describer) -> YouRow {
    let state = combat.state();
    let player = state.creature(state.player.creature);
    YouRow {
        id: state.player.creature.0,
        stars: (state.player.character == Character::Regent || state.player.stars != 0)
            .then_some(state.player.stars),
        draw: state.player.draw.len(),
        discard: state.player.discard.len(),
        exhaust: state.player.exhaust.len(),
        name: state.player.character.name().to_owned(),
        hp: player.hp,
        max_hp: player.max_hp,
        block: player.block,
        energy: state.player.energy,
        max_energy: state.player.max_energy,
        powers: powers(state, describer, state.player.creature, true),
        resources: resources(state),
        pet_slot: state.player.character == Character::Necrobinder || state.osty().is_some(),
        orb_slots: usize::from(state.player.orb_slots),
        blades: blades(state),
    }
}

fn combatants(
    combat: &Combat,
    describer: &Describer,
    legal: &[Action],
    previews: &[(CreatureId, Preview)],
    sel: &Selection,
) -> Vec<Block> {
    let state = combat.state();
    let mut blocks = Vec::new();
    let order = enemy_order(state);
    let lost = lost_cards(state);
    let mut escaped_thieves = 0;
    for (i, &enemy) in order.iter().enumerate() {
        let row = enemy_row(
            combat,
            describer,
            (legal, previews, sel),
            (i, enemy),
            (&lost, &mut escaped_thieves),
        );
        blocks.push(Block::Enemy(Box::new(row)));
    }
    blocks.push(Block::You(you_row(combat, describer)));
    for &pet in &state.player.pets {
        let creature = state.creature(pet);
        blocks.push(Block::Pet(Box::new(PetRow {
            id: pet.0,
            name: names::creature(state, pet),
            hp: creature.hp,
            max_hp: creature.max_hp,
            block: creature.block,
            alive: creature.is_alive(),
            powers: powers(state, describer, pet, true),
            target: can_target(legal, sel, pet).then(|| {
                let mut button = Button::row(ids::creature(pet).0, Rich::default());
                if sel.focus == Some(Item::Creature(pet)) {
                    button.state = ButtonState::Selected;
                }
                button
            }),
        })));
    }
    for orb in &state.player.orbs {
        blocks.push(Block::Orbs(OrbRow {
            name: names::title_case(orb.kind.id().trim_end_matches("_ORB")),
            passive: state.orb_passive_val(*orb).to_int_truncating(),
            evoke: state.orb_evoke_val(*orb).to_int_truncating(),
        }));
    }
    blocks
}

/// Whether `creature` can be picked now as the target of what waits.
fn can_target(legal: &[Action], sel: &Selection, creature: CreatureId) -> bool {
    match sel.pending {
        Some(Pending::Card(card)) => legal.iter().any(|a| {
            matches!(a, Action::PlayCard { card: c, target: Some(t) } if *c == card && *t == creature)
        }),
        Some(Pending::Potion(slot)) => legal.iter().any(|a| {
            matches!(a, Action::UsePotion { slot: s, target: Some(t) } if *s == slot && *t == creature)
        }),
        None => false,
    }
}

/// What the selected card does to each living enemy: played at that enemy if it takes a target,
/// else played once (all enemies at once, or none).
fn previews(combat: &Combat, legal: &[Action], sel: &Selection) -> Vec<(CreatureId, Preview)> {
    let Some(card) = sel.selected else {
        return Vec::new();
    };
    let state = combat.state();
    let targets: Vec<Option<CreatureId>> = legal
        .iter()
        .filter_map(|a| match a {
            Action::PlayCard { card: c, target } if *c == card => Some(*target),
            _ => None,
        })
        .collect();
    let untargeted = targets
        .contains(&None)
        .then(|| combat.play_outcome_until_draw(card, None));
    let mut out: Vec<(CreatureId, Preview)> = state
        .enemies
        .iter()
        .filter(|&&e| state.creature(e).is_alive())
        .filter_map(|&e| {
            let outcome = match &untargeted {
                Some(once) => once.as_ref(),
                None if targets.contains(&Some(e)) => {
                    let outcome = combat.play_outcome_until_draw(card, Some(e));
                    return Some((e, preview_of(outcome.as_ref(), e)));
                }
                None => return None,
            };
            Some((e, preview_of(outcome, e)))
        })
        .collect();
    // The focused card only previews what it is sure of; the waiting card says everything.
    if !sel.waiting {
        out.retain(|(_, p)| matches!(p, Preview::Certain(_) | Preview::BeforeDraw(_)));
    }
    out
}

/// `outcome`'s part for `id`: unchanged when the whole play does not touch it, unknown when the
/// part before its draw does not.
fn preview_of(outcome: Option<&PlayOutcome>, id: CreatureId) -> Preview {
    outcome.map_or(Preview::Unknown, |o| match (o.of(id), o.partial) {
        (Some(c), false) => Preview::Certain(hit_of(c)),
        (Some(c), true) => Preview::BeforeDraw(hit_of(c)),
        (None, false) => Preview::Unchanged,
        (None, true) => Preview::Unknown,
    })
}

const fn hit_of(c: &CreatureOutcome) -> Hit {
    Hit {
        hp_before: c.hp_before,
        hp_after: c.hp_after,
        block_before: c.block_before,
        block_after: c.block_after,
        dies: c.dies,
    }
}

fn incoming_row(combat: &Combat, sel: &Selection) -> Option<IncomingRow> {
    let (incoming, stale) = match combat.incoming() {
        Some(incoming) => (incoming, false),
        None if combat.status() == Status::Decision => (sel.kept_incoming.clone()?, true),
        None => return None,
    };
    Some(IncomingRow {
        total: incoming.total,
        blocked: incoming.blocked,
        absorbed: incoming.absorbed_by.iter().map(|&(_, n)| n).sum(),
        hp_loss: incoming.hp_loss,
        hp_after: incoming.hp_after,
        lethal: incoming.lethal,
        certain: incoming.certain,
        stale,
    })
}

fn hand_buttons(
    combat: &Combat,
    describer: &Describer,
    legal: &[Action],
    sel: &Selection,
) -> Vec<Button> {
    let state = combat.state();
    let mut hand = Vec::new();
    for (i, &card) in state.player.hand.iter().enumerate() {
        let mut button = Button::row(ids::row(i).0, card_label(combat, describer, card, sel));
        button.key = Some(Key::Digit(u8::try_from(i + 1).unwrap_or(0) % 10));
        button.right = Some(spans([(
            Role::Energy,
            format!("ϟ{}", combat.energy_cost(card)),
        )]));
        button.detail = card_detail(describer, state, card, sel.target);
        let mut face = card_face(combat, describer, card);
        // A card choice in progress replaces the glow (`SelectModeGoldGlowOverride`).
        face.glow = combat.pending_decision().is_none() && state.glows_gold(card);
        button.card = Some(face);
        let playable = legal
            .iter()
            .any(|a| matches!(a, Action::PlayCard { card: c, .. } if *c == card));
        button.state = if sel.selected == Some(card) {
            ButtonState::Selected
        } else if playable {
            ButtonState::Enabled
        } else {
            let reason = state.unplayable_reason(card).map_or_else(
                || "it has no target now".to_owned(),
                |reason| text::plain(&describer.unplayable(state, reason)),
            );
            ButtonState::Dim(reason)
        };
        hand.push(button);
    }
    hand
}

/// The answers to the fight's card choice, in display order: a pick or an unpick for each card in
/// the request's own order (a picked card keeps its place, as in the game's hand selection), then
/// the rest of the engine's legal ones.
pub(crate) fn choice_actions(combat: &Combat) -> Vec<Action> {
    let mut actions = combat.legal_actions();
    let Some(request) = combat.pending_decision() else {
        return actions;
    };
    actions.extend(request.unpicks().into_iter().map(Action::Resolve));
    let DecisionRequest::ChooseCards { options, .. } = request;
    let place = |a: &Action| match a {
        Action::Resolve(Decision::Pick(c) | Decision::Unpick(c)) => {
            options.iter().position(|o| o == c).unwrap_or(options.len())
        }
        _ => usize::MAX,
    };
    actions.sort_by_key(place);
    actions
}

/// The card choice of the fight as a sheet: what asks, the cards on offer, and the picks so
/// far in the title's count.
fn choice_sheet(combat: &Combat, describer: &Describer, sel: &Selection) -> Option<Sheet> {
    let DecisionRequest::ChooseCards {
        options,
        max,
        min,
        picked,
        prompt,
    } = combat.pending_decision()?;
    let state = combat.state();
    let source = describer.prompt_source(state, prompt.source);
    let ask = describer
        .prompt(state, *prompt, *min, *max)
        .map_or_else(|| "Choose".to_owned(), |t| text::plain(&t));
    let ask = ask.trim_end_matches('.');
    let count = format!("({} of {max})", picked.len());
    let title = if source.is_empty() {
        format!("{ask} {count}")
    } else {
        format!("{source} · {ask} {count}")
    };
    // Every option keeps its place: once the picks are full, the cards left are shown dimmed (no
    // answer picks them) instead of leaving the grid.
    let actions = choice_actions(combat);
    let mut rows = Vec::new();
    for (j, card) in options.iter().enumerate() {
        let found = actions.iter().enumerate().find(|(_, a)| {
            matches!(a, Action::Resolve(Decision::Pick(c) | Decision::Unpick(c)) if c == card)
        });
        let id = found.map_or_else(
            || crate::model::ButtonId(u32::MAX - u32::try_from(j).unwrap_or(0)),
            |(i, _)| ids::row(i),
        );
        let mut row = Button::row(id.0, card_label(combat, describer, *card, sel));
        row.right = Some(spans([(
            Role::Energy,
            format!("ϟ{}", combat.energy_cost(*card)),
        )]));
        row.detail = card_detail(describer, state, *card, None);
        row.card = Some(card_face(combat, describer, *card));
        match found {
            None => row.state = ButtonState::Dim(format!("{max} already picked")),
            Some((i, action)) => {
                row.key = Some(Key::Digit(u8::try_from(i + 1).unwrap_or(0) % 10));
                if matches!(action, Action::Resolve(Decision::Unpick(_))) {
                    // A picked card: tapped again, it goes back.
                    row.state = ButtonState::Picked;
                } else if sel.focus == Some(Item::Choice(i + 1, *card)) {
                    row.state = ButtonState::Selected;
                }
            }
        }
        rows.push(row);
    }
    let prompt = prompt_line(describer, state, *prompt, *min, *max);
    let prompt = if picked.is_empty() {
        prompt
    } else {
        format!(
            "{prompt}   picked {}/{max}: {}",
            picked.len(),
            names::cards(state, picked)
        )
    };
    Some(Sheet::Choice(ChoiceSheet {
        title: spans([(Role::Title, title)]),
        prompt: spans([(Role::Plain, prompt)]),
        rows,
        page: None,
    }))
}

/// "Headbutt: Choose a card to put on top of your Draw Pile.": what asks and the game's prompt,
/// or how many cards to pick without the game's text.
fn prompt_line(
    describer: &Describer,
    state: &CombatState,
    prompt: Prompt,
    min: usize,
    max: usize,
) -> String {
    let source = describer.prompt_source(state, prompt.source);
    let ask = describer.prompt(state, prompt, min, max).map_or_else(
        || {
            let plural = |n: usize| if n == 1 { "card" } else { "cards" };
            let count = match (min, max) {
                (0, max) => format!("up to {max} {}", plural(max)),
                (min, max) if min == max => format!("{max} {}", plural(max)),
                (min, max) => format!("{min} to {max} cards"),
            };
            format!("choose {count}")
        },
        |t| text::plain(&t),
    );
    if source.is_empty() {
        ask
    } else {
        format!("{source}: {ask}")
    }
}

/// "Strike → pick a target" while a card or potion waits for its target.
fn targeting_title(state: &CombatState, sel: &Selection) -> Option<Rich> {
    let Some(pending) = sel.pending else {
        let card = sel.armed?;
        let what = names::card(state, card);
        return Some(spans([(
            Role::Title,
            format!("{what} → drop it on the field"),
        )]));
    };
    let what = match pending {
        Pending::Card(card) => names::card(state, card),
        Pending::Potion(slot) => state
            .player
            .potions
            .get(slot)
            .copied()
            .flatten()
            .map_or_else(String::new, |p| names::potion(p.kind.id())),
    };
    Some(spans([(Role::Title, format!("{what} → pick a target"))]))
}

/// The main button: a card choice's Done, else End Turn (dim outside the player's turn).
fn main_button(combat: &Combat, sel: &Selection) -> Option<Button> {
    if combat.status() == Status::Decision {
        let at = choice_actions(combat)
            .iter()
            .position(|a| matches!(a, Action::Resolve(Decision::Done)))?;
        let mut done = Button::row(ids::row(at).0, spans([(Role::Key, "Done".to_owned())]));
        done.key = Some(Key::Digit(u8::try_from(at + 1).unwrap_or(0) % 10));
        if sel.focus == Some(Item::Done(at + 1)) {
            done.state = ButtonState::Selected;
        }
        return Some(done);
    }
    let mut end = Button::row(ids::END_TURN.0, spans([(Role::Key, "End Turn".to_owned())]));
    end.key = Some(Key::Char('e'));
    end.state = if combat.status() == Status::PlayerTurn {
        if sel.focus == Some(Item::EndTurn) {
            ButtonState::Selected
        } else {
            ButtonState::Enabled
        }
    } else {
        ButtonState::Disabled
    };
    Some(end)
}

/// One line per thief that got away with a card, for the rewards screen of a fight that ended
/// with the escape.
pub(crate) fn escaped_thefts(state: &CombatState) -> Vec<String> {
    let thieves = state.escaped.iter().filter(|&&c| {
        state
            .creature(c)
            .monster
            .as_ref()
            .is_some_and(|m| matches!(m.kind, MonsterKind::ThievingHopper(_)))
    });
    thieves
        .zip(lost_cards(state))
        .map(|(&thief, card)| {
            format!(
                "{} escaped with {}.",
                names::creature(state, thief),
                names::card(state, card)
            )
        })
        .collect()
}

/// The combat cards of deck cards a thief took that are neither held by a living thief nor on
/// offer as a reward: the ones an escaped thief kept, in the order they were taken.
fn lost_cards(state: &CombatState) -> Vec<CardId> {
    let held: Vec<usize> = state
        .enemies
        .iter()
        .flat_map(|&e| state.creature(e).powers.iter())
        .filter_map(|p| match state.power_data(p.id) {
            PowerData::Swipe(d) => d.stolen.and_then(|c| state.card(c).deck_version()),
            _ => None,
        })
        .collect();
    state
        .removed_from_deck
        .iter()
        .filter(|&&i| {
            !held.contains(&i) && !state.extra_rewards.contains(&ExtraReward::SpecialCard(i))
        })
        .filter_map(|&i| {
            state
                .cards
                .iter()
                .find(|c| c.deck_version() == Some(i))
                .map(|c| c.id)
        })
        .collect()
}

fn powers(
    state: &CombatState,
    describer: &Describer,
    creature: CreatureId,
    on_player: bool,
) -> Vec<PowerRow> {
    let owner = names::creature(state, creature);
    state
        .creature(creature)
        .powers
        .iter()
        .map(|power| PowerRow {
            name: names::power(power.kind).to_owned(),
            art: power.kind.id().to_owned(),
            amount: power.amount,
            debuff: power.kind.type_for_amount(Decimal::from(power.amount)) == PowerType::Debuff,
            shown: power.kind.is_visible(),
            next: if power.kind == PowerKind::Poison {
                poison_next(state, creature, power.amount)
            } else {
                0
            },
            detail: describer
                .power(
                    power.kind,
                    power.amount,
                    state.power_data(power.id),
                    &owner,
                    on_player,
                )
                .map_or_else(Vec::new, |markup| text::lines(&markup, Role::Plain)),
        })
        .collect()
}

/// `PoisonPower.CalculateTotalDamageNextTurn`: `Amount - i` for each of the `TriggerCount`
/// triggers (`Math.Min(Amount, 1 + the living opponents' Accelerant)`). The damage hooks on
/// unblockable, unpowered damage are not applied (the core exposes none to a view).
fn poison_next(state: &CombatState, owner: CreatureId, amount: i32) -> i32 {
    let opponents = match state.creature(owner).side {
        Side::Player => Side::Enemy,
        Side::Enemy => Side::Player,
    };
    let accelerant: i32 = state
        .creatures_on(opponents)
        .into_iter()
        .map(|c| state.creature(c))
        .filter(|c| c.is_alive())
        .map(|c| c.power_amount(PowerKind::Accelerant))
        .sum();
    (0..amount.min(1 + accelerant)).map(|i| amount - i).sum()
}

fn resources(state: &CombatState) -> Vec<Rich> {
    let mut out = Vec::new();
    if state.player.character == Character::Regent || state.player.stars != 0 {
        out.push(spans([(Role::Energy, format!("✦ {}", state.player.stars))]));
    }
    if state.player.character == Character::Defect {
        out.push(spans([(
            Role::Muted,
            format!("{} orb slot(s)", state.player.orb_slots),
        )]));
    }
    out
}

fn card_label(combat: &Combat, describer: &Describer, card: CardId, sel: &Selection) -> Rich {
    let state = combat.state();
    let mut out = Vec::new();
    out.push(RichSpan {
        fx: None,
        role: Role::Energy,
        bold: false,
        text: format!("ϟ{} ", combat.energy_cost(card)),
    });
    out.push(RichSpan {
        fx: None,
        role: Role::Title,
        bold: false,
        text: format!(
            "{}{}",
            names::card(state, card),
            names::card_tag(state, card).map_or_else(String::new, |t| format!(" {t}"))
        ),
    });
    let markup = describer
        .hand_card_text(state, card, sel.target)
        .or_else(|| describer.hand_card(state, card));
    if let Some(markup) = markup {
        let text = text::inline(&markup, Role::Body);
        out.extend(text.0);
    }
    Rich(out)
}

/// The Sovereign Blades in every pile.
fn blades(state: &CombatState) -> Vec<BladeRow> {
    let p = &state.player;
    let mut out = Vec::new();
    for (place, ids) in [
        ("hand", &p.hand),
        ("draw", &p.draw),
        ("discard", &p.discard),
        ("exhaust", &p.exhaust),
        ("play", &p.play),
    ] {
        for &id in ids {
            let card = state.card(id);
            let CardData::SovereignBlade(d) = card.data else {
                continue;
            };
            if card.kind != CardKind::SovereignBlade {
                continue;
            }
            out.push(BladeRow {
                card: id.0,
                damage: d.damage.get().to_int_truncating(),
                repeats: d.repeats.get().to_int_truncating(),
                place: place.to_owned(),
            });
        }
    }
    out
}

/// The cost of a combat card as the game shows it: the energy (`X`, `—` for an unplayable
/// card), then `·s2` for a star cost.
#[must_use]
pub fn cost_label(state: &CombatState, id: CardId) -> String {
    let card = state.card(id);
    let energy = if card.energy_cost.costs_x {
        "X".to_owned()
    } else {
        match state.energy_cost(id) {
            n if n < 0 => "—".to_owned(),
            n => n.to_string(),
        }
    };
    let stars = if card.star_cost.costs_x {
        "·sX".to_owned()
    } else {
        match state.star_cost(id) {
            n if n < 0 => String::new(),
            n => format!("·s{n}"),
        }
    };
    format!("{energy}{stars}")
}

/// A card type as a word.
#[must_use]
pub const fn type_word(kind: CardType) -> &'static str {
    match kind {
        CardType::Attack => "Attack",
        CardType::Skill => "Skill",
        CardType::Power => "Power",
        CardType::Status => "Status",
        CardType::Curse => "Curse",
        CardType::Quest => "Quest",
    }
}

/// The colour of a card's frame, as the game's frame material names it.
///
/// `CardModel.VisualCardPool.CardFrameMaterialPath`: the character pool's (`red`, `green`,
/// `blue`, `orange`, `pink`), else `colorless`, `curse` or `quest`. `entry` is the card's
/// `ModelId.Entry`.
#[must_use]
pub fn frame_color(entry: &str, card_type: CardType) -> &'static str {
    use std::collections::HashMap;
    use std::sync::OnceLock;
    static OWNERS: OnceLock<HashMap<&'static str, Character>> = OnceLock::new();
    match card_type {
        CardType::Curse => return "curse",
        CardType::Status => return "colorless",
        CardType::Quest => return "quest",
        _ => {}
    }
    if sts2_core::content::cards::pools::is_visual_pool_colorless(entry) {
        return "colorless";
    }
    let owners = OWNERS.get_or_init(|| {
        Character::ALL
            .into_iter()
            .flat_map(|c| sts2_core::run::cards::unlocked_cards(c).map(move |card| (card.id, c)))
            .collect()
    });
    match owners.get(entry) {
        Some(Character::Ironclad) => "red",
        Some(Character::Silent) => "green",
        Some(Character::Defect) => "blue",
        Some(Character::Regent) => "orange",
        Some(Character::Necrobinder) => "pink",
        None => "colorless",
    }
}

/// A card's rarity as a frame word: starter cards are `Basic`, event and token cards `Special`.
#[must_use]
pub const fn rarity_word(rarity: CardRarity) -> &'static str {
    match rarity {
        CardRarity::None | CardRarity::Quest => "",
        CardRarity::Basic => "Starter",
        CardRarity::Common => "Common",
        CardRarity::Uncommon => "Uncommon",
        CardRarity::Rare => "Rare",
        CardRarity::Ancient => "Ancient",
        CardRarity::Event | CardRarity::Token => "Special",
        CardRarity::Status => "Status",
        CardRarity::Curse => "Curse",
    }
}

/// A card's markup on one line: its sentences joined by ` · `, without their full stops.
pub(crate) fn joined(markup: &str) -> String {
    markup
        .trim()
        .trim_end_matches('.')
        .replace(".\n", " · ")
        .replace('\n', " · ")
}

/// A hand card in parts: cost, name, type, its text on one line (the numbers a power changed
/// lit) and in full.
pub(crate) fn card_face(combat: &Combat, describer: &Describer, id: CardId) -> CardFace {
    card_face_at(combat, describer, id, None)
}

/// [`card_face`] with the numbers against `target`, as the game shows a card that is aimed.
#[must_use]
pub fn card_face_at(
    combat: &Combat,
    describer: &Describer,
    id: CardId,
    target: Option<CreatureId>,
) -> CardFace {
    let state = combat.state();
    let tag = names::card_tag(state, id).map_or_else(String::new, |t| format!(" {t}"));
    let (effect, text) = describer.hand_card_at(state, id, target).map_or_else(
        || {
            let words = crate::describe::vars_text(state, id, target).replace('\n', " ");
            let plain = Rich(vec![RichSpan {
                fx: None,
                role: Role::Body,
                bold: false,
                text: words,
            }]);
            (plain.clone(), plain)
        },
        |m| {
            let mut effect = text::inline(&joined(&m), Role::Body);
            if let Some(base) = describer.base_hand_card_text(state, id) {
                let live = describer
                    .hand_card_text(state, id, target)
                    .unwrap_or_else(|| m.clone());
                effect = text::lit_changes(
                    effect,
                    &text::plain(&joined(&base)),
                    &text::plain(&joined(&live)),
                );
            }
            (effect, text::inline(&m, Role::Body))
        },
    );
    CardFace {
        card: id.0,
        art: state.card(id).kind.entry().to_owned(),
        cost: cost_label(state, id),
        name: format!("{}{tag}", names::card(state, id)),
        kind: type_word(state.card(id).card_type()).to_owned(),
        rarity: rarity_word(state.card(id).kind.rarity()).to_owned(),
        frame: frame_color(state.card(id).kind.entry(), state.card(id).card_type()).to_owned(),
        effect,
        text,
        glow: false,
        aims: state.target_type(id).needs_target(),
        before: None,
        upgrade: None,
    }
}

fn card_detail(
    describer: &Describer,
    state: &CombatState,
    card: CardId,
    target: Option<CreatureId>,
) -> Vec<Rich> {
    describer
        .hand_card_text(state, card, target)
        .map_or_else(Vec::new, |markup| text::lines(&markup, Role::Plain))
}

/// The game's tip for each of an enemy's intents (title, text), by creature id: what its tap
/// reads. Without the game's text there are none.
#[must_use]
pub fn intent_tips(combat: &Combat, describer: &Describer) -> Vec<(u32, Vec<(String, Rich)>)> {
    let state = combat.state();
    state
        .enemies
        .iter()
        .filter(|&&e| state.creature(e).is_alive() && !state.escaped.contains(&e))
        .map(|&e| {
            let tips = state
                .intents(e)
                .iter()
                .filter_map(|i| describer.intent_tip(i))
                .map(|(title, text)| {
                    let line = text::lines(&text, Role::Plain).into_iter().next();
                    (title, line.unwrap_or_default())
                })
                .collect();
            (e.0, tips)
        })
        .collect()
}

/// The enemies' intents as one rich line, or `None` when there are none.
fn intent_rich(state: &CombatState, enemy: CreatureId, verbose: bool) -> Option<Rich> {
    let intents = state.intents(enemy);
    if intents.is_empty() {
        return None;
    }
    let mut out = Vec::new();
    for (i, intent) in intents.iter().enumerate() {
        if i > 0 {
            out.push(RichSpan {
                fx: None,
                role: Role::Plain,
                bold: false,
                text: "  ".to_owned(),
            });
        }
        let (glyph, role) = intent_glyph(intent);
        out.push(RichSpan {
            fx: None,
            role,
            bold: true,
            text: format!("{glyph} {}", intent_words(intent, verbose)),
        });
    }
    Some(Rich(out))
}

/// What an intent does in a few words: `3`, `6×3`, `Weak 2`, `2 Slimed`, else its kind in a
/// word (`Asleep`, `Blocks`). `verbose` says `3 damage` rather than `3` (the big screen).
#[must_use]
pub fn intent_words(intent: &Intent, verbose: bool) -> String {
    if let Some(a) = intent.attack {
        let hits = if a.hits > 1 {
            format!("{}×{}", a.damage, a.hits)
        } else {
            a.damage.to_string()
        };
        return if verbose {
            format!("{hits} damage")
        } else {
            hits
        };
    }
    if let Some(n) = intent.cards.filter(|&n| n > 0) {
        return intent.card.map_or_else(
            || format!("{n} status card{}", if n == 1 { "" } else { "s" }),
            |card| format!("{n} {}", card.name()),
        );
    }
    // The game's debuff intents show no number (nor the power): the icon says it.
    let debuff = matches!(
        intent.kind,
        IntentType::Debuff | IntentType::DebuffStrong | IntentType::CardDebuff
    );
    let applies: Vec<String> = intent
        .applies
        .iter()
        .map(|&(power, n)| format!("{} {n}", names::power(power)))
        .collect();
    if !debuff && !applies.is_empty() {
        return applies.join(", ");
    }
    intent_word(intent.kind).to_owned()
}

/// An intent's kind in a word, for an intent whose glyph would otherwise stand alone.
#[must_use]
pub const fn intent_word(kind: IntentType) -> &'static str {
    match kind {
        IntentType::Attack | IntentType::DeathBlow => "Attacks",
        IntentType::Buff => "Buffs",
        IntentType::Debuff | IntentType::DebuffStrong | IntentType::CardDebuff => "Debuffs",
        IntentType::StatusCard => "Status cards",
        IntentType::Defend => "Blocks",
        IntentType::Escape => "Flees",
        IntentType::Heal => "Heals",
        IntentType::Hidden | IntentType::Unknown => "Unknown",
        IntentType::Summon => "Summons",
        IntentType::Sleep => "Asleep",
        IntentType::Stun => "Stunned",
    }
}

/// An intent's glyph and role, as the game's icons.
#[must_use]
pub const fn intent_glyph(intent: &Intent) -> (&'static str, Role) {
    match intent.kind {
        IntentType::Attack | IntentType::DeathBlow => ("⚔", Role::Attack),
        IntentType::Buff => ("▲", Role::Buff),
        IntentType::Debuff | IntentType::DebuffStrong | IntentType::CardDebuff => {
            ("▼", Role::Debuff)
        }
        IntentType::StatusCard => ("✚", Role::Debuff),
        IntentType::Defend => ("▣", Role::Defend),
        IntentType::Escape => ("»", Role::Unknown),
        IntentType::Heal => ("♥", Role::Buff),
        IntentType::Hidden | IntentType::Unknown => ("?", Role::Unknown),
        IntentType::Summon => ("✦", Role::Buff),
        IntentType::Sleep => ("z", Role::Unknown),
        IntentType::Stun => ("✶", Role::Unknown),
    }
}

fn spans<const N: usize>(parts: [(Role, String); N]) -> Rich {
    Rich(
        parts
            .into_iter()
            .map(|(role, text)| RichSpan {
                fx: None,
                role,
                bold: false,
                text,
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;

    use sts2_core::{Character, CombatConfig};

    #[test]
    fn a_frame_is_its_pools_colour() {
        assert_eq!(frame_color("BASH", CardType::Attack), "red");
        assert_eq!(frame_color("ZAP", CardType::Skill), "blue");
        assert_eq!(frame_color("STRIKE_NECROBINDER", CardType::Attack), "pink");
        assert_eq!(frame_color("FINESSE", CardType::Skill), "colorless");
        assert_eq!(frame_color("DAZED", CardType::Status), "colorless");
        assert_eq!(frame_color("INJURY", CardType::Curse), "curse");
    }

    fn combat() -> (Combat, Describer) {
        let config = CombatConfig::new(Character::Necrobinder, "RL2S0", 10)
            .expect("config")
            .with_events();
        let combat = Combat::new(config);
        (combat, Describer::new(Rc::new(text::Localization::empty())))
    }

    #[test]
    fn a_fight_has_the_enemies_the_hand_and_end_turn() {
        let (combat, describer) = combat();
        let screen = screen(&combat, &describer, &Selection::default());
        assert!(screen.top.is_none(), "the bar is Ui's: it needs the run");
        assert!(screen.blocks.iter().any(|b| matches!(b, Block::Enemy(_))));
        assert!(screen.blocks.iter().any(|b| matches!(b, Block::You(_))));
        let Block::List { rows, .. } = screen
            .blocks
            .iter()
            .find(|b| matches!(b, Block::List { .. }))
            .expect("a hand")
        else {
            unreachable!()
        };
        assert_eq!(rows.len(), combat.state().player.hand.len());
        assert_eq!(screen.primary.expect("end turn").key, Some(Key::Char('e')));
    }

    #[test]
    fn a_selected_card_is_the_selected_row() {
        let (combat, describer) = combat();
        let card = combat.state().player.hand[0];
        let screen = screen(
            &combat,
            &describer,
            &Selection {
                selected: Some(card),
                ..Selection::default()
            },
        );
        let Block::List { rows, .. } = screen
            .blocks
            .iter()
            .find(|b| matches!(b, Block::List { .. }))
            .expect("a hand")
        else {
            unreachable!()
        };
        assert!(rows.iter().any(|row| row.state == ButtonState::Selected));
    }

    fn hand_glows(combat: &Combat, describer: &Describer) -> Vec<bool> {
        let screen = screen(combat, describer, &Selection::default());
        let Some(Block::List { rows, .. }) = screen
            .blocks
            .iter()
            .find(|b| matches!(b, Block::List { .. }))
        else {
            unreachable!("a hand")
        };
        rows.iter()
            .map(|r| r.card.as_ref().expect("a card row").glow)
            .collect()
    }

    #[test]
    fn hand_cards_glow_gold_when_their_condition_holds() {
        // Impatience glows with no Attack in hand (`ShouldGlowGoldInternal`).
        let describer = Describer::new(Rc::new(text::Localization::empty()));
        let mut config = CombatConfig::new(Character::Necrobinder, "RL2S0", 10).expect("config");
        config.deck = vec![sts2_core::combat::DeckCard::new(CardKind::Impatience); 10];
        let combat = Combat::new(config.clone());
        assert!(hand_glows(&combat, &describer).iter().all(|&g| g));
        config.deck[..5].fill(sts2_core::combat::DeckCard::new(
            CardKind::StrikeNecrobinder,
        ));
        let combat = Combat::new(config);
        assert!(hand_glows(&combat, &describer).iter().all(|&g| !g));
    }
}
