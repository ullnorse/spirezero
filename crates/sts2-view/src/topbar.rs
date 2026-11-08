//! The bar above every screen: HP, gold, the floor, the Regent's stars and the potion belt.
//!
//! One builder for every screen, in a fight or out of it. A fight owns the HP, the gold and
//! the stars while it runs; the belt is the run's. Which potions the engine lets be used comes
//! from `legal_actions`; which one waits for its confirmation or its target is the client's
//! [`Waiting`].

use sts2_core::Character;
use sts2_core::run::Run;

use crate::combat::POTION_KEYS;
use crate::describe::Describer;
use crate::model::{PotionSlot, RelicChip, RelicLook, TopBar};
use crate::names;
use crate::text::{self, Role};
use crate::ui::{ids, potion_actions};
use sts2_core::content::relics::RelicStatus;

/// The belt slot that is selected, and what it waits for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Waiting {
    pub slot: usize,
    /// Waiting for a confirmation (Enter uses it, `t` throws it away) rather than a target.
    pub confirm: bool,
}

/// The top bar for the run as it stands; `waiting` is the selected potion, if any.
#[must_use]
pub fn build(run: &Run, describer: &Describer, waiting: Option<Waiting>) -> TopBar {
    let state = run.state();
    let (hp, gold, stars) = run.combat().map_or((state.hp, state.gold, None), |c| {
        let s = c.state();
        (
            s.creature(s.player.creature).hp.max(0),
            s.player.gold,
            (s.player.character == Character::Regent).then_some(s.player.stars),
        )
    });
    let usable: Vec<usize> = potion_actions(run)
        .into_iter()
        .map(|(slot, _)| slot)
        .collect();
    TopBar {
        hp,
        max_hp: state.max_hp,
        gold,
        floor: u32::try_from(state.total_floor()).unwrap_or(0),
        stars,
        fight: run.combat().is_some(),
        potions: run
            .potions()
            .iter()
            .enumerate()
            .map(|(slot, potion)| PotionSlot {
                key: POTION_KEYS.get(slot).copied(),
                art: potion.map_or_else(String::new, str::to_owned),
                name: potion.map_or_else(String::new, names::potion),
                empty: potion.is_none(),
                id: ids::potion(slot),
                usable: usable.contains(&slot),
                selected: waiting.is_some_and(|w| w.slot == slot),
                asks: waiting.is_some_and(|w| w.slot == slot && w.confirm),
                detail: potion
                    .and_then(|p| describer.potion(p))
                    .map_or_else(Vec::new, |markup| text::lines(&markup, Role::Plain)),
            })
            .collect(),
        relics: relics(run, describer),
    }
}

/// The relics with their counters and text; in a fight their fields are the fight's.
fn relics(run: &Run, describer: &Describer) -> Vec<RelicChip> {
    let combat = run.combat().map(sts2_core::Combat::state);
    let owned = &run.state().relics;
    let in_fight = combat
        .map(|c| &c.player.relics)
        .filter(|r| r.len() == owned.len());
    owned
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let data = in_fight.map_or(r.data, |fight| fight[i].data);
            RelicChip {
                name: r.kind.name().to_owned(),
                art: r.kind.id().to_owned(),
                counter: r.kind.counter(&data, combat),
                wax: r.wax,
                // `UpdateShader`: a melted relic shows neither.
                status: match r.kind.status(&data, combat, run.state()) {
                    _ if r.melted => RelicLook::Normal,
                    RelicStatus::Normal => RelicLook::Normal,
                    RelicStatus::Active => RelicLook::Active,
                    RelicStatus::Disabled => RelicLook::Disabled,
                },
                detail: describer
                    .owned_relic(r.kind, &data)
                    .map_or_else(Vec::new, |markup| text::lines(&markup, Role::Plain)),
            }
        })
        .collect()
}
