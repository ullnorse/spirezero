//! The run env's fixed action space: the combat space ([`crate::action`], indices
//! 0-139) with the run-level choices appended from [`RUN_BASE`] (`docs/specs/rl-2x-plan.md`,
//! stage 9).
//!
//! | block | index | slots |
//! |---|---|---|
//! | ancient option `i` | [`ANCIENT_BASE`] + `i` | [`OPTION_SLOTS`] |
//! | event option `i` | [`EVENT_BASE`] + `i` | [`OPTION_SLOTS`] |
//! | map: travel to column `c` of the next row | [`NODE_BASE`] + `c` | [`COLUMN_SLOTS`] |
//! | claim reward `r` | [`CLAIM_BASE`] + `r` | [`REWARD_SLOTS`] |
//! | card `c` of card reward `r` | [`PICK_CARD_BASE`] + `r` × [`REWARD_CARD_SLOTS`] + `c` | 8 × 5 |
//! | alternative `a` (1 or 2; 0 is skip) of card reward `r` | [`ALTERNATIVE_BASE`] + `r` × 2 + `a` − 1 | 8 × 2 |
//! | skip reward `r` | [`SKIP_BASE`] + `r` | [`REWARD_SLOTS`] |
//! | rest-site option `i` | [`REST_BASE`] + `i` | [`OPTION_SLOTS`] |
//! | use the potion in belt slot `s` | [`USE_POTION_BASE`] + `s` | [`POTION_SLOTS`] |
//! | pick option `i` of a deck selection | [`SELECT_BASE`] + `i` | [`SELECT_SLOTS`] |
//! | confirm / cancel a selection, take the chest's relic | [`CONFIRM_SELECT`], [`CANCEL_SELECT`], [`TAKE_RELIC`] | 1 each |
//! | buy shop card / relic / potion `i` | [`BUY_CARD_BASE`], [`BUY_RELIC_BASE`], [`BUY_POTION_BASE`] + `i` | 8, 4, 4 |
//! | the shop's card removal | [`REMOVE_CARD`] | 1 |
//! | take bundle `i` | [`BUNDLE_BASE`] + `i` | [`BUNDLE_SLOTS`] |
//! | proceed | [`PROCEED`] | 1 |
//!
//! A combat action has its combat index. The map's next points all lie in one row, so a column
//! names one. As in the combat space, the mask is derived from [`Run::legal_actions`]'s list:
//! each listed action sets its index's bit and records its list position; a listed action
//! with no index (past a block's slots) gets no bit and is counted, as is one whose index an
//! earlier listed action took. [`RunAction::DiscardPotion`] is never listed, and [`RunAction::DeselectCard`] is left to the player, so neither has an index.

use sts2_core::run::{Run, RunAction, RunDecision};

use crate::action::{self, ACTIONS, IndexMask};

/// Options of an ancient, an event page or a rest site.
pub const OPTION_SLOTS: usize = 8;
/// Map columns ([`sts2_core::run::map::COLUMNS`] is 7).
pub const COLUMN_SLOTS: usize = 8;
/// Rewards of a rewards screen.
pub const REWARD_SLOTS: usize = 8;
/// Cards of one card reward.
pub const REWARD_CARD_SLOTS: usize = 5;
/// Alternatives of one card reward besides skipping.
pub const ALTERNATIVE_SLOTS: usize = 2;
/// Potion belt slots.
pub const POTION_SLOTS: usize = action::POTION_SLOTS;
/// Options of a deck card selection.
pub const SELECT_SLOTS: usize = 80;
/// Shop cards (character then colorless), relics and potions.
pub const SHOP_CARD_SLOTS: usize = 8;
pub const SHOP_RELIC_SLOTS: usize = 4;
pub const SHOP_POTION_SLOTS: usize = 4;
/// Bundles of a choose-a-bundle screen.
pub const BUNDLE_SLOTS: usize = 4;

