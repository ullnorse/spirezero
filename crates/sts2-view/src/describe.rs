//! Descriptions of cards, relics, potions, powers and events: the game's text with its numbers
//! filled in from what the engine's public API can tell.
//!
//! A card's numbers are its `DynamicVars` as the engine previews them
//! (`CombatState::card_preview`, `RunCard::preview`): in the hand, damage and block with the
//! global hooks (Strength, Dexterity and the like); elsewhere the enchantment's changes only; a
//! calculated var is calculated in a fight and shows its base outside one, as in the game.
//! Values the card has no var for stay as `‹Name›` (see `text::format`).

use std::collections::HashMap;
use std::rc::Rc;

use sts2_core::Character;
use sts2_core::combat::state::{Orb, OrbKind};
use sts2_core::combat::{
    CardId, CombatState, CreatureId, Preventer, Prompt, PromptSource, PromptText, UnplayableReason,
};
use sts2_core::combat::{Intent, IntentType};
use sts2_core::content::cards::{CardKeyword, CardType, Keywords, PreviewMode, TargetType, Var};
use sts2_core::content::enchantments::{Enchantment, EnchantmentKind};
use sts2_core::content::events::EventVar;
use sts2_core::content::potions::PotionKind;
use sts2_core::content::powers::{PowerData, PowerKind};
use sts2_core::content::relics::{RelicData, RelicKind};
use sts2_core::content::tables::{ModelVar, VarText};
use sts2_core::decimal::Decimal;
use sts2_core::run::odds::PotionRarity;
use sts2_core::run::potions;
use sts2_core::run::{Modifier, RestOptionKind, Run, RunCard, SelectPrompt};

use crate::names;
use crate::text::{self, Localization, Value};

/// Card numbers by placeholder name.
type CardVars = HashMap<String, Value>;

/// The keywords a card's hover tips list first, in `CardKeyword` order, with their entry in
/// `card_keywords`.
pub const KEYWORDS: [(CardKeyword, &str); 7] = [
    (CardKeyword::Exhaust, "EXHAUST"),
    (CardKeyword::Ethereal, "ETHEREAL"),
    (CardKeyword::Innate, "INNATE"),
    (CardKeyword::Unplayable, "UNPLAYABLE"),
    (CardKeyword::Retain, "RETAIN"),
    (CardKeyword::Sly, "SLY"),
    (CardKeyword::Eternal, "ETERNAL"),
];

/// The keywords whose line goes above the description, top to bottom (`CardKeywordOrder.beforeDescription`
/// is inserted at the top one by one, so the game shows it reversed).
const KEYWORDS_BEFORE: [CardKeyword; 5] = [
    CardKeyword::Unplayable,
    CardKeyword::Innate,
    CardKeyword::Retain,
    CardKeyword::Sly,
    CardKeyword::Ethereal,
];

/// The keywords whose line goes below the description (`CardKeywordOrder.afterDescription`).
const KEYWORDS_AFTER: [CardKeyword; 2] = [CardKeyword::Exhaust, CardKeyword::Eternal];

/// One glossary entry: a term's title and its description (markup).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Term {
    pub title: String,
    pub text: String,
}

/// Looks up and fills in game text. Holds the loaded localization and the hand's numbers.
#[derive(Clone, Debug)]
pub struct Describer {
    loc: Rc<Localization>,
    /// The numbers of each card in the hand of the fight in progress.
    hand: HashMap<CardId, CardVars>,
    /// The glossary's terms by lower-case title: `(table, entry)` of the keywords, powers and
    /// static hover tips.
    terms: Rc<HashMap<String, (&'static str, String)>>,
}

impl Describer {
    #[must_use]
    pub fn new(loc: Rc<Localization>) -> Self {
        let mut terms = HashMap::new();
        let mut add = |table: &'static str, entry: String| {
            if let Some(title) = loc.get(table, &format!("{entry}.title")) {
                terms.entry(title.to_lowercase()).or_insert((table, entry));
            }
        };
        for (_, entry) in KEYWORDS {
            add("card_keywords", entry.to_owned());
        }
        for kind in PowerKind::ALL {
            add("powers", kind.id().to_owned());
        }
        for key in loc.keys("static_hover_tips") {
            if let Some(entry) = key.strip_suffix(".title") {
                add("static_hover_tips", entry.to_owned());
            }
        }
        Self {
            loc,
            hand: HashMap::new(),
            terms: Rc::new(terms),
        }
    }

    /// The localization this describes with.
    #[must_use]
    pub fn localization(&self) -> Rc<Localization> {
        Rc::clone(&self.loc)
    }

    /// Whether any game text was found.
    #[must_use]
    pub fn has_text(&self) -> bool {
        !self.loc.is_empty()
    }

    /// Re-reads the numbers of the hand; call after every action.
    pub fn refresh(&mut self, run: &Run) {
        self.hand.clear();
        if !self.has_text() {
            return;
        }
        let Some(combat) = run.combat() else {
            return;
        };
        let state = combat.state();
        for &id in &state.player.hand {
            self.hand.insert(id, combat_card_vars(state, id, None));
        }
    }

    fn text(&self, table: &str, key: &str) -> Option<&str> {
        self.loc.get(table, key)
    }

    fn card_markup(
        &self,
        entry: &str,
        upgrade_level: u8,
        target: TargetType,
        vars: &CardVars,
        in_combat: bool,
        keywords: Keywords,
    ) -> Option<String> {
        let template = self.text("cards", &format!("{entry}.description"))?;
        let target = format!("{target:?}");
        let body = text::format(template, &|name| match name {
            "IfUpgraded" => Some(Value::Flag(upgrade_level > 0)),
            "InCombat" => Some(Value::Flag(in_combat)),
            "TargetType" => Some(Value::Name(target.clone())),
            _ => vars.get(name).cloned().or_else(|| global(name)),
        });
        let lines = |order: &[CardKeyword]| -> Vec<String> {
            order
                .iter()
                .filter(|&&k| keywords.contains(k))
                .filter_map(|&k| self.keyword_line(k))
                .collect()
        };
        let mut out = lines(&KEYWORDS_BEFORE);
        if !body.is_empty() {
            out.push(body);
        }
        out.extend(lines(&KEYWORDS_AFTER));
        Some(out.join("\n"))
    }

    /// `CardKeyword.GetCardText`: the keyword's gold title and the period.
    fn keyword_line(&self, keyword: CardKeyword) -> Option<String> {
        let (_, entry) = KEYWORDS.iter().find(|&&(k, _)| k == keyword)?;
        let title = self.text("card_keywords", &format!("{entry}.title"))?;
        let period = self.text("card_keywords", "PERIOD").unwrap_or(".");
        Some(format!("[gold]{title}[/gold]{period}"))
    }

    /// The description of a card of the fight: the hand's with the global hooks, a card in a
    /// pile with its enchantment only (the game's pile screens show cards the same way).
    #[must_use]
    pub fn hand_card(&self, state: &CombatState, id: CardId) -> Option<String> {
        self.hand_card_at(state, id, None)
    }

    /// [`Describer::hand_card`] aimed at `target`: the numbers against that creature.
    #[must_use]
    pub fn hand_card_at(
        &self,
        state: &CombatState,
        id: CardId,
        target: Option<CreatureId>,
    ) -> Option<String> {
        let text = self.hand_card_text(state, id, target);
        let vars = state.enchantment_preview(id, target, PreviewMode::Normal);
        self.with_enchantment(text, state.card(id).enchantment, &vars)
    }

    /// [`Describer::hand_card`] without the enchantment's text, aimed at `target` (the numbers
    /// against that creature, as the game shows a card being aimed).
    #[must_use]
    pub fn hand_card_text(
        &self,
        state: &CombatState,
        id: CardId,
        target: Option<CreatureId>,
    ) -> Option<String> {
        let card = state.card(id);
        let cached = target.is_none().then(|| self.hand.get(&id)).flatten();
        let vars = cached
            .cloned()
            .unwrap_or_else(|| combat_card_vars(state, id, target));
        self.card_markup(
            card.kind.entry(),
            card.upgrade_level,
            state.target_type(id),
            &vars,
            true,
            card_text_keywords(state, id),
        )
    }

