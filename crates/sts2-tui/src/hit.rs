//! The hit-test layer between drawing and the mouse.
//!
//! Every frame the screens register the rectangles of what can be clicked ([`Hits::push`]); a
//! mouse event is looked up in the last frame's regions ([`Hits::at`]) and becomes the same
//! [`UiAction`] a key would make, so a layout change moves the regions with it and the input
//! code never sees coordinates. The frame also carries the keyboard focus ([`Hits::focused`])
//! so every screen highlights the same thing the same way.

use ratatui::layout::{Position, Rect};
use sts2_core::combat::{CardId, CreatureId};
use sts2_core::run::MapCoord;

use crate::app::Overlay;

/// What the player asks for, by key or by mouse. The app and the select screen carry these out;
/// keys and clicks only differ in how they find them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UiAction {
    /// The `n`th (1-based) entry of the current list: a menu entry or a fight's card choice,
    /// what typing `n` picks.
    Pick(usize),
    EndTurn,
    /// A card of the hand is pressed (a key, Enter or a click): played at once when it needs no
    /// target, else it waits for its target.
    Card(CardId),
    /// Selects a card of the hand (a drag starts).
    Arm(CardId),
    /// A creature as the target of the selected card or potion.
    Target(CreatureId),
    /// Plays the dragged card that needs no target (a drop on the battlefield).
    PlayArmed,
    /// Uses the potion in this belt slot (targeting first if it needs one); throws it away
    /// (from the `≡` menu).
    UsePotion(usize),
    TossPotion(usize),
    /// Travels to a map node.
    Travel(MapCoord),
    Toggle(Overlay),
    /// Esc or a right click: closes an overlay or a menu, cancels a pick or a confirmation.
    Back,
    /// Scrolls the open list (an overlay, the log in a fight, else the menu); negative is up.
    Scroll(isize),
    /// Scrolls the map screen's grid; negative shows rows further up (toward the boss).
    ScrollMap(isize),
    /// Confirms the button pressed once (its own key again, Enter, a click on it again), or
    /// puts it back (any other key, a click elsewhere).
    Yes,
    No,
    /// End of run: retry the seed, a new seed, back to the character select.
    Retry,
    NewSeed,
    CharacterSelect,
    Quit,
    /// Character select: pick a character by its place on the screen.
    SelectCharacter(usize),
    /// Character select: one ascension up (`true`) or down.
    Ascension(bool),
    EditSeed,
    RandomSeed,
    Start,
    /// Select screen: continue the saved run, from its last action or the one before; or set
    /// up a new run instead.
    Continue,
    ContinueEarlier,
    NewRun,
    /// Asks the bot what it would do here (`w`).
    Hint,
    /// Opens the help or the settings; closes either.
    Help,
    Settings,
    Close,
    /// Settings: the next value of the `n`th (0-based) row.
    Setting(usize),
}

/// A scrollable area, so the wheel scrolls what is under the pointer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Pane {
    Log,
    Menu,
    Overlay,
    Map,
    Ascension,
    Help,
}

/// Something drawn that the mouse can click or the keyboard focus can rest on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Target {
    /// A button: a click performs the action.
    Button(UiAction),
    /// A card in the hand.
    Card(CardId),
    /// The `n`th (1-based) card offered by a fight's card choice.
    Choice(usize, CardId),
    /// A Sovereign Blade (or another card shown for information).
    InfoCard(CardId),
    Creature(CreatureId),
    /// The creature's `n`th power (in `Creature::powers`).
    Power(CreatureId, usize),
    Intent(CreatureId),
    Orb(usize),
    Relic(usize),
    Potion(usize),
    MapNode(MapCoord),
    /// The fight's play area: where a selected card that needs no target is dropped.
    Battlefield,
    Pane(Pane),
}

impl Target {
    /// What a click does, or `None` for what only shows information: the click then falls
    /// through to what is under it.
    #[must_use]
    pub const fn click(self) -> Option<UiAction> {
        match self {
            Self::Button(action) => Some(action),
            Self::Card(id) => Some(UiAction::Card(id)),
            Self::Choice(n, _) => Some(UiAction::Pick(n)),
            Self::Potion(slot) => Some(UiAction::UsePotion(slot)),
            Self::MapNode(coord) => Some(UiAction::Travel(coord)),
            Self::InfoCard(_)
            | Self::Creature(_)
            | Self::Power(..)
            | Self::Intent(_)
            | Self::Orb(_)
            | Self::Relic(_)
            | Self::Battlefield
            | Self::Pane(_) => None,
        }
    }
}

/// One clickable or hoverable rectangle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Region {
    pub rect: Rect,
    pub target: Target,
}

/// The regions of one frame, in drawing order (later ones are on top), and the keyboard focus
/// the frame highlights.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Hits {
    regions: Vec<Region>,
    /// What the keyboard focus rests on: drawn highlighted, pressed by Enter.
    focus: Option<Target>,
    /// How far each pane can scroll, as the frame laid it out.
    limits: Vec<(Pane, isize, isize)>,
}