/// The first run-level index, after the combat space.
pub const RUN_BASE: usize = ACTIONS;
pub const ANCIENT_BASE: usize = RUN_BASE;
pub const EVENT_BASE: usize = ANCIENT_BASE + OPTION_SLOTS;
pub const NODE_BASE: usize = EVENT_BASE + OPTION_SLOTS;
pub const CLAIM_BASE: usize = NODE_BASE + COLUMN_SLOTS;
pub const PICK_CARD_BASE: usize = CLAIM_BASE + REWARD_SLOTS;
pub const ALTERNATIVE_BASE: usize = PICK_CARD_BASE + REWARD_SLOTS * REWARD_CARD_SLOTS;
pub const SKIP_BASE: usize = ALTERNATIVE_BASE + REWARD_SLOTS * ALTERNATIVE_SLOTS;
pub const REST_BASE: usize = SKIP_BASE + REWARD_SLOTS;
pub const USE_POTION_BASE: usize = REST_BASE + OPTION_SLOTS;
pub const SELECT_BASE: usize = USE_POTION_BASE + POTION_SLOTS;
pub const CONFIRM_SELECT: usize = SELECT_BASE + SELECT_SLOTS;
pub const CANCEL_SELECT: usize = CONFIRM_SELECT + 1;
pub const TAKE_RELIC: usize = CANCEL_SELECT + 1;
pub const BUY_CARD_BASE: usize = TAKE_RELIC + 1;
pub const BUY_RELIC_BASE: usize = BUY_CARD_BASE + SHOP_CARD_SLOTS;
pub const BUY_POTION_BASE: usize = BUY_RELIC_BASE + SHOP_RELIC_SLOTS;
pub const REMOVE_CARD: usize = BUY_POTION_BASE + SHOP_POTION_SLOTS;
pub const BUNDLE_BASE: usize = REMOVE_CARD + 1;
pub const PROCEED: usize = BUNDLE_BASE + BUNDLE_SLOTS;
/// Indices in the whole space (combat and run).
pub const RUN_ACTIONS: usize = PROCEED + 1;
/// `u64` words of a run mask.
pub const RUN_MASK_WORDS: usize = RUN_ACTIONS.div_ceil(64);

/// The run space's mask.
pub type RunMask = IndexMask<RUN_ACTIONS, RUN_MASK_WORDS>;

/// `base + i` if `i` is below `slots`.
fn slot(base: usize, i: usize, slots: usize) -> Option<usize> {
    (i < slots).then_some(base + i)
}

/// The index of `action` in `run`'s current position, if the space has one.
#[must_use]
pub fn run_index_of(run: &Run, action: &RunAction) -> Option<usize> {
    match action {
        RunAction::Combat(action) => action::index_of(run.combat()?, action),
        other => run_level_index(other),
    }
}

