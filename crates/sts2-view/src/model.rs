//! What every human client draws: a [`Screen`] of [`Block`]s and [`Button`]s, and the [`Input`]
//! a renderer sends back.
//!
//! The model has no terminal or game rules in it; `sts2-core` decides what is legal and what a
//! number is, a renderer decides where a block goes. The TUI draws it with ratatui; the iOS app
//! decodes the JSON form into `SwiftUI`. A button's `id` is stable for one screen, so a tap is
//! just `Input::Tap(id)` in either client.

use serde::{Deserialize, Serialize};

use crate::text::Rich;

/// One screen.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Screen {
    /// HP, gold, floor, stars and the belt; the same on every screen.
    pub top: Option<TopBar>,
    /// The screen's heading.
    pub title: Option<Rich>,
    /// A line at the right of the title (`Act 1 · Floor 7` on the run's end).
    pub aside: Option<Rich>,
    /// The content, in reading order.
    pub blocks: Vec<Block>,
    /// End Turn, Proceed, Start, Continue: the one action the screen is really asking for.
    pub primary: Option<Button>,
    /// A detail, choice or menu over the screen.
    pub sheet: Option<Sheet>,
    /// A transient line: "Not enough energy", "Saved", an error.
    pub status: Option<Rich>,
}

/// The bar above every screen.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TopBar {
    pub hp: i32,
    pub max_hp: i32,
    pub gold: i32,
    pub floor: u32,
    /// Regent stars; `None` for a character without them.
    pub stars: Option<i32>,
    /// A fight is on (an empty belt then says so).
    pub fight: bool,
    /// The belt, in slot order.
    pub potions: Vec<PotionSlot>,
    /// The relics, in the order they were got.
    #[serde(default)]
    pub relics: Vec<RelicChip>,
}

/// One owned relic, for a client that lists them.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelicChip {
    pub name: String,
    /// The game's id for its picture (`ModelId.Entry`, as the art index names it: a client's
    /// own lookup of the player's game art); empty when unknown.
    #[serde(default)]
    pub art: String,
    /// Its counter (`Pen Nib`'s attacks), when the game shows one.
    pub counter: Option<i32>,
    /// Melts when used up (`ToyBox`'s).
    pub wax: bool,
    /// What its icon shows (`RelicModel.Status`): `Active` pulses (it is about to trigger, or
    /// its bonus is on), `Disabled` is used up; `Normal` once melted.
    #[serde(default)]
    pub status: RelicLook,
    /// The relic's text, its fields now.
    pub detail: Vec<Rich>,
}

/// A relic icon's state (`RelicStatus`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RelicLook {
    #[default]
    Normal,
    Active,
    Disabled,
}

/// One belt slot.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "the slot's independent facts, serialised flat for the clients"
)]
pub struct PotionSlot {
    /// The key that selects it, when the client prints one.
    pub key: Option<char>,
    /// The game's id for its picture (`ModelId.Entry`, as the art index names it: a client's
    /// own lookup of the player's game art); empty when unknown.
    #[serde(default)]
    pub art: String,
    pub name: String,
    pub empty: bool,
    /// The row's button id.
    pub id: ButtonId,
    /// The engine lets the potion be used now.
    pub usable: bool,
    /// Selected, waiting for its confirmation or its target.
    pub selected: bool,
    /// Selected and asking whether to use it or throw it away (rather than for a target).
    pub asks: bool,
    /// The potion's text.
    pub detail: Vec<Rich>,
}

/// A piece of a screen, in reading order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Block {
    /// Story, a description, the last-turn line.
    Text(Vec<Rich>),
    /// One enemy, as one row (or one card on a wide screen).
    Enemy(Box<EnemyRow>),
    /// What the enemies' turn does to the player.
    Incoming(IncomingRow),
    /// What the last turn or play did, in one line.
    Last(Rich),
    /// The player.
    You(YouRow),
    /// Osty or another pet.
    Pet(Box<PetRow>),
    /// The Defect's orbs.
    Orbs(OrbRow),
    /// A framed list of rows, with its page controls when it is longer than fits.
    List {
        frame: bool,
        rows: Vec<Button>,
        page: Option<Page>,
    },
    /// The act map.
    Map(MapGrid),
    /// A horizontal rule.
    Rule,
}

