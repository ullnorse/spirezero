//! C28: event text values (`EventRoom::vars`) the engine used to leave unknown, and finished
//! pages' keys.

use super::*;
use crate::content::characters::Character;
use crate::content::events::{EventKind, EventVar};
use crate::content::tables::VarText;

/// `T6S0` in Act 2 with 300 gold, standing in `kind`.
fn in_event(kind: EventKind) -> Run {
    let mut run = Run::new(
        Character::Necrobinder,
        "T6S0",
        0,
        UnlockState::all(),
        Act1::Overgrowth,
    )
    .expect("T6S0 starts");
    run.state.act_index = 1;
    run.state.gold = 300;
    run.state.room = Room::Event { id: kind.id() };
    run.phase = run.begin_event(kind).expect("the event begins");
    run
}

fn var(run: &Run, name: &str) -> Option<EventVar> {
    let room = run
        .finished_event()
        .or_else(|| run.event())
        .expect("an event");
    room.vars(&run.state)
        .into_iter()
        .find(|&(n, _)| n == name)
        .map(|(_, v)| v)
}

fn number(n: i32) -> EventVar {
    EventVar::Number(Decimal::from(n))
}

#[test]
fn wongo_purchases_show_their_points() {
    // `CheckObtainWongoBadge(32)` with no profile points: 32 earned, 1968 to the badge.
    let mut run = in_event(EventKind::WelcomeToWongos);
    run.apply(RunAction::EventOption(0)).expect("BARGAIN_BIN");
    let room = run.finished_event().expect("finished");
    assert_eq!(
        room.finished.as_deref(),
        Some("WELCOME_TO_WONGOS.pages.AFTER_BUY.description")
    );
    assert_eq!(var(&run, "WongoPointAmount"), Some(number(32)));
    assert_eq!(var(&run, "RemainingWongoPointAmount"), Some(number(1968)));
    assert_eq!(var(&run, "TotalWongoBadgeAmount"), Some(number(0)));
}

#[test]
fn computed_event_vars_are_filled_in() {
    let run = in_event(EventKind::ColossalFlower);
    assert_eq!(var(&run, "Prize1"), Some(number(35)));
    assert_eq!(var(&run, "Prize3"), Some(number(135)));
    let run = in_event(EventKind::BattlewornDummy);
    assert!(matches!(var(&run, "Setting2Hp"), Some(EventVar::Number(_))));
    let run = in_event(EventKind::TeaMaster);
    assert_eq!(
        var(&run, "BoneTeaDescription"),
        Some(EventVar::Text(VarText::RelicDescription("BONE_TEA")))
    );
    let run = in_event(EventKind::AromaOfChaos);
    assert_eq!(
        var(&run, "AromaPrinciple"),
        Some(EventVar::Text(VarText::Character("aromaPrinciple")))
    );
    // `Rng.Chaotic` is not reproducible: no `EntrantNumber`.
    let run = in_event(EventKind::Trial);
    assert_eq!(var(&run, "EntrantNumber"), None);
}

#[test]
fn trial_keeps_its_case_and_verdict() {
    let mut run = in_event(EventKind::Trial);
    run.apply(RunAction::EventOption(0)).expect("ACCEPT");
    let room = run.event().expect("on the case page");
    assert_eq!(room.page_description(), Some("TRIAL.trialFormat"));
    let Some(EventVar::Text(VarText::Event(story))) = var(&run, "TrialStory") else {
        panic!("the case's story");
    };
    let case = story
        .strip_prefix("TRIAL.pages.")
        .and_then(|s| s.strip_suffix(".description"))
        .expect("a case page");
    run.apply(RunAction::EventOption(1)).expect("INNOCENT");
    let room = run.finished_event().expect("finished");
    assert_eq!(room.finished.as_deref(), Some("TRIAL.trialResult"));
    assert_eq!(room.page_description(), None);
    let result = format!("TRIAL.pages.{case}_INNOCENT.description");
    assert!(
        matches!(var(&run, "TrialResult"), Some(EventVar::Text(VarText::Event(k))) if k == result)
    );
}

/// C29: non-event deck selections carry the game's prompt.
mod c29 {
    use super::*;
    use crate::combat::PromptText;
    use crate::run::{CardSelect, SelectPrompt};

    fn select(purpose: SelectPurpose) -> CardSelect {
        CardSelect {
            purpose,
            options: Vec::new(),
            offered: Vec::new(),
            count: 1,
            picked: Vec::new(),
        }
    }

    #[test]
    fn selections_prompt_with_the_commands_text() {
        let run = Run::new(
            Character::Necrobinder,
            "T6S0",
            0,
            UnlockState::all(),
            Act1::Overgrowth,
        )
        .expect("T6S0 starts");
        let prompt = |purpose| select(purpose).prompt(&run.state);
        let shared = SelectPrompt::Shared;
        assert_eq!(prompt(SelectPurpose::Smith), shared(PromptText::Upgrade));
        assert_eq!(prompt(SelectPurpose::Cook), shared(PromptText::Remove));
        assert_eq!(
            prompt(SelectPurpose::ShopRemoval {
                cost: 75,
                cancelable: true
            }),
            shared(PromptText::Remove)
        );
        assert_eq!(
            prompt(SelectPurpose::TransformRandomUpgraded),
            shared(PromptText::Transform)
        );
        assert_eq!(
            prompt(SelectPurpose::ChooseACard { extra: None }),
            shared(PromptText::ChooseACard)
        );
        assert_eq!(
            prompt(SelectPurpose::Clone),
            SelectPrompt::Relic("DOLLYS_MIRROR")
        );
        assert_eq!(
            prompt(SelectPurpose::AddToDeckUpTo),
            SelectPrompt::Relic("SEA_GLASS")
        );
    }
}