    /// [`Describer::hand_card_text`] with the card's base values (`RunCard::preview`, no
    /// fight): what its numbers are before Strength, Weak or the enchantment change them.
    #[must_use]
    pub fn base_hand_card_text(&self, state: &CombatState, id: CardId) -> Option<String> {
        let card = state.card(id);
        let mut base = RunCard::new(card.kind.entry());
        base.upgrade_level = card.upgrade_level;
        let vars = base.preview().map(to_card_vars).unwrap_or_default();
        self.card_markup(
            card.kind.entry(),
            card.upgrade_level,
            state.target_type(id),
            &vars,
            true,
            card_text_keywords(state, id),
        )
    }

    /// The description of `card` one upgrade up (what upgrading it would make it), if it can
    /// be upgraded.
    #[must_use]
    pub fn upgraded_run_card(&self, card: RunCard) -> Option<String> {
        if !card.is_upgradable() {
            return None;
        }
        let mut up = card;
        up.upgrade();
        self.run_card(up)
    }

    /// The description of a deck, reward or shop card.
    #[must_use]
    pub fn run_card(&self, card: RunCard) -> Option<String> {
        let text = self.run_card_text(card);
        let vars = card.enchantment_preview().unwrap_or_default();
        self.with_enchantment(text, card.enchantment, &vars)
    }

    /// [`Describer::run_card`] without the enchantment's text.
    fn run_card_text(&self, card: RunCard) -> Option<String> {
        if !self.has_text() {
            return None;
        }
        let vars = card.preview().map(to_card_vars).unwrap_or_default();
        let target = card
            .to_combat()
            .map_or(TargetType::None, |c| c.kind.target());
        let keywords = card.card().map_or(Keywords::NONE, |c| c.local_keywords());
        self.card_markup(card.id, card.upgrade_level, target, &vars, false, keywords)
    }