/// One enemy.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnemyRow {
    /// The engine's creature id (a number in the combat dump), not a display name.
    pub id: u32,
    /// The game's id for its picture (`ModelId.Entry`, as the art index names it: a client's
    /// own lookup of the player's game art); empty when unknown.
    #[serde(default)]
    pub art: String,
    pub name: String,
    pub hp: i32,
    pub max_hp: i32,
    pub block: i32,
    pub alive: bool,
    /// Down in a dead state that revives it (Test Subject's `RESPAWN_MOVE`), not killed.
    #[serde(default)]
    pub dead_state: bool,
    /// The next move, in words.
    pub intent: Option<Rich>,
    /// The same with the damage spelled out (`6×3 damage`), for a screen with room.
    pub intent_long: Option<Rich>,
    pub powers: Vec<PowerRow>,
    /// Left the fight alive (`CreatureCmd.Escape`): it is no longer in play, `alive` is false.
    #[serde(default)]
    pub escaped: bool,
    /// A deck card it stole (`SwipePower.StolenCard`): held while it lives; after it escaped,
    /// the card the run lost. `None` when it took nothing.
    #[serde(default)]
    pub stolen: Option<CardFace>,
    /// What the card waiting (or in focus) does to it; `None` with no card selected.
    pub preview: Option<Preview>,
    /// The button that aims the waiting card or potion at it; `None` when it cannot be
    /// aimed at now.
    pub target: Option<Button>,
}

/// What one play of the selected card does to one creature.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Preview {
    /// What the engine says certainly happens to it.
    Certain(Hit),
    /// What certainly happens before the play draws or shuffles; what follows is unknown.
    BeforeDraw(Hit),
    /// The play leaves it as it is.
    Unchanged,
    /// The play draws, shuffles or rolls: the engine cannot say.
    Unknown,
}

/// A creature's HP and Block before and after a play.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hit {
    pub hp_before: i32,
    pub hp_after: i32,
    pub block_before: i32,
    pub block_after: i32,
    pub dies: bool,
}

/// What the enemies' next turn does to the player, from the engine's preview.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IncomingRow {
    pub total: i32,
    pub blocked: i32,
    /// Taken by Osty or another pet.
    pub absorbed: i32,
    pub hp_loss: i32,
    pub hp_after: i32,
    pub lethal: bool,
    /// The engine is sure of the numbers.
    pub certain: bool,
    /// Kept from before the turn's last play (a card choice has no preview of its own).
    pub stale: bool,
}

/// The player's row.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct YouRow {
    /// The engine's creature id.
    pub id: u32,
    pub name: String,
    pub hp: i32,
    pub max_hp: i32,
    pub block: i32,
    pub energy: i32,
    pub max_energy: i32,
    pub powers: Vec<PowerRow>,
    /// Stars, orbs, or the character's other resource, already worded.
    pub resources: Vec<Rich>,
    /// Regent stars, for a character that has them.
    pub stars: Option<i32>,
    /// Cards in the draw, discard and exhaust piles.
    pub draw: usize,
    pub discard: usize,
    pub exhaust: usize,
    /// The character has a pet's row even while the pet is dead or gone (Osty).
    pub pet_slot: bool,
    /// Orb slots, filled or not (the Defect).
    pub orb_slots: usize,
    /// The Sovereign Blades in any pile (the Regent).
    pub blades: Vec<BladeRow>,
}

/// A Sovereign Blade card, wherever it is.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BladeRow {
    /// The engine's card id.
    pub card: u32,
    pub damage: i32,
    pub repeats: i32,
    /// `hand`, `draw`, `discard`, `exhaust` or `play`.
    pub place: String,
}

