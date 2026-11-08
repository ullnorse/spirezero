//! `DollRoom` (`Models.Events/DollRoom.cs`): one of three doll relics (`DaughterOfTheWind`,
//! `MrStruggles`, `BingBong`) at random, or 5 HP to choose from two, or 15 HP to choose from all
//! three. Shared, Act 2 only.

use crate::content::events::{EventAction, EventCtx, EventModel, EventOption, Page};
use crate::content::relics::RelicKind;
use crate::run::{RunError, RunState};
use crate::shuffle::stable_shuffle_by;

/// `_dolls`, in declaration order.
const DOLLS: [RelicKind; 3] = [
    RelicKind::DaughterOfTheWind,
    RelicKind::MrStruggles,
    RelicKind::BingBong,
];
/// Each doll's `descriptionKey`, the page `ChooseDollAndShowDescription` finishes on.
const DOLL_PAGES: [&str; 3] = [
    "DOLL_ROOM.pages.DAUGHTER_OF_WIND.description",
    "DOLL_ROOM.pages.MR_STRUGGLES.description",
    "DOLL_ROOM.pages.FABLE.description",
];
/// `DamageVar("TakeTimeHpLoss", 5m, Unblockable | Unpowered)`.
const TAKE_TIME_HP_LOSS: i32 = 5;
/// `DamageVar("ExamineHpLoss", 15m, Unblockable | Unpowered)`.
const EXAMINE_HP_LOSS: i32 = 15;

pub struct DollRoom;

/// `_dolls.ToList().StableShuffle(Rng)`: sorted by `RelicModel.CompareTo` (the id, ordinal),
/// then shuffled.
fn shuffled_dolls(ctx: &mut EventCtx<'_>) -> Vec<RelicKind> {
    let mut dolls = DOLLS.to_vec();
    stable_shuffle_by(&mut dolls, ctx.rng, |a, b| a.id().cmp(b.id()));
    dolls
}

/// `OptionFromChoice`: the option's `TextKey` is the relic's raw title; its argument is the
/// doll's index in [`DOLLS`].
fn doll_options(dolls: &[RelicKind]) -> Vec<EventOption> {
    dolls
        .iter()
        .map(|&doll| {
            let index = DOLLS.iter().position(|&d| d == doll).expect("a doll");
            EventOption::with_arg(doll.name(), "TAKE", index)
        })
        .collect()
}

impl EventModel for DollRoom {
    /// `CurrentActIndex == 1` (`DollRoom.cs:79-82`).
    fn is_allowed(&self, s: &RunState) -> bool {
        s.act_index == 1
    }

    fn begin(&self, _ctx: &mut EventCtx<'_>) -> Result<Vec<EventOption>, RunError> {
        Ok(vec![
            EventOption::new("DOLL_ROOM.pages.INITIAL.options.RANDOM", "RANDOM"),
            EventOption::new(
                "DOLL_ROOM.pages.INITIAL.options.TAKE_SOME_TIME",
                "TAKE_SOME_TIME",
            ),
            EventOption::new("DOLL_ROOM.pages.INITIAL.options.EXAMINE", "EXAMINE"),
        ])
    }

    fn choose(&self, action: EventAction, ctx: &mut EventCtx<'_>) -> Result<Page, RunError> {
        match action.tag {
            "RANDOM" => {
                let doll = *ctx.rng.next_item(&DOLLS).expect("three dolls");
                ctx.obtain_relic(doll.id())?;
                let index = DOLLS.iter().position(|&d| d == doll).expect("a doll");
                Ok(Page::Finished(DOLL_PAGES[index].to_owned()))
            }
            "TAKE_SOME_TIME" => {
                ctx.state().damage(TAKE_TIME_HP_LOSS);
                let dolls = shuffled_dolls(ctx);
                Ok(Page::Next(doll_options(&dolls[..2])))
            }
            "EXAMINE" => {
                ctx.state().damage(EXAMINE_HP_LOSS);
                let dolls = shuffled_dolls(ctx);
                Ok(Page::Next(doll_options(&dolls)))
            }
            _ => {
                ctx.obtain_relic(DOLLS[action.arg].id())?;
                Ok(Page::Finished(DOLL_PAGES[action.arg].to_owned()))
            }
        }
    }
}