/// The index of a run-level (not combat) action, if the space has one; `None` for a combat
/// action, whose index depends on the fight.
#[must_use]
pub fn run_level_index(action: &RunAction) -> Option<usize> {
    match action {
        RunAction::AncientOption(i) => slot(ANCIENT_BASE, *i, OPTION_SLOTS),
        RunAction::EventOption(i) => slot(EVENT_BASE, *i, OPTION_SLOTS),
        RunAction::ChooseNode(coord) => slot(NODE_BASE, coord.col, COLUMN_SLOTS),
        RunAction::ClaimReward(r) => slot(CLAIM_BASE, *r, REWARD_SLOTS),
        RunAction::PickCard { reward, card } if *reward < REWARD_SLOTS => slot(
            PICK_CARD_BASE + reward * REWARD_CARD_SLOTS,
            *card,
            REWARD_CARD_SLOTS,
        ),
        RunAction::CardAlternative {
            reward,
            alternative,
        } if *reward < REWARD_SLOTS && *alternative >= 1 => slot(
            ALTERNATIVE_BASE + reward * ALTERNATIVE_SLOTS,
            alternative - 1,
            ALTERNATIVE_SLOTS,
        ),
        RunAction::Combat(_)
        | RunAction::PickCard { .. }
        | RunAction::CardAlternative { .. }
        | RunAction::DeselectCard(_)
        | RunAction::DiscardPotion(_) => None,
        RunAction::SkipReward(r) => slot(SKIP_BASE, *r, REWARD_SLOTS),
        RunAction::RestOption(i) => slot(REST_BASE, *i, OPTION_SLOTS),
        RunAction::UsePotion(s) => slot(USE_POTION_BASE, *s, POTION_SLOTS),
        RunAction::SelectCard(i) => slot(SELECT_BASE, *i, SELECT_SLOTS),
        RunAction::ConfirmSelect => Some(CONFIRM_SELECT),
        RunAction::CancelSelect => Some(CANCEL_SELECT),
        RunAction::TakeRelic => Some(TAKE_RELIC),
        RunAction::BuyCard(i) => slot(BUY_CARD_BASE, *i, SHOP_CARD_SLOTS),
        RunAction::BuyRelic(i) => slot(BUY_RELIC_BASE, *i, SHOP_RELIC_SLOTS),
        RunAction::BuyPotion(i) => slot(BUY_POTION_BASE, *i, SHOP_POTION_SLOTS),
        RunAction::RemoveCard => Some(REMOVE_CARD),
        RunAction::SelectBundle(i) => slot(BUNDLE_BASE, *i, BUNDLE_SLOTS),
        RunAction::Proceed => Some(PROCEED),
    }
}

impl RunMask {
    /// Fills the mask from `legal`, `run`'s legal list; the number of listed actions that get
    /// no bit.
    ///
    /// # Panics
    /// If the list has 65,536 actions or more.
    pub fn fill_run(&mut self, run: &Run, legal: &[RunAction]) -> u32 {
        // The combat lookups need the fight; read it once.
        match run.pending_decision() {
            RunDecision::Combat(combat) => self.fill_with(legal, |a| match a {
                RunAction::Combat(a) => action::index_of(combat, a),
                other => run_index_of(run, other),
            }),
            _ => self.fill_with(legal, |a| run_index_of(run, a)),
        }
    }

    /// Clears each index whose action (`legal`'s, `run`'s list, as filled) the engine would
    /// refuse ([`Run::accepts`]); how many it cleared.
    ///
    /// # Panics
    /// If the mask was not filled from `legal`.
    pub fn retain_accepted(&mut self, run: &Run, legal: &[RunAction]) -> u32 {
        let mut refused = 0;
        for word in 0..RUN_MASK_WORDS {
            let mut bits = self.bits()[word];
            while bits != 0 {
                let index = word * 64 + bits.trailing_zeros() as usize;
                bits &= bits - 1;
                let position = self.position(index).expect("a set index");
                if !run.accepts(&legal[position]) {
                    self.remove(index);
                    refused += 1;
                }
            }
        }
        refused
    }
}

/// Checks that `mask` is `legal`'s (`run`'s legal list) less the indices whose action the
/// engine would refuse.
///
/// Every set bit decodes to the first listed action with that index, every listed action the
/// space maps has its index's bit unless [`Run::accepts`] says no, and nothing else is set.
///
/// # Errors
/// The first disagreement.
pub fn check_run_mask(run: &Run, legal: &[RunAction], mask: &RunMask) -> Result<(), String> {
    let mut seen = RunMask::new();
    let mut mapped = RunMask::new();
    for (i, action) in legal.iter().enumerate() {
        let Some(index) = run_index_of(run, action) else {
            continue;
        };
        if seen.contains(index) {
            continue;
        }
        seen.fill_one(index, i);
        if !run.accepts(action) {
            continue;
        }
        mapped.fill_one(index, i);
        if mask.position(index) != Some(i) {
            return Err(format!("{action:?} (index {index}) is not at position {i}"));
        }
    }
    if mask.bits() != mapped.bits() {
        return Err(format!(
            "{} bits set for {} mapped indices",
            mask.count(),
            mapped.count()
        ));
    }
    Ok(())
}