/// A pet (Osty) or a summoned ally.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PetRow {
    pub id: u32,
    pub name: String,
    pub hp: i32,
    pub max_hp: i32,
    pub block: i32,
    pub alive: bool,
    pub powers: Vec<PowerRow>,
    /// The button that aims the waiting card or potion at it; `None` when it cannot be
    /// aimed at now.
    pub target: Option<Button>,
}

/// One orb on the player's row.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrbRow {
    pub name: String,
    pub passive: i32,
    pub evoke: i32,
}

/// A power with its amount and its sheet text.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PowerRow {
    pub name: String,
    pub amount: i32,
    /// A debuff, for the tag's colour.
    pub debuff: bool,
    /// The game shows it (some powers are bookkeeping only).
    pub shown: bool,
    pub detail: Vec<Rich>,
    /// The game's id for its picture (`ModelId.Entry`, as the art index names it: a client's
    /// own lookup of the player's game art); empty when unknown.
    #[serde(default)]
    pub art: String,
    /// Poison only: the damage its owner takes at the next turn start (`PoisonPower.
    /// CalculateTotalDamageNextTurn`), what the HP bar's poison part covers; 0 on other powers.
    #[serde(default)]
    pub next: i32,
}

/// A row's key: one, two or more keys pressed in sequence (a digit then a target).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Key {
    Digit(u8),
    Char(char),
    Enter,
    Esc,
    Backspace,
    Up,
    Down,
    Left,
    Right,
    PageUp,
    PageDown,
    Space,
    Tab,
}

/// A button's stable identity on one screen.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct ButtonId(pub u32);

/// What a renderer sends.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Input {
    /// A tap/click on a button.
    Tap(ButtonId),
    /// Open the detail sheet of a button (`i` + its number, a right click, the `›`).
    Info(ButtonId),
    /// A key the screen did not put on a row (a global: Esc, a page, a menu).
    Key(Key),
    /// A page step.
    Page(i32),
    /// A drag starts on a card: it is selected and waits for a drop.
    Arm(ButtonId),
    /// The dragged card is dropped on the battlefield (a drop on an enemy is a `Tap` on it).
    Drop,
    /// Back/close.
    Back,
    /// The screen's main button.
    Primary,
}

/// A row.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Button {
    pub id: ButtonId,
    /// What the row prints at the left; `None` for a row with no key.
    pub key: Option<Key>,
    pub label: Rich,
    /// The full game text the detail sheet shows.
    pub detail: Vec<Rich>,
    /// A number or price at the right (a card's cost, a price, a count).
    pub right: Option<Rich>,
    pub state: ButtonState,
    /// The `›` target: what the detail sheet describes.
    pub info: Option<InfoRef>,
    /// "Tap again to confirm", the second-tap text.
    pub confirm: Option<Rich>,
    /// A card row's parts, for a client that lays a card out itself.
    pub card: Option<CardFace>,
}

/// A card as a row shows it, in parts.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CardFace {
    /// The engine's card id.
    pub card: u32,
    /// The game's id for its picture (`ModelId.Entry`, as the art index names it: a client's
    /// own lookup of the player's game art); empty when unknown.
    #[serde(default)]
    pub art: String,
    /// `1`, `X`, `—`, with `·s2` for a star cost.
    pub cost: String,
    pub name: String,
    /// `Attack`, `Skill`, ...
    pub kind: String,
    /// `Starter`, `Common`, `Uncommon`, `Rare`, `Ancient`, `Special`, `Status`, `Curse`; empty
    /// when unknown. A client frames the card by it.
    pub rarity: String,
    /// The frame's colour, as the game's frame material names it: `red`, `green`, `blue`,
    /// `orange`, `pink` (the character pools), `colorless`, `curse`, `quest`.
    #[serde(default)]
    pub frame: String,
    /// The card's whole text on one line, the numbers a power changed lit.
    pub effect: Rich,
    /// The card's text in full.
    pub text: Rich,
    /// In the hand: its gold glow is on (`ShouldGlowGold`: a condition of the card holds).
    #[serde(default)]
    pub glow: bool,
    /// In a fight: the card is played at a creature the player picks (`AnyEnemy`, `AnyPlayer`),
    /// even when only one can be picked (the game still draws its targeting arrow).
    #[serde(default)]
    pub aims: bool,
    /// In an upgrade selection: this face is the card upgraded (its changes marked
    /// [`Role::Upgrade`](crate::text::Role::Upgrade)) and this is the card as it is now.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before: Option<Box<Self>>,
    /// In the deck sheet: the card upgraded (its changes marked [`Role::Upgrade`](crate::text::Role::Upgrade)),
    /// for a client's View Upgrades; none when the card cannot be upgraded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upgrade: Option<Box<Self>>,
}

