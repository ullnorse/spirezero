//! Ancients (`Models/AncientEventModel.cs`): the event at each act's start point. One file per
//! ancient, a unit struct implementing [`AncientModel`] that declares its options.
//!
//! To add an ancient: its file `<name>.rs` declaring `pub struct <Name>`; `build.rs` generates the
//! variant, `ALL`, `id` and `model` (see `docs/specs/content-api.md`, "Ancients"). The run layer
//! (`Run::enter_ancient`) does the rest: the heal on entry, the event `Rng`, the option choice
//! (`RelicCmd.Obtain`, then `Done`).

include!(concat!(env!("OUT_DIR"), "/content/ancients.rs"));

use crate::content::relics::{RelicData, RelicKind};
use crate::rng::Rng;
use crate::run::{Modifier, RunState};

/// An ancient's static data and its `GenerateInitialOptions`.
pub(crate) trait AncientModel {
    /// `GenerateInitialOptions`, after `BeforeEventStarted` (the heal): the options in display
    /// order, rolled on `rng` (the event `Rng`, `runSeed + xxh64(Id.Entry)`). `s` is the run as
    /// the player enters (`Owner.Deck`, `Owner.Character`, ...); an option's setup may draw on
    /// its streams (`DustyTome.SetupForPlayer` on the player's `rewards`).
    fn generate_options(&self, s: &mut RunState, rng: &mut Rng) -> Vec<AncientOption>;
}

/// One option of an ancient's page.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AncientOption {
    /// `AncientEventModel.RelicOption` (`AncientEventModel.cs:261-290`): `RelicCmd.Obtain`, then
    /// `Done`. `data` presets the relic's fields before it is obtained (`SeaGlass.CharacterId`,
    /// `SetupForPlayer`); `None` is its field initializers.
    Relic {
        /// The relic's `ModelId.Entry`.
        id: &'static str,
        /// Preset fields, or `None` for the relic's initial ones.
        data: Option<RelicData>,
    },
    /// An option with a null `onChosen` (`OROBAS.pages.INITIAL.options.OPTION_POOL_3_LOCKED`):
    /// shown, never chosen. The name is the last part of its text key.
    Locked(&'static str),
    /// A run modifier's option on Neow's page (`ModifierModel.GenerateNeowOption`): its title and
    /// description, its effect on choosing. With modifiers Neow offers only these, one page each.
    Modifier(Modifier),
}

impl AncientOption {
    /// `RelicOption<T>()`: the relic `id` with its field initializers.
    #[must_use]
    pub const fn relic(id: &'static str) -> Self {
        Self::Relic { id, data: None }
    }

    /// The relic it obtains, if any.
    #[must_use]
    pub const fn relic_id(self) -> Option<&'static str> {
        match self {
            Self::Relic { id, .. } => Some(id),
            Self::Locked(_) | Self::Modifier(_) => None,
        }
    }

    /// The relic, or the locked option's name: the last part of its text key.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Relic { id, .. } | Self::Locked(id) => id,
            Self::Modifier(modifier) => modifier.id(),
        }
    }

    /// The option's `TextKey` (`EventModel.OptionKey("INITIAL", name)`,
    /// `EventModel.cs:615-621`): `<ANCIENT>.pages.INITIAL.options.<name>`.
    #[must_use]
    pub fn text_key(self, ancient: AncientKind) -> String {
        format!("{}.pages.INITIAL.options.{}", ancient.id(), self.name())
    }

    /// Whether choosing it is ported: a relic whose effects are. A locked option is never chosen.
    #[must_use]
    pub fn is_supported(self) -> bool {
        matches!(self, Self::Modifier(_))
            || self
                .relic_id()
                .is_some_and(|id| RelicKind::from_id(id).is_some_and(|k| k.model().obtainable()))
    }
}

/// An ancient's page, before an option is chosen.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AncientRoom {
    /// The ancient.
    pub kind: AncientKind,
    /// `GeneratedOptions`, in display order.
    pub options: Vec<AncientOption>,
}

impl AncientKind {
    /// The ancient with `ModelId.Entry` `id`, if ported; in tests also the test-only ones (an act's
    /// ancient is an id, so a test can stand one in).
    #[must_use]
    pub fn lookup(id: &str) -> Option<Self> {
        let found = Self::from_id(id);
        #[cfg(test)]
        let found = found.or_else(|| Self::TEST_ONLY.into_iter().find(|k| k.id() == id));
        found
    }
}