impl Hits {
    /// An empty frame.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            regions: Vec::new(),
            focus: None,
            limits: Vec::new(),
        }
    }

    /// Sets what the frame highlights as focused.
    pub const fn set_focus(&mut self, focus: Option<Target>) {
        self.focus = focus;
    }

    /// Whether the keyboard focus rests on `target`.
    #[must_use]
    pub fn focused(&self, target: Target) -> bool {
        self.focus == Some(target)
    }

    /// Whether the focus rests on a creature (a card or potion is being aimed) other than
    /// `creature`.
    #[must_use]
    pub fn aiming_elsewhere(&self, creature: CreatureId) -> bool {
        matches!(self.focus, Some(Target::Creature(c)) if c != creature)
    }

    /// Registers `target` at `rect` (an empty rectangle is ignored).
    pub fn push(&mut self, rect: Rect, target: Target) {
        if !rect.is_empty() {
            self.regions.push(Region { rect, target });
        }
    }

    /// A popup is drawn at `popup` over everything so far: a click anywhere else is `target`,
    /// and what is under it cannot be reached, except the buttons that open other popups (or
    /// quit) still in view beside it.
    pub fn modal(&mut self, backdrop: Rect, popup: Rect, target: Target) {
        let kept: Vec<Region> = self
            .regions
            .drain(..)
            .filter(|r| {
                matches!(
                    r.target,
                    Target::Button(UiAction::Toggle(_) | UiAction::Quit)
                ) && r.rect.intersection(popup).is_empty()
            })
            .collect();
        self.push(backdrop, target);
        self.regions.extend(kept);
    }

    /// Notes that `pane` can scroll from `min` to `max`.
    pub fn limit(&mut self, pane: Pane, min: isize, max: isize) {
        self.limits.retain(|(p, ..)| *p != pane);
        self.limits.push((pane, min, max));
    }

    /// How far `pane` could scroll in this frame, if it was drawn: `(min, max)`.
    #[must_use]
    pub fn scroll_limit(&self, pane: Pane) -> Option<(isize, isize)> {
        self.limits
            .iter()
            .find(|(p, ..)| *p == pane)
            .map(|&(_, min, max)| (min, max))
    }

    /// The targets under `pos`, the topmost first.
    pub fn at(&self, pos: Position) -> impl Iterator<Item = Target> + '_ {
        self.regions
            .iter()
            .rev()
            .filter(move |r| r.rect.contains(pos))
            .map(|r| r.target)
    }

    /// Where `target` was drawn last (the topmost of its regions).
    #[must_use]
    pub fn rect_of(&self, target: Target) -> Option<Rect> {
        self.regions
            .iter()
            .rev()
            .find(|r| r.target == target)
            .map(|r| r.rect)
    }

    #[must_use]
    pub fn regions(&self) -> &[Region] {
        &self.regions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn rect(x: u16, y: u16, width: u16, height: u16) -> Rect {
        Rect {
            x,
            y,
            width,
            height,
        }
    }

    #[test]
    fn the_topmost_region_wins_and_lower_ones_follow() {
        let mut hits = Hits::new();
        hits.push(rect(0, 0, 20, 10), Target::Battlefield);
        hits.push(rect(2, 2, 5, 3), Target::Button(UiAction::EndTurn));
        hits.push(rect(0, 0, 0, 5), Target::Button(UiAction::Quit));
        let inside: Vec<Target> = hits.at(Position::new(3, 3)).collect();
        assert_eq!(
            inside,
            [Target::Button(UiAction::EndTurn), Target::Battlefield]
        );
        assert_eq!(hits.at(Position::new(25, 3)).count(), 0);
        assert_eq!(hits.regions().len(), 2, "an empty rectangle is dropped");
        assert_eq!(
            hits.rect_of(Target::Button(UiAction::EndTurn)),
            Some(rect(2, 2, 5, 3))
        );
    }

    #[test]
    fn a_modal_hides_what_is_under_it() {
        let mut hits = Hits::new();
        hits.push(rect(0, 0, 10, 10), Target::Button(UiAction::EndTurn));
        let deck = Target::Button(UiAction::Toggle(Overlay::Deck));
        hits.push(rect(0, 9, 3, 1), deck);
        hits.modal(
            rect(0, 0, 10, 10),
            rect(2, 2, 3, 3),
            Target::Button(UiAction::Back),
        );
        assert_eq!(
            hits.at(Position::new(1, 9)).next(),
            Some(deck),
            "kept beside the popup"
        );
        hits.push(rect(2, 2, 3, 3), Target::Pane(Pane::Overlay));
        let inside: Vec<Target> = hits.at(Position::new(3, 3)).collect();
        assert_eq!(
            inside,
            [Target::Pane(Pane::Overlay), Target::Button(UiAction::Back)]
        );
    }

    #[test]
    fn information_falls_through_to_what_is_under_it() {
        assert_eq!(Target::Relic(0).click(), None);
        assert_eq!(Target::Potion(1).click(), Some(UiAction::UsePotion(1)));
        assert_eq!(
            Target::Choice(2, CardId(0)).click(),
            Some(UiAction::Pick(2))
        );
    }

    #[test]
    fn limits_are_kept_per_pane() {
        let mut hits = Hits::new();
        hits.set_focus(Some(Target::Battlefield));
        hits.limit(Pane::Menu, 0, 4);
        hits.limit(Pane::Menu, 0, 6);
        assert_eq!(hits.scroll_limit(Pane::Menu), Some((0, 6)));
        assert_eq!(hits.scroll_limit(Pane::Log), None);
        assert!(hits.focused(Target::Battlefield));
    }
}