impl Button {
    /// A plain enabled row with an id and a label.
    #[must_use]
    pub const fn row(id: u32, label: Rich) -> Self {
        Self {
            id: ButtonId(id),
            key: None,
            label,
            detail: Vec::new(),
            right: None,
            state: ButtonState::Enabled,
            info: None,
            confirm: None,
            card: None,
        }
    }
}

/// Whether a row can be chosen.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ButtonState {
    #[default]
    Enabled,
    /// Shown but not choosable, with the reason.
    Dim(String),
    /// The keyboard/mouse focus.
    Selected,
    /// Picked and waiting for the confirmation or the rest of the selection.
    Picked,
    /// Not legal at all.
    Disabled,
}

/// What a detail sheet describes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum InfoRef {
    Card(String),
    Relic(String),
    Potion(String),
    Power(String),
    Enemy(u32),
    Room(String),
}

/// A sheet over the screen.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Sheet {
    Detail(DetailSheet),
    Choice(ChoiceSheet),
    Menu(MenuSheet),
}

/// The full text of one object.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetailSheet {
    pub title: Rich,
    pub lines: Vec<Rich>,
    /// Use, throw away, back; usually none (the sheet closes with Back).
    pub buttons: Vec<Button>,
    /// The cards of a deck or pile sheet, one per card, for a client that lays them out as
    /// cards; empty for any other sheet.
    #[serde(default)]
    pub cards: Vec<CardFace>,
}

/// A card/target/option choice that needs its own screen.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChoiceSheet {
    pub title: Rich,
    /// What asks and what is picked so far, in one line, for a client with no room for the
    /// title's frame (`Headbutt: Choose a card…   picked 1/2: Strike`).
    pub prompt: Rich,
    pub rows: Vec<Button>,
    pub page: Option<Page>,
}

/// The run menu, deck, relics and other lists.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MenuSheet {
    pub title: Rich,
    pub rows: Vec<Button>,
    pub page: Option<Page>,
}

/// A list's page.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Page {
    /// 0-based.
    pub index: usize,
    pub count: usize,
    /// The page's rows are `rows[first..first + len]` of its list, which holds every page's
    /// (a renderer aligns the columns over all of them, so the page turns without a jump).
    pub first: usize,
    pub len: usize,
}

/// The act map: its nodes, the paths between them and what lies ahead.
///
/// `x` is the column and `y` the floor (0 at the start), the boss on floor `boss_at.1`; the
/// cells are every node below the boss. The fields past `cells` are additive and default to
/// empty for a client that draws only the compact grid.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MapGrid {
    pub width: u8,
    pub height: u8,
    pub cells: Vec<MapCell>,
    /// A path from a node to the node it leads to, the boss's included.
    #[serde(default)]
    pub edges: Vec<MapEdge>,
    /// Where the player stands (`x`, `y`); `None` before the first room.
    #[serde(default)]
    pub you: Option<(u8, u8)>,
    /// The act's boss, by name.
    #[serde(default)]
    pub boss: String,
    /// The boss encounter's id (`VANTOM_BOSS`), which names its map icon.
    #[serde(default)]
    pub boss_art: String,
    /// The act's id (`OVERGROWTH`, `UNDERDOCKS`, `HIVE`, `GLORY`), which names its map page.
    #[serde(default)]
    pub area: String,
    /// The boss's node (`x`, `y`).
    #[serde(default)]
    pub boss_at: (u8, u8),
    /// The boss's travel button when it is the next room (the boss is not one of the `cells`).
    #[serde(default)]
    pub boss_button: Option<ButtonId>,
    /// The room kinds the map is read by, in legend order, with how many of each lie ahead.
    #[serde(default)]
    pub kinds: Vec<MapKind>,
    /// The act, counted from 1; 0 when the client is not told.
    #[serde(default)]
    pub act: u8,
    /// How many of the act's easy hallway fights are still to come (the act's first ordinary
    /// fights draw from its weak encounters: 3 in most acts).
    #[serde(default)]
    pub easy_fights_left: u8,
}