    /// An enchantment's description (`ENCHANTMENT.description`): `Amount`, and its own vars
    /// as the card previews them (`CombatState::enchantment_preview`, `RunCard::enchantment_preview`:
    /// Adroit's `Block` with Dexterity in the hand).
    #[must_use]
    pub fn enchantment(
        &self,
        enchantment: Enchantment,
        vars: &[(&'static str, Decimal)],
    ) -> Option<String> {
        let kind = enchantment.kind;
        let template = self.text("enchantments", &format!("{}.description", kind.id()))?;
        let amount = i64::from(enchantment.amount);
        Some(text::format(template, &|name| {
            if let Some(&(_, value)) = vars.iter().find(|&&(n, _)| n == name) {
                return Some(Value::Int(i64::from(value.to_int_truncating())));
            }
            (name == "Amount").then_some(Value::Int(amount))
        }))
    }

    /// `text`, then a line with the enchantment's name and description (the name alone without
    /// game text).
    fn with_enchantment(
        &self,
        text: Option<String>,
        enchantment: Option<Enchantment>,
        vars: &[(&'static str, Decimal)],
    ) -> Option<String> {
        let Some(enchantment) = enchantment else {
            return text;
        };
        let title = self
            .text("enchantments", &format!("{}.title", enchantment.kind.id()))
            .unwrap_or_else(|| enchantment.kind.name());
        let line = self.enchantment(enchantment, vars).map_or_else(
            || format!("[purple]✦ {title}[/purple]"),
            |d| format!("[purple]✦ {title}:[/purple] {d}"),
        );
        Some(match text {
            Some(t) if !t.is_empty() => format!("{t}\n{line}"),
            _ => line,
        })
    }

    /// The description of a relic as offered (a reward, the shop, an ancient): its canonical
    /// vars (`RelicKind::vars` of a fresh relic).
    #[must_use]
    pub fn relic(&self, id: &str) -> Option<String> {
        RelicKind::from_id(id).map_or_else(
            || self.model_description("relics", id, &[], &[]),
            |kind| self.owned_relic(kind, &kind.init_data()),
        )
    }

    /// The description of an owned relic with its fields `data`.
    #[must_use]
    pub fn owned_relic(&self, kind: RelicKind, data: &RelicData) -> Option<String> {
        self.model_description("relics", kind.id(), &kind.vars(data), kind.canonical_vars())
    }

    /// A rest-site option's name and description in the game's text (`rest_site_ui`,
    /// `OPTION_<id>`), its numbers from `vars` ([`sts2_core::run::RunState::rest_option_vars`]); a disabled option
    /// gets its `descriptionDisabled` line where the game has one.
    #[must_use]
    pub fn rest_option(
        &self,
        kind: RestOptionKind,
        vars: &[(&'static str, Decimal)],
        enabled: bool,
    ) -> Option<(String, String)> {
        let key = format!("OPTION_{}", kind.id());
        let name = self
            .text("rest_site_ui", &format!("{key}.name"))?
            .to_owned();
        let disabled = (!enabled)
            .then(|| self.text("rest_site_ui", &format!("{key}.descriptionDisabled")))
            .flatten();
        let template =
            disabled.or_else(|| self.text("rest_site_ui", &format!("{key}.description")))?;
        let description = text::format(template, &|var| {
            if let Some(&(_, value)) = vars.iter().find(|&&(n, _)| n == var) {
                // `DynamicVar.ToString` is `IntValue`: the heal shows truncated.
                return Some(Value::Int(i64::from(value.to_int_truncating())));
            }
            match var {
                "RelicName" => Some(Value::Name(names::relic(RelicKind::PumpkinCandle.id()))),
                "EnchantmentName" => Some(Value::Name(EnchantmentKind::Clone.name().to_owned())),
                "ExtraText" => Some(Value::Name(String::new())),
                _ => global(var),
            }
        });
        Some((name, description))
    }

    /// The description of a potion with its vars (`PotionKind::vars`).
    #[must_use]
    pub fn potion(&self, id: &str) -> Option<String> {
        let vars = PotionKind::from_id(id)
            .map(PotionKind::vars)
            .unwrap_or_default();
        self.model_description("potions", id, &vars, &[])
    }

    /// A relic's or potion's description: `vars` are its numbers, `declared` its vars as
    /// declared (an enchantment `StringVar` shows the enchantment's title).
    fn model_description(
        &self,
        table: &str,
        id: &str,
        vars: &[(&'static str, Decimal)],
        declared: &[ModelVar],
    ) -> Option<String> {
        let template = self.text(table, &format!("{id}.description"))?;
        Some(text::format(template, &|name| {
            if let Some(&(_, value)) = vars.iter().find(|&&(n, _)| n == name) {
                return Some(decimal_value(value));
            }
            if let Some(text) = declared
                .iter()
                .find(|v| v.name == name)
                .and_then(|v| v.text)
            {
                return Some(Value::Name(self.var_text(text)));
            }
            global(name)
        }))
    }

    /// The description of a power with `amount` and fields `data` on `owner` (`on_player`: the
    /// owner is the player).
    ///
    /// The power's vars come from the engine (`PowerKind::vars`: Vulnerable's 1.5 shows as 50%,
    /// Slow's displayed amount, The Bomb's damage); if a number is still unknown the power's
    /// static description is shown instead.
    #[must_use]
    pub fn power(
        &self,
        kind: PowerKind,
        amount: i32,
        data: &PowerData,
        owner: &str,
        on_player: bool,
    ) -> Option<String> {
        let key = kind.id();
        let vars = kind.vars(data);
        let declared = kind.canonical_vars();
        let fill = |template: &str| {
            text::format(template, &|name| match name {
                "Amount" => Some(Value::Int(i64::from(amount))),
                "OwnerName" => Some(Value::Name(owner.to_owned())),
                "OnPlayer" => Some(Value::Flag(on_player)),
                _ => vars
                    .iter()
                    .find(|&&(n, _)| n == name)
                    .map(|&(_, value)| decimal_value(value))
                    .or_else(|| {
                        let text = declared.iter().find(|v| v.name == name)?.text?;
                        Some(Value::Name(self.var_text(text)))
                    })
                    .or_else(|| global(name)),
            })
        };
        let smart = self.text("powers", &format!("{key}.smartDescription"));
        let fixed = self.text("powers", &format!("{key}.description"));
        let filled = smart.or(fixed).map(fill)?;
        if filled.contains('‹')
            && let Some(fixed) = fixed
        {
            return Some(fill(fixed));
        }
        Some(filled)
    }

    /// What a card choice asks (`CardSelectorPrefs.Prompt`, or the choose-a-card banner) in
    /// `state`, the state at the choice: `Amount` and `MaxCount` are `max`, `MinCount` is `min`
    /// (the `CardSelectorPrefs` constructors), the rest the asking model's vars
    /// (`SelectionScreenPrompt` adds its `DynamicVars`).
    #[must_use]
    pub fn prompt(
        &self,
        state: &CombatState,
        prompt: Prompt,
        min: usize,
        max: usize,
    ) -> Option<String> {
        let (table, key) = prompt.key(state)?;
        let template = self.text(table, &key)?;
        let vars: Vec<(String, Decimal)> = match prompt.source {
            PromptSource::Card(card) => state
                .card_preview(card, None, PreviewMode::Normal)
                .into_iter()
                .map(|(var, n)| (var.to_string(), Decimal::from(n.to_int_truncating())))
                .collect(),
            PromptSource::Potion(kind) => named(kind.vars()),
            PromptSource::Relic(relic) => usize::try_from(relic.0)
                .ok()
                .and_then(|i| state.player.relics.get(i))
                .map(|r| named(r.kind.vars(&r.data)))
                .unwrap_or_default(),
            PromptSource::Power { owner, kind } => state
                .creature(owner)
                .powers
                .iter()
                .find(|p| p.kind == kind)
                .map(|p| named(kind.vars(state.power_data(p.id))))
                .unwrap_or_default(),
            PromptSource::Monster(_) => Vec::new(),
        };
        Some(text::format(template, &|name| {
            count_var(name, min, max)
                .or_else(|| {
                    vars.iter()
                        .find(|(n, _)| n == name)
                        .map(|&(_, value)| decimal_value(value))
                })
                .or_else(|| global(name))
        }))
    }

    /// A shared selection prompt (`card_selection.TO_REMOVE`, ...) with `Amount` `max` and
    /// `MinCount` `min`; `None` for [`PromptText::Own`] or without the text.
    #[must_use]
    pub fn shared_prompt(&self, text: PromptText, min: usize, max: usize) -> Option<String> {
        let (table, key) = text.shared_key()?;
        let template = self.text(table, key)?;
        Some(text::format(template, &|name| count_var(name, min, max)))
    }

    /// A run deck selection's prompt with `Amount` `max` and `MinCount` `min`; `None` without
    /// the text (or for [`PromptText::Own`]).
    #[must_use]
    pub fn select_prompt(&self, prompt: SelectPrompt, min: usize, max: usize) -> Option<String> {
        match prompt {
            SelectPrompt::Shared(text) => self.shared_prompt(text, min, max),
            SelectPrompt::Relic(id) => self.own_prompt("relics", id, min, max),
            SelectPrompt::EventPage(page) => self.own_prompt("events", page, min, max),
            SelectPrompt::Modifier(id) => {
                let template = self.text("modifiers", &format!("{id}.selectionPrompt"))?;
                Some(text::format(template, &|name| count_var(name, min, max)))
            }
        }
    }

    fn own_prompt(&self, table: &str, id: &str, min: usize, max: usize) -> Option<String> {
        let template = self.text(table, &format!("{id}.selectionScreenPrompt"))?;
        Some(text::format(template, &|name| count_var(name, min, max)))
    }

    /// A modifier's loc table and key stem (`CharacterCards` uses its character's).
    fn modifier_key(modifier: Modifier, field: &str) -> (&'static str, String) {
        modifier.character().map_or_else(
            || {
                (
                    "modifiers",
                    format!("{}.{}", modifier.id(), field.to_ascii_lowercase()),
                )
            },
            |c| {
                (
                    "characters",
                    format!("{}.cardsModifier{field}", c.name().to_ascii_uppercase()),
                )
            },
        )
    }

    /// A modifier's title (`ModifierModel.Title`).
    #[must_use]
    pub fn modifier_title(&self, modifier: Modifier) -> String {
        let (table, key) = Self::modifier_key(modifier, "Title");
        self.text(table, &key)
            .map_or_else(|| names::title_case(modifier.id()), str::to_owned)
    }

    /// A modifier's description (`ModifierModel.Description`), with its markup.
    #[must_use]
    pub fn modifier_description(&self, modifier: Modifier) -> Option<String> {
        let (table, key) = Self::modifier_key(modifier, "Description");
        self.text(table, &key).map(str::to_owned)
    }

    /// The localized title of a power (its name without the text).
    #[must_use]
    pub fn power_title(&self, kind: PowerKind) -> String {
        self.text("powers", &format!("{}.title", kind.id()))
            .map_or_else(|| names::power(kind).to_owned(), str::to_owned)
    }

    /// The localized title of a relic (its name without the text).
    #[must_use]
    pub fn relic_title(&self, id: &str) -> String {
        self.text("relics", &format!("{id}.title"))
            .map_or_else(|| names::title_case(id), str::to_owned)
    }

    /// The localized title of what asks a card choice (its name without the text).
    #[must_use]
    pub fn prompt_source(&self, state: &CombatState, source: PromptSource) -> String {
        let (table, id) = match source {
            PromptSource::Card(card) => {
                let card = state.card(card);
                let plus = if card.is_upgraded() { "+" } else { "" };
                return self
                    .text("cards", &format!("{}.title", card.kind.entry()))
                    .map_or_else(|| card.kind.name().to_owned(), str::to_owned)
                    + plus;
            }
            PromptSource::Potion(kind) => ("potions", kind.id()),
            PromptSource::Relic(relic) => {
                let Some(r) = usize::try_from(relic.0)
                    .ok()
                    .and_then(|i| state.player.relics.get(i))
                else {
                    return String::new();
                };
                ("relics", r.kind.id())
            }
            PromptSource::Power { kind, .. } => ("powers", kind.id()),
            PromptSource::Monster(id) => return names::creature(state, id),
        };
        self.text(table, &format!("{id}.title")).map_or_else(
            || match source {
                PromptSource::Potion(kind) => kind.name().to_owned(),
                PromptSource::Power { kind, .. } => names::power(kind).to_owned(),
                _ => names::title_case(id),
            },
            str::to_owned,
        )
    }

    /// An orb's title (`ORB.title`), or its id as a name without the text.
    #[must_use]
    pub fn orb_title(&self, kind: OrbKind) -> String {
        self.text("orbs", &format!("{}.title", kind.id()))
            .map_or_else(
                || names::title_case(kind.id().trim_end_matches("_ORB")),
                str::to_owned,
            )
    }

    /// An orb's hover text in `state` (`OrbModel.HoverTips`): its `smartDescription` with
    /// `Passive` and `Evoke` as they are now (Focus included), or its plain description.
    #[must_use]
    pub fn orb(&self, state: &CombatState, orb: Orb) -> Option<String> {
        let id = orb.kind.id();
        let template = self
            .text("orbs", &format!("{id}.smartDescription"))
            .or_else(|| self.text("orbs", &format!("{id}.description")))?;
        let passive = state.orb_passive_val(orb);
        let evoke = state.orb_evoke_val(orb);
        Some(text::format(template, &|name| match name {
            "Passive" => Some(decimal_value(passive)),
            "Evoke" => Some(decimal_value(evoke)),
            _ => global(name),
        }))
    }

    /// An empty orb slot's title and description (`OrbModel.EmptySlotHoverTipHoverTip`).
    #[must_use]
    pub fn empty_orb_slot(&self) -> Option<(&str, &str)> {
        Some((
            self.text("orbs", "EMPTY_SLOT.title")?,
            self.text("orbs", "EMPTY_SLOT.description")?,
        ))
    }

    /// An ascension level's title (`LEVEL_NN.title`).
    #[must_use]
    pub fn ascension_title(&self, level: u8) -> Option<String> {
        self.text("ascension", &format!("LEVEL_{level:02}.title"))
            .map(str::to_owned)
    }

    /// An ascension level's description.
    #[must_use]
    pub fn ascension_description(&self, level: u8) -> Option<String> {
        self.text("ascension", &format!("LEVEL_{level:02}.description"))
            .map(str::to_owned)
    }

    /// A character's blurb on the select screen, from its class name (`Ironclad`).
    #[must_use]
    pub fn character_description(&self, class_name: &str) -> Option<String> {
        let key = format!("{}.description", class_name.to_uppercase());
        self.text("characters", &key).map(str::to_owned)
    }

    /// The game's tip for a monster's intent (`intents`: `ATTACK.title` and `.description`):
    /// `Aggressive` and `This enemy intends to Attack for 11 damage.`
    #[must_use]
    pub fn intent_tip(&self, intent: &Intent) -> Option<(String, String)> {
        let id = match intent.kind {
            IntentType::Attack => "ATTACK",
            IntentType::Buff => "BUFF",
            IntentType::Debuff => "DEBUFF",
            IntentType::DebuffStrong => "DEBUFF_STRONG",
            IntentType::Defend => "DEFEND",
            IntentType::Escape => "ESCAPE",
            IntentType::Heal => "HEAL",
            IntentType::Hidden | IntentType::Unknown => "UNKNOWN",
            IntentType::Summon => "SUMMON",
            IntentType::Sleep => "SLEEP",
            IntentType::Stun => "STUN",
            IntentType::StatusCard => "STATUS",
            IntentType::CardDebuff => "CARD_DEBUFF",
            IntentType::DeathBlow => "DEATH_BLOW",
        };
        let title = self.text("intents", &format!("{id}.title"))?.to_owned();
        let description = self.text("intents", &format!("{id}.description"))?;
        let attack = intent.attack;
        let text = text::format(description, &|name| match name {
            "Damage" => attack.map(|a| Value::Int(i64::from(a.damage))),
            "Repeat" => attack.map(|a| Value::Int(i64::from(a.hits))),
            "CardCount" => Some(Value::Int(i64::from(intent.cards.unwrap_or(0)))),
            "IsMultiplayer" => Some(Value::Flag(false)),
            _ => None,
        });
        Some((title, text))
    }

    /// Whether the extra tables the phone client loads (`rest_site_ui`, `ancients`, `intents`)
    /// are here: its screens then use the game's wording where the terminal's keep our own lines.
    #[must_use]
    pub fn game_wording(&self) -> bool {
        self.text("rest_site_ui", "PROMPT").is_some()
    }

    /// A line of a localization table as it is (`rest_site_ui` `PROMPT`).
    #[must_use]
    pub fn line(&self, table: &str, key: &str) -> Option<String> {
        self.text(table, key).map(str::to_owned)
    }

    /// What an ancient says on its screen in the game's text: its epithet (`NEOW.epithet`),
    /// then, with a `character`, the first line the ancient speaks to that class
    /// (`NEOW.talk.<CLASS>.0-0.ancient`) when it has one.
    #[must_use]
    pub fn ancient_lines(&self, id: &str, character: Character) -> Vec<String> {
        let class = character.name().to_uppercase();
        [
            format!("{id}.epithet"),
            format!("{id}.talk.{class}.0-0.ancient"),
        ]
        .iter()
        .filter_map(|key| self.text("ancients", key).map(str::to_owned))
        .collect()
    }

    /// An act's title from its `ModelId.Entry` (`OVERGROWTH`).
    #[must_use]
    pub fn act_title(&self, id: &str) -> Option<String> {
        self.text("acts", &format!("{id}.title")).map(str::to_owned)
    }

    /// An event's title.
    #[must_use]
    pub fn event_title(&self, id: &str) -> Option<String> {
        self.text("events", &format!("{id}.title"))
            .map(str::to_owned)
    }

    /// The body text of the page an option's `text_key` belongs to, with the event's vars.
    #[must_use]
    pub fn event_body(&self, text_key: &str, event: &EventText<'_>) -> Option<String> {
        let page = text_key.split_once(".options.")?.0;
        let template = self.text("events", &format!("{page}.description"))?;
        Some(text::format(template, &|name| self.event_var(event, name)))
    }

    /// An event page's body from its description key, with the event's vars: a finished
    /// event's last page (`SetEventFinished(description)`, [`EventRoom::finished`]) or a page
    /// whose text is not its options' (`EventRoom::page_description`); `None` for an empty key.
    ///
    /// [`EventRoom::finished`]: sts2_core::run::EventRoom
    #[must_use]
    pub fn event_description(&self, key: &str, event: &EventText<'_>) -> Option<String> {
        let template = self.text("events", key).filter(|t| !t.is_empty())?;
        Some(text::format(template, &|name| self.event_var(event, name)))
    }

    /// An event option's title and description, with the event's vars.
    #[must_use]
    pub fn event_option(
        &self,
        text_key: &str,
        event: &EventText<'_>,
    ) -> Option<(String, Option<String>)> {
        let fill = |template: &str| text::format(template, &|name| self.event_var(event, name));
        // Some titles carry a variable too (`Insert {Rarity} Potion`).
        // The title's `{Rarity}` is the potion's, the description's the card's.
        let title_template = self.text("events", &format!("{text_key}.title"))?;
        let title = text::plain(&text::format(title_template, &|name| {
            let name = match (name, event.extra.iter().any(|(n, _)| *n == "PotionRarity")) {
                ("Rarity", true) => "PotionRarity",
                _ => name,
            };
            self.event_var(event, name)
        }));
        let description = self
            .text("events", &format!("{text_key}.description"))
            .filter(|d| !d.is_empty())
            .map(fill);
        Some((title, description))
    }

    /// `TheFutureOfPotions.GenerateInitialOptions`' vars for the option of `potion` with the
    /// rolled card `kind`: the potion's rarity (title), its title, the card rarity
    /// (`GetCardRarity`) and the card type, as the game's localized words.
    #[must_use]
    pub fn future_of_potions_vars(
        &self,
        potion: &str,
        kind: CardType,
    ) -> Vec<(&'static str, String)> {
        // `PotionRarity` of the event and token potions the run layer does not list.
        let rarity = match potion {
            "AMBERGRIS" | "FOUL_POTION" | "GLOWWATER_POTION" => "EVENT",
            "POTION_SHAPED_ROCK" => "TOKEN",
            id => match potions::potion_rarity(id) {
                Some(PotionRarity::Uncommon) => "UNCOMMON",
                Some(PotionRarity::Rare) => "RARE",
                _ => "COMMON",
            },
        };
        let card_rarity = match rarity {
            "UNCOMMON" => "UNCOMMON",
            "COMMON" | "TOKEN" => "COMMON",
            _ => "RARE",
        };
        let kind = match kind {
            CardType::Attack => "ATTACK",
            CardType::Skill => "SKILL",
            _ => "POWER",
        };
        let ui = |key: String| {
            self.text("gameplay_ui", &key)
                .unwrap_or_default()
                .to_owned()
        };
        vec![
            ("PotionRarity", ui(format!("POTION_RARITY.{rarity}"))),
            (
                "Potion",
                self.text("potions", &format!("{potion}.title"))
                    .map_or_else(|| names::potion(potion), str::to_owned),
            ),
            ("Rarity", ui(format!("CARD_RARITY.{card_rarity}"))),
            ("Type", ui(format!("CARD_TYPE.{kind}"))),
        ]
    }

    fn event_var(&self, event: &EventText<'_>, name: &str) -> Option<Value> {
        if let Some((_, text)) = event.extra.iter().find(|(n, _)| *n == name) {
            return Some(Value::Name(text.clone()));
        }
        let Some(&(_, var)) = event.vars.iter().find(|&&(n, _)| n == name) else {
            return self
                .character_var(event.character, name)
                .or_else(|| global(name));
        };
        Some(match var {
            // `DynamicVar.ToString` is `IntValue`: a fractional event var (Whirlpool's 26.4 heal) shows truncated.
            EventVar::Number(n) => Value::Int(i64::from(n.to_int_truncating())),
            // `{CHARACTER}.aromaPrinciple`: the player's character's text.
            EventVar::Text(VarText::Character(field)) => {
                let id = event.character?.name().to_uppercase();
                Value::Name(
                    self.text("characters", &format!("{id}.{field}"))?
                        .to_owned(),
                )
            }
            EventVar::Text(text) => Value::Name(self.var_text(text)),
        })
    }

    /// `CharacterModel.AddDetailsTo`'s variables (`character`, `pronounSubject`, ...) and
    /// `EventOption`'s `IsMultiplayer` (single player: false).
    fn character_var(&self, character: Option<Character>, name: &str) -> Option<Value> {
        let field = match name {
            "IsMultiplayer" => return Some(Value::Flag(false)),
            "character" => "title",
            "characterObject" => "titleObject",
            "possessiveAdjective" | "pronounObject" | "pronounPossessive" | "pronounSubject" => {
                name
            }
            _ => return None,
        };
        let id = character?.name().to_uppercase();
        let text = self.text("characters", &format!("{id}.{field}"))?;
        Some(Value::Name(text.to_owned()))
    }

    /// The glossary of a card or power: the card's keywords (`keywords`, in the game's order),
    /// then every term the text names in gold (`[gold]Vulnerable[/gold]`) that is a keyword,
    /// a power or a static hover tip, each once, with the game's description.
    #[must_use]
    pub fn glossary(&self, markup: &str, keywords: Keywords) -> Vec<Term> {
        let mut found: Vec<(&'static str, String)> = KEYWORDS
            .iter()
            .filter(|&&(k, _)| keywords.contains(k))
            .map(|&(_, entry)| ("card_keywords", entry.to_owned()))
            .collect();
        for word in gold_words(markup) {
            let word = word
                .trim_matches(|c: char| !c.is_alphanumeric())
                .to_lowercase();
            // `Exhausted`, `Channeled`: the term's inflections.
            let hit = [
                Some(word.as_str()),
                word.strip_suffix("ed"),
                word.strip_suffix('d'),
                word.strip_suffix('s'),
            ]
            .into_iter()
            .flatten()
            .find_map(|w| self.terms.get(w));
            if let Some(term) = hit
                && !found.contains(term)
            {
                found.push(term.clone());
            }
        }
        found
            .into_iter()
            .filter_map(|(table, entry)| {
                let title = self.text(table, &format!("{entry}.title"))?;
                let text = self.text(table, &format!("{entry}.description"))?;
                Some(Term {
                    title: title.to_owned(),
                    text: text::format(text, &|name| global(name)),
                })
            })
            .collect()
    }

    /// Why a card cannot be played, as the game's thought bubble says it
    /// (`combat_messages`, one line), or in plain words without the text.
    #[must_use]
    pub fn unplayable(&self, state: &CombatState, reason: UnplayableReason) -> String {
        let (key, fallback) = match reason {
            UnplayableReason::NotEnoughEnergy => ("NOT_ENOUGH_ENERGY", "Not enough Energy."),
            UnplayableReason::NotEnoughStars => ("NOT_ENOUGH_STARS", "Not enough Stars."),
            UnplayableReason::BlockedBy(_) => ("BLOCKED_BY_HOOK", "Blocked by {BlockingHook}."),
            UnplayableReason::Unplayable => ("UNPLAYABLE", "This card can't be played."),
        };
        let template = self.text("combat_messages", key).unwrap_or(fallback);
        let hook = match reason {
            UnplayableReason::BlockedBy(preventer) => self.preventer_title(state, preventer),
            _ => String::new(),
        };
        text::format(template, &|name| {
            (name == "BlockingHook").then(|| Value::Name(hook.clone()))
        })
        .replace('\n', " ")
    }

    /// The title of what blocks a card (`GetPlayerDialogueLine`'s `BlockingHook`).
    fn preventer_title(&self, state: &CombatState, preventer: Preventer) -> String {
        let titled = |table: &str, id: &str, fallback: &str| {
            self.text(table, &format!("{id}.title"))
                .map_or_else(|| fallback.to_owned(), str::to_owned)
        };
        match preventer {
            Preventer::Power(kind) => titled("powers", kind.id(), names::power(kind)),
            Preventer::Relic(kind) => titled("relics", kind.id(), kind.name()),
            Preventer::Potion(kind) => titled("potions", kind.id(), kind.name()),
            Preventer::Monster(id) => names::creature(state, id),
            Preventer::Card(id) => {
                let kind = state.card(id).kind;
                titled("cards", kind.entry(), kind.name())
            }
            Preventer::Affliction(id) => state.card(id).affliction.map_or_else(String::new, |a| {
                titled("afflictions", a.kind.id(), a.kind.name())
            }),
            Preventer::Enchantment(id) => {
                state.card(id).enchantment.map_or_else(String::new, |e| {
                    titled("enchantments", e.kind.id(), e.kind.name())
                })
            }
        }
    }

    /// A `StringVar`'s text: a model's localized title (its id without the text).
    fn var_text(&self, text: VarText) -> String {
        let (table, key) = match text {
            VarText::Literal(s) => return s.to_owned(),
            // `GetRawText`: the text as written, placeholders and all.
            VarText::Event(key) => {
                return self
                    .text("events", key)
                    .map_or_else(|| key.to_owned(), str::to_owned);
            }
            VarText::RelicDescription(id) => {
                return self.relic(id).unwrap_or_else(|| names::relic(id));
            }
            VarText::Character(field) => return field.to_owned(),
            VarText::Card(id) => ("cards", format!("{id}.title")),
            VarText::Relic(id) => ("relics", format!("{id}.title")),
            VarText::Potion(id) => ("potions", format!("{id}.title")),
            VarText::Enchantment(id) => ("enchantments", format!("{id}.title")),
            VarText::Affliction(id) => ("afflictions", format!("{id}.title")),
            VarText::Dish(id) => ("events", format!("ENDLESS_CONVEYOR.DISHES.{id}.title")),
        };
        self.text(table, &key).map_or_else(
            || {
                let id = key.trim_end_matches(".title");
                names::title_case(id.rsplit('.').next().unwrap_or(id))
            },
            str::to_owned,
        )
    }
}

/// The first sentence of plain `text` (up to its first full stop or line break), without the
/// full stop: a card's or a potion's effect on one row.
#[must_use]
pub fn first_sentence(text: &str) -> String {
    let line = text.lines().next().unwrap_or_default().trim();
    let end = line
        .match_indices(". ")
        .next()
        .map_or(line.len(), |(i, _)| i);
    line[..end].trim_end_matches('.').to_owned()
}

/// The keywords a card's text lists: its live keywords, with Sly and Retain also from a
/// single-turn grant (`IsSlyThisTurn`, `ShouldRetainThisTurn`).
fn card_text_keywords(state: &CombatState, id: CardId) -> Keywords {
    let mut keywords = state.keywords(id);
    if state.is_sly_this_turn(id) {
        keywords.insert(CardKeyword::Sly);
    }
    if state.should_retain_this_turn(id) {
        keywords.insert(CardKeyword::Retain);
    }
    keywords
}

/// A card of the fight in plain words without the game's text: its keywords and its preview
/// vars as the engine computes them (`Damage 6 · Block 5`), aimed at `target`.
#[must_use]
pub fn vars_text(state: &CombatState, id: CardId, target: Option<CreatureId>) -> String {
    let keywords = state.keywords(id);
    let mut parts: Vec<String> = KEYWORDS
        .iter()
        .filter(|(k, _)| keywords.contains(*k))
        .map(|(_, entry)| names::title_case(entry))
        .collect();
    parts.extend(
        state
            .card_preview(id, target, PreviewMode::Normal)
            .into_iter()
            .map(|(var, value)| {
                let name = var.to_string();
                let name = name.strip_suffix("Power").unwrap_or(&name).to_owned();
                format!("{name} {}", value.to_int_truncating())
            }),
    );
    parts.join(" · ")
}

/// What an event's text is filled from: its vars (`EventRoom::vars`) and the player's
/// character.
#[derive(Clone, Copy, Debug, Default)]
pub struct EventText<'a> {
    pub vars: &'a [(&'static str, EventVar)],
    pub character: Option<Character>,
    /// Text vars of one option, filled from the localization (`TheFutureOfPotions`'s
    /// `Potion`, `Rarity` and `Type`); they win over `vars`.
    pub extra: &'a [(&'static str, String)],
}

/// The variables every card, relic and power description gets (`RelicModel.DynamicDescription`,
/// `CardModel.cs:1389`, `PowerModel.cs:546`): the star icon and the energy icons' color prefix.
fn global(name: &str) -> Option<Value> {
    match name {
        "singleStarIcon" => Some(Value::Name("★".to_owned())),
        "energyPrefix" => Some(Value::Name(String::new())),
        _ => None,
    }
}

/// The counts `CardSelectorPrefs` adds to its prompt: `Amount` and `MaxCount` (the most picks)
/// and `MinCount`.
fn count_var(name: &str, min: usize, max: usize) -> Option<Value> {
    let count = |n: usize| Value::Int(i64::try_from(n).unwrap_or(i64::MAX));
    match name {
        "Amount" | "MaxCount" => Some(count(max)),
        "MinCount" => Some(count(min)),
        _ => None,
    }
}

/// The texts between `[gold]` and `[/gold]` in `markup`.
fn gold_words(markup: &str) -> Vec<&str> {
    let mut words = Vec::new();
    let mut rest = markup;
    while let Some(open) = rest.find("[gold]") {
        let after = &rest[open + "[gold]".len()..];
        let Some(close) = after.find("[/gold]") else {
            break;
        };
        words.push(&after[..close]);
        rest = &after[close..];
    }
    words
}

/// Vars named as owned strings.
fn named(vars: Vec<(&'static str, Decimal)>) -> Vec<(String, Decimal)> {
    vars.into_iter().map(|(n, v)| (n.to_owned(), v)).collect()
}

/// A var's value for a template: an int, or hundredths for a fraction (`1.25`).
fn decimal_value(value: Decimal) -> Value {
    let int = value.to_int_truncating();
    if Decimal::from(int) == value {
        Value::Int(i64::from(int))
    } else {
        Value::Hundredths(i64::from((value * Decimal::from(100)).to_int_truncating()))
    }
}

/// A card of the fight's preview vars by name ([`CombatState::card_preview`] at `target`),
/// each with its `EnchantedValue` for `diff()`: the card's own values with only its
/// enchantment's changes ([`Card::preview_out_of_combat`], `DynamicVar.EnchantedValue`).
///
/// [`Card::preview_out_of_combat`]: sts2_core::combat::state::Card::preview_out_of_combat
fn combat_card_vars(state: &CombatState, id: CardId, target: Option<CreatureId>) -> CardVars {
    let preview = state.card_preview(id, target, PreviewMode::Normal);
    let bases = state.card(id).preview_out_of_combat();
    preview
        .into_iter()
        .zip(bases)
        .map(|((var, value), (_, base))| {
            let value = i64::from(value.to_int_truncating());
            let base = i64::from(base.to_int_truncating());
            (var.to_string(), Value::Compared { value, base })
        })
        .collect()
}

/// Preview values by name, truncated as the text shows them (`IntValue`).
fn to_card_vars(vars: Vec<(Var, Decimal)>) -> CardVars {
    vars.into_iter()
        .map(|(var, value)| {
            (
                var.to_string(),
                Value::Int(value.to_int_truncating().into()),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_sentence_is_kept() {
        assert_eq!(
            first_sentence("Deal 10 damage. Apply 2 Vulnerable."),
            "Deal 10 damage"
        );
        assert_eq!(first_sentence("Gain 5 Block.\nExhaust."), "Gain 5 Block");
        assert_eq!(first_sentence(""), "");
    }
    use sts2_core::Character;
    use sts2_core::content::cards::CardKind;
    use sts2_core::content::enchantments::EnchantmentKind;
    use sts2_core::run::UnlockState;

    fn describer() -> Describer {
        let loc = Localization::from_json(&[
            (
                "cards",
                r#"{"STRIKE_NECROBINDER.description": "Hit for {Damage:diff()}{IfUpgraded:show: (up)}.",
                    "DEFEND_NECROBINDER.description": "Block {Block:diff()}; {Missing:diff()}"}"#,
            ),
            (
                "powers",
                r#"{"VULNERABLE_POWER.smartDescription": "{OwnerName} takes more for {Amount} {Amount:plural:turn|turns}.",
                    "PLATING_POWER.smartDescription": "Gain {Amount} Block, down by {Decrement}.",
                    "WEAK_POWER.smartDescription": "Deals {DamageDecrease:percentLess()}% less.",
                    "POISON_POWER.smartDescription": "At the start of {OnPlayer:your turn, lose|its turn, loses} {Amount}.",
                    "SLOW_POWER.smartDescription": "Slow {DisplayAmount}%.",
                    "BUFFER_POWER.smartDescription": "Buffer {Unknown}.",
                    "BUFFER_POWER.description": "Buffer for good."}"#,
            ),
            (
                "relics",
                r#"{"BOUND_PHYLACTERY.description": "Relic text."}"#,
            ),
            (
                "events",
                r#"{"E.title": "Event", "E.pages.A.description": "Body.",
                    "E.pages.A.options.GO.title": "Go {Rarity}", "E.pages.A.options.GO.description": "Gain gold.",
                    "E.pages.A.options.NO.title": "No", "E.pages.A.options.NO.description": ""}"#,
            ),
        ]);
        Describer::new(Rc::new(loc))
    }

    #[test]
    fn deck_cards_get_their_var_values() {
        let d = describer();
        let strike = RunCard::new("STRIKE_NECROBINDER");
        assert_eq!(
            d.run_card(strike).map(|m| text::plain(&m)).as_deref(),
            Some("Hit for 6.")
        );
        let mut up = strike;
        up.upgrade_level = 1;
        assert_eq!(
            d.run_card(up).map(|m| text::plain(&m)).as_deref(),
            Some("Hit for 9 (up).")
        );
        // A name the card has no var for stays visible.
        assert_eq!(
            d.run_card(RunCard::new("DEFEND_NECROBINDER"))
                .map(|m| text::plain(&m))
                .as_deref(),
            Some("Block 5; ‹Missing›")
        );
        assert_eq!(
            d.run_card(RunCard::new("BODYGUARD")),
            None,
            "no text for it"
        );
    }

    /// A describer with the keyword titles and a few cards' descriptions.
    fn keyword_describer() -> Describer {
        let loc = Localization::from_json(&[
            (
                "cards",
                r#"{"DEFILE.description": "Deal {Damage:diff()} damage.",
                    "ADRENALINE.description": "Gain energy.",
                    "BACKSTAB.description": "Deal {Damage:diff()} damage.",
                    "SUPPRESS.description": ""}"#,
            ),
            (
                "card_keywords",
                r#"{"ETHEREAL.title": "Ethereal", "EXHAUST.title": "Exhaust",
                    "INNATE.title": "Innate", "PERIOD": "."}"#,
            ),
        ]);
        Describer::new(Rc::new(loc))
    }

    #[test]
    fn keyword_lines_follow_the_games_order() {
        let d = keyword_describer();
        let plain = |id: &'static str| {
            d.run_card(RunCard::new(id))
                .map(|m| text::plain(&m))
                .expect("text")
        };
        assert_eq!(plain("DEFILE"), "Ethereal.\nDeal 13 damage.");
        assert_eq!(plain("ADRENALINE"), "Gain energy.\nExhaust.");
        assert_eq!(plain("BACKSTAB"), "Innate.\nDeal 11 damage.\nExhaust.");
        let gold = d.run_card(RunCard::new("DEFILE")).expect("text");
        assert!(gold.starts_with("[gold]Ethereal[/gold]."), "{gold}");
    }

    #[test]
    fn every_flagged_card_shows_its_keyword_lines() {
        let dir = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../referee/vendor/upstream/localization_eng"
        );
        let read = |t: &str| std::fs::read_to_string(format!("{dir}/{t}.json")).ok();
        let (Some(cards), Some(keywords)) = (read("cards"), read("card_keywords")) else {
            return; // no vendored text in this checkout
        };
        let d = Describer::new(Rc::new(Localization::from_json(&[
            ("cards", &cards),
            ("card_keywords", &keywords),
        ])));
        let tsv = include_str!("../../sts2-core/src/content/flags/cards.tsv");
        let mut checked = 0;
        for row in tsv
            .lines()
            .skip(1)
            .map(|l| l.split('\t').collect::<Vec<_>>())
        {
            let card = RunCard::new(Box::leak(row[0].to_owned().into_boxed_str()));
            let (Some(text), Some(local)) = (d.run_card(card), card.card()) else {
                continue;
            };
            let text = text::plain(&text);
            for (k, entry) in KEYWORDS {
                let line = format!("{}.", names::title_case(entry));
                assert_eq!(
                    text.lines().any(|l| l == line),
                    local.local_keywords().contains(k),
                    "{}: {text}",
                    row[0]
                );
            }
            checked += 1;
        }
        assert!(checked > 100, "{checked}");
    }

    /// A run at its first fight.
    fn first_fight() -> Run {
        fight_of(Character::Necrobinder, "RL2S2")
    }

    /// A run of `character` on `seed` at its first fight.
    fn fight_of(character: Character, seed: &str) -> Run {
        let mut run = Run::new(
            character,
            seed,
            0,
            UnlockState::all(),
            sts2_core::run::Act1::Overgrowth,
        )
        .expect("run")
        .with_events();
        for _ in 0..40 {
            if run.combat().is_some() {
                break;
            }
            let action = run
                .legal_actions()
                .into_iter()
                .find(|a| run.clone().apply(a.clone()).is_ok())
                .expect("a choice");
            run.apply(action).expect("legal");
        }
        assert!(run.combat().is_some(), "a fight within 40 actions");
        run
    }

    #[test]
    fn hand_cards_show_the_preview() {
        let mut d = describer();
        let run = first_fight();
        d.refresh(&run);
        let combat = run.combat().expect("a fight");
        let strike = combat
            .state()
            .player
            .hand
            .iter()
            .copied()
            .find(|&c| combat.state().card(c).kind == CardKind::StrikeNecrobinder)
            .expect("a Strike in the opening hand");
        assert_eq!(
            d.hand_card(combat.state(), strike)
                .map(|m| text::plain(&m))
                .as_deref(),
            Some("Hit for 6.")
        );
    }

    /// The first card of `kind` in the hand of `run`'s fight.
    fn in_hand(run: &Run, kind: CardKind) -> Option<CardId> {
        let state = run.combat()?.state();
        state
            .player
            .hand
            .iter()
            .copied()
            .find(|&c| state.card(c).kind == kind)
    }

    #[test]
    fn aimed_cards_show_their_numbers_against_the_target_coloured_like_the_game() {
        let loc = Localization::from_json(&[(
            "cards",
            r#"{"STRIKE_IRONCLAD.description": "Deal {Damage:diff()} damage."}"#,
        )]);
        let d = Describer::new(Rc::new(loc));
        // An Ironclad opening hand with Bash and a Strike: Bash makes the enemy Vulnerable.
        let mut run = (0..40)
            .map(|i| fight_of(Character::Ironclad, &format!("UTWELVE{i}")))
            .find(|run| {
                in_hand(run, CardKind::Bash).is_some()
                    && in_hand(run, CardKind::StrikeIronclad).is_some()
            })
            .expect("a seed with Bash and a Strike in the opening hand");
        let enemy = run.combat().expect("a fight").state().enemies[0];
        let bash = in_hand(&run, CardKind::Bash).expect("Bash");
        run.apply(sts2_core::run::RunAction::Combat(
            sts2_core::Action::PlayCard {
                card: bash,
                target: Some(enemy),
            },
        ))
        .expect("Bash is playable");
        let state = run.combat().expect("a fight").state();
        let strike = in_hand(&run, CardKind::StrikeIronclad).expect("a Strike");
        let aimed = d.hand_card_text(state, strike, Some(enemy));
        assert_eq!(aimed.as_deref(), Some("Deal [green]9[/green] damage."));
        assert_eq!(
            d.hand_card_text(state, strike, None).as_deref(),
            Some("Deal 6 damage.")
        );
    }

    #[test]
    fn the_glossary_lists_the_keywords_then_the_gold_terms_once() {
        let loc = Localization::from_json(&[
            (
                "card_keywords",
                r#"{"EXHAUST.title": "Exhaust", "EXHAUST.description": "Removed until the end of combat.",
                    "ETHEREAL.title": "Ethereal", "ETHEREAL.description": "Gone at turn end."}"#,
            ),
            (
                "powers",
                r#"{"VULNERABLE_POWER.title": "Vulnerable", "VULNERABLE_POWER.description": "Takes more."}"#,
            ),
            (
                "static_hover_tips",
                r#"{"BLOCK.title": "Block", "BLOCK.description": "Stops damage."}"#,
            ),
        ]);
        let d = Describer::new(Rc::new(loc));
        let text = "Apply 2 [gold]Vulnerable[/gold]. Gain [gold]Block[/gold]. \
                    [gold]Exhausted[/gold], [gold]Vulnerable[/gold], [gold]Nothing[/gold].";
        let terms = d.glossary(text, Keywords::NONE.with(CardKeyword::Exhaust));
        let titles: Vec<&str> = terms.iter().map(|t| t.title.as_str()).collect();
        assert_eq!(titles, ["Exhaust", "Vulnerable", "Block"]);
        assert_eq!(terms[2].text, "Stops damage.");
        assert_eq!(d.glossary("plain", Keywords::NONE), Vec::new());
    }