/// One map node.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MapCell {
    pub x: u8,
    pub y: u8,
    /// The room's key (`Monster`, `Elite`, `Event`, ...).
    pub room: String,
    pub reachable: bool,
    pub visited: bool,
    /// The letter the room is drawn with (`M`, `E`, `?`, `$`, `T`, `R`).
    #[serde(default)]
    pub glyph: char,
    #[serde(default)]
    pub role: crate::text::Role,
    /// The node can still be reached from where the player stands (every node before the
    /// first room).
    #[serde(default)]
    pub ahead: bool,
    /// The button that travels here, while it is `reachable`.
    #[serde(default)]
    pub button: Option<ButtonId>,
    /// What the paths from the player through this node to the boss still pass, per room kind
    /// in [`MapGrid::kinds`] order (this node counted); empty off the paths ahead.
    #[serde(default)]
    pub through: Vec<MapRange>,
}

/// The fewest and the most rooms of one kind over a set of paths to the boss.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MapRange {
    pub min: u8,
    pub max: u8,
}

/// A path between two nodes (`x`, `y`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MapEdge {
    pub from: (u8, u8),
    pub to: (u8, u8),
}

/// A kind of room in the map's legend.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MapKind {
    /// The same key as [`MapCell::room`].
    pub room: String,
    pub glyph: char,
    /// `monster`, `elite`, `unknown`, `shop`, `treasure`, `rest`.
    pub word: String,
    pub role: crate::text::Role,
    /// How many of them lie ahead of the player.
    pub ahead: u32,
    /// The fewest and the most of them on any path from the player to the boss.
    #[serde(default)]
    pub range: MapRange,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::{RichSpan, Role};

    fn rich(text: &str) -> Rich {
        Rich(vec![RichSpan {
            fx: None,
            role: Role::Plain,
            bold: false,
            text: text.to_owned(),
        }])
    }

    #[test]
    fn a_screen_round_trips_through_json() {
        let screen = Screen {
            top: Some(TopBar {
                hp: 52,
                max_hp: 66,
                gold: 99,
                floor: 3,
                stars: None,
                fight: true,
                potions: vec![PotionSlot {
                    key: Some('z'),
                    name: "Fire Potion".to_owned(),
                    empty: false,
                    ..PotionSlot::default()
                }],
                relics: Vec::new(),
            }),
            title: Some(rich("Fight")),
            aside: None,
            blocks: vec![Block::List {
                frame: true,
                rows: vec![Button::row(1, rich("Strike"))],
                page: None,
            }],
            primary: Some(Button {
                key: Some(Key::Char('e')),
                label: rich("End Turn"),
                ..Button::row(0, rich("End Turn"))
            }),
            sheet: None,
            status: Some(rich("Not enough energy")),
        };
        let json = serde_json::to_string(&screen).expect("serialize");
        let back: Screen = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(screen, back);
    }

    #[test]
    fn a_tap_names_a_button_not_a_position() {
        let screen = Screen {
            blocks: vec![Block::List {
                frame: false,
                rows: vec![Button::row(7, rich("Take")), Button::row(8, rich("Skip"))],
                page: None,
            }],
            ..Screen::default()
        };
        let Block::List { rows, .. } = &screen.blocks[0] else {
            panic!("a list");
        };
        assert_eq!(rows[1].id, ButtonId(8));
        let tap = Input::Tap(rows[1].id);
        assert_eq!(serde_json::to_string(&tap).expect("json"), r#"{"Tap":8}"#);
    }
}