    #[test]
    fn unplayable_cards_say_why_in_the_games_words() {
        let loc = Localization::from_json(&[(
            "combat_messages",
            r#"{"NOT_ENOUGH_ENERGY": "I don't have\nenough [red]Energy[/red].",
                "BLOCKED_BY_HOOK": "I'm blocked\nby [red]{BlockingHook}[/red]."}"#,
        )]);
        let d = Describer::new(Rc::new(loc));
        let run = first_fight();
        let mut state = run.combat().expect("a fight").state().clone();
        let strike = in_hand(&run, CardKind::StrikeNecrobinder).expect("a Strike");
        assert_eq!(state.unplayable_reason(strike), None);
        state.player.energy = 0;
        let reason = state.unplayable_reason(strike).expect("no energy");
        assert_eq!(reason, UnplayableReason::NotEnoughEnergy);
        assert_eq!(
            text::plain(&d.unplayable(&state, reason)),
            "I don't have enough Energy."
        );
        let blocked = UnplayableReason::BlockedBy(Preventer::Power(PowerKind::Vulnerable));
        assert_eq!(
            text::plain(&d.unplayable(&state, blocked)),
            "I'm blocked by Vulnerable."
        );
    }

    #[test]
    fn run_selection_prompts_use_the_shared_or_the_relics_own_text() {
        let loc = Localization::from_json(&[
            (
                "card_selection",
                r#"{"TO_REMOVE": "Choose {Amount:plural:a card|[blue]{}[/blue] cards} to [gold]Remove[/gold]."}"#,
            ),
            (
                "relics",
                r#"{"CLAWS.selectionScreenPrompt": "Select cards to Transform into Maul."}"#,
            ),
        ]);
        let d = Describer::new(Rc::new(loc));
        let plain = |p| d.select_prompt(p, 0, 2).map(|m| text::plain(&m));
        assert_eq!(
            plain(SelectPrompt::Shared(PromptText::Remove)).as_deref(),
            Some("Choose 2 cards to Remove.")
        );
        assert_eq!(
            plain(SelectPrompt::Relic("CLAWS")).as_deref(),
            Some("Select cards to Transform into Maul.")
        );
        assert_eq!(plain(SelectPrompt::Relic("SEA_GLASS")), None);
        assert_eq!(d.relic_title("SEA_GLASS"), "Sea Glass");
    }

    #[test]
    fn prompts_name_their_source_and_fill_in_counts_and_vars() {
        let loc = Localization::from_json(&[
            (
                "cards",
                r#"{"STRIKE_NECROBINDER.title": "Strike",
                    "STRIKE_NECROBINDER.selectionScreenPrompt": "Pick {Amount} for {Damage}."}"#,
            ),
            (
                "card_selection",
                r#"{"TO_DISCARD": "Choose {Amount:plural:a card|[blue]{}[/blue] cards} to [gold]Discard[/gold]."}"#,
            ),
            ("gameplay_ui", r#"{"CHOOSE_CARD_HEADER": "Choose a Card"}"#),
        ]);
        let d = Describer::new(Rc::new(loc));
        let run = first_fight();
        let state = run.combat().expect("a fight").state();
        let strike = state
            .player
            .hand
            .iter()
            .copied()
            .find(|&c| state.card(c).kind == CardKind::StrikeNecrobinder)
            .expect("a Strike in the opening hand");
        let plain = |m: Option<String>| m.map(|m| text::plain(&m));
        let own = Prompt::card(strike, PromptText::Own);
        assert_eq!(d.prompt_source(state, own.source), "Strike");
        assert_eq!(
            plain(d.prompt(state, own, 2, 2)).as_deref(),
            Some("Pick 2 for 6.")
        );
        let discard = Prompt::card(strike, PromptText::Discard);
        assert_eq!(
            plain(d.prompt(state, discard, 1, 1)).as_deref(),
            Some("Choose a card to Discard.")
        );
        assert_eq!(
            plain(d.prompt(state, discard, 3, 3)).as_deref(),
            Some("Choose 3 cards to Discard.")
        );
        let banner = Prompt::card(strike, PromptText::ChooseACard);
        assert_eq!(
            plain(d.prompt(state, banner, 0, 1)).as_deref(),
            Some("Choose a Card")
        );
        // Without the text: no prompt, the source by its engine name.
        let bare = Describer::new(Rc::new(Localization::empty()));
        assert_eq!(bare.prompt(state, own, 1, 1), None);
        assert_eq!(bare.prompt_source(state, own.source), "Strike");
    }

    #[test]
    fn enchantments_fill_in_their_own_vars() {
        let loc = Localization::from_json(&[(
            "enchantments",
            r#"{"ADROIT.title": "Adroit", "ADROIT.description": "Gain {Block:diff()} Block.",
                "TEZCATARAS_EMBER.description": "Deals {Damage} more."}"#,
        )]);
        let d = Describer::new(Rc::new(loc));
        let enchanted = |kind, amount| {
            let mut card = RunCard::new("DEFEND_NECROBINDER");
            card.enchantment = Some(Enchantment::new(kind, amount));
            card
        };
        let adroit = d.run_card(enchanted(EnchantmentKind::Adroit, 3));
        assert_eq!(
            adroit.map(|m| text::plain(&m)).as_deref(),
            Some("✦ Adroit: Gain 3 Block.")
        );
        // `DamageVar(3)` through the card's own enchantment, as the game shows it.
        let ember = d.run_card(enchanted(EnchantmentKind::TezcatarasEmber, 1));
        assert!(
            ember.is_some_and(|m| text::plain(&m).ends_with("Deals 6 more.")),
            "Ember's damage"
        );
    }

    #[test]
    fn powers_relics_and_events_use_their_tables() {
        let d = describer();
        assert_eq!(
            d.power(
                PowerKind::Vulnerable,
                2,
                &PowerData::None,
                "Jaw Worm",
                false
            )
            .map(|m| text::plain(&m))
            .as_deref(),
            Some("Jaw Worm takes more for 2 turns.")
        );
        assert_eq!(d.relic("BOUND_PHYLACTERY").as_deref(), Some("Relic text."));
        assert_eq!(d.relic("ANCHOR"), None);
        assert_eq!(d.event_title("E").as_deref(), Some("Event"));
        let key = "E.pages.A.options.GO";
        let none = EventText::default();
        assert_eq!(d.event_body(key, &none).as_deref(), Some("Body."));
        assert_eq!(
            d.event_option(key, &none),
            Some(("Go ‹Rarity›".to_owned(), Some("Gain gold.".to_owned())))
        );
        let vars = [("Rarity", EventVar::Text(VarText::Literal("Rare")))];
        let rare = EventText {
            vars: &vars,
            character: None,
            extra: &[],
        };
        assert_eq!(
            d.event_option(key, &rare).map(|o| o.0),
            Some("Go Rare".to_owned())
        );
        assert_eq!(
            d.event_option("E.pages.A.options.NO", &none),
            Some(("No".to_owned(), None))
        );
    }

    #[test]
    fn powers_fill_in_their_vars_and_whose_they_are() {
        let d = describer();
        let plain = |m: Option<String>| m.map(|m| text::plain(&m));
        let none = &PowerData::None;
        assert_eq!(
            plain(d.power(PowerKind::Plating, 3, none, "Clam", false)).as_deref(),
            Some("Gain 3 Block, down by 1.")
        );
        // `DynamicVar("DamageDecrease", 0.75m)` with `percentLess`.
        assert_eq!(
            plain(d.power(PowerKind::Weak, 1, none, "Clam", false)).as_deref(),
            Some("Deals 25% less.")
        );
        assert_eq!(
            plain(d.power(PowerKind::Poison, 4, none, "Me", true)).as_deref(),
            Some("At the start of your turn, lose 4.")
        );
        assert_eq!(
            plain(d.power(PowerKind::Poison, 4, none, "Louse", false)).as_deref(),
            Some("At the start of its turn, loses 4.")
        );
        // `DisplayAmount`, canonically 0 (the engine's test covers a played card).
        assert_eq!(
            plain(d.power(PowerKind::Slow, 1, none, "x", false)).as_deref(),
            Some("Slow 0%.")
        );
        // A number nobody knows: the static description instead of a `‹Unknown›`.
        assert_eq!(
            plain(d.power(PowerKind::Buffer, 1, none, "x", false)).as_deref(),
            Some("Buffer for good.")
        );
    }

    #[test]
    fn relics_and_potions_fill_in_their_vars() {
        let loc = Localization::from_json(&[
            (
                "relics",
                r#"{"ANCHOR.description": "Gain {Block} Block.",
                    "BOWLER_HAT.description": "Gain {GoldIncrease:percentMore()}% more Gold."}"#,
            ),
            (
                "potions",
                r#"{"FIRE_POTION.description": "Deal {Damage} damage."}"#,
            ),
        ]);
        let d = Describer::new(Rc::new(loc));
        assert_eq!(d.relic("ANCHOR").as_deref(), Some("Gain 10 Block."));
        assert_eq!(
            d.relic("BOWLER_HAT").as_deref(),
            Some("Gain 25% more Gold.")
        );
        assert_eq!(d.potion("FIRE_POTION").as_deref(), Some("Deal 20 damage."));
    }

    #[test]
    fn without_text_everything_is_none() {
        let d = Describer::new(Rc::new(Localization::empty()));
        assert!(!d.has_text());
        assert_eq!(d.run_card(RunCard::new("STRIKE_NECROBINDER")), None);
        assert_eq!(
            d.power(PowerKind::Vulnerable, 1, &PowerData::None, "x", false),
            None
        );
        assert_eq!(
            d.event_option("E.pages.A.options.GO", &EventText::default()),
            None
        );
    }
}
