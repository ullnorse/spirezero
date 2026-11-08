//! Whether [`Run::apply`] accepts a listed action, without applying it.
//!
//! Some actions [`Run::legal_actions`] lists meet content the engine does not port, and `apply`
//! refuses them (leaving the run unchanged). [`Run::accepts`] answers exactly as `apply` would:
//! from the run as it is for the actions whose every way to fail is ruled out below, and for
//! the others (map travel, event, ancient and rest options, card selections, relic rewards,
//! shop purchases, potion uses) by applying the action to a copy.

use super::rewards::Reward;
use super::state::{Room, RoomType};
use super::{Phase, Run, RunAction};

impl Run {
    /// Whether [`Run::apply`] would accept `action`, one of [`Run::legal_actions`]'s.
    ///
    /// A combat action, a card picked or a reward skipped or claimed (but a relic) is answered
    /// from the run; anything else is applied to a copy.
    #[must_use]
    pub fn accepts(&self, action: &RunAction) -> bool {
        self.surely_accepts(action) || self.clone().apply_unjournaled(action).is_ok()
    }

    /// Whether the listed `action` cannot fail, known without applying it.
    fn surely_accepts(&self, action: &RunAction) -> bool {
        match (&self.phase, action) {
            // `Combat::apply` accepts every action its fight lists, so only the victory it may
            // bring can fail ([`Run::victory_may_fail`]).
            (Phase::Combat(_), RunAction::Combat(_)) => !self.victory_may_fail(),
            // Gold, a potion (listed only while a slot takes it) or a card: nothing to fail.
            (Phase::Rewards(rewards), RunAction::ClaimReward(i)) => matches!(
                rewards.get(*i),
                Some(Reward::Gold(_) | Reward::Potion(_) | Reward::SpecialCard(_))
            ),
            // `leave_rewards`: out of a boss room it is `finish_prompt`.
            (Phase::Rewards(_), RunAction::Proceed) => {
                !self.finish_prompt_may_fail()
                    && !matches!(
                        self.state.room,
                        Room::Combat {
                            room_type: RoomType::Boss,
                            ..
                        }
                    )
            }
            // `remove_reward` or a card added to the deck; `leave_room`.
            (Phase::Rewards(_), RunAction::SkipReward(_) | RunAction::PickCard { .. })
            | (
                Phase::Treasure(_) | Phase::Unsupported(_) | Phase::RestSite(_),
                RunAction::Proceed,
            ) => true,
            _ => false,
        }
    }

    /// Whether winning the current fight may fail: `win_combat` runs the relics'
    /// `after_combat_victory_in_run` (each says when it may fail), then the rewards (which
    /// cannot) or, for an encounter without them, `finish_prompt`.
    fn victory_may_fail(&self) -> bool {
        self.finish_prompt_may_fail()
            || self.state.relics.iter().enumerate().any(|(me, relic)| {
                !relic.melted && relic.kind.model().victory_in_run_may_fail(&self.state, me)
            })
    }

    /// Whether `finish_prompt` may fail: only a shop's queued free purchases can.
    const fn finish_prompt_may_fail(&self) -> bool {
        self.suspended_shop.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::Status;
    use crate::content::characters::Character;
    use crate::content::relics::RelicKind;
    use crate::run::relics::RunRelic;
    use crate::run::rooms::Act1;
    use crate::run::unlock::UnlockState;

    /// `SplitMix64`, for the random play.
    struct Mix(u64);

    impl Mix {
        fn below(&mut self, n: usize) -> usize {
            let n = u64::try_from(n).expect("fits");
            self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            usize::try_from((z ^ (z >> 31)) % n).expect("below n")
        }
    }

    /// Plays `runs` random runs of up to `actions` actions from seed `first`; at every position
    /// checks [`Run::accepts`] against `apply` on a copy for each listed action. The number of
    /// refused listed actions seen.
    fn check_random_runs(first: u64, runs: u64, actions: usize) -> usize {
        let mut refused = 0;
        let mut legal = Vec::new();
        for seed in first..first + runs {
            let character = Character::ALL[usize::try_from(seed % 5).expect("fits")];
            let Ok(mut run) = Run::new(
                character,
                &format!("ACC{seed}"),
                0,
                UnlockState::all(),
                Act1::Random,
            ) else {
                continue;
            };
            let mut mix = Mix(seed);
            for _ in 0..actions {
                run.legal_actions_into(&mut legal);
                let accepted: Vec<&RunAction> = legal
                    .iter()
                    .filter(|&action| {
                        let applied = run.clone().apply(action.clone()).is_ok();
                        assert_eq!(
                            run.accepts(action),
                            applied,
                            "{character:?} ACC{seed}: {action:?} in {:?}",
                            run.pending_decision()
                        );
                        refused += usize::from(!applied);
                        applied
                    })
                    .collect();
                if accepted.is_empty() {
                    break;
                }
                let action = accepted[mix.below(accepted.len())].clone();
                run.apply(action).expect("accepted");
            }
        }
        refused
    }

    #[test]
    fn accepts_agrees_with_apply_on_random_runs() {
        check_random_runs(0, 10, 400);
    }

    /// 10,000 runs (30 s): `cargo test --profile regress -p sts2-core accepts_agrees_long --
    /// --ignored`; 21 refused listed actions among them.
    #[test]
    #[ignore = "long; run in the regress profile"]
    fn accepts_agrees_long() {
        let refused = check_random_runs(1_000, 10_000, 3_000);
        assert!(refused > 0, "some listed actions are refused");
    }

    #[test]
    fn a_victory_that_meets_unported_content_is_not_accepted() {
        let mut run = Run::new(
            Character::Necrobinder,
            "T6S0",
            0,
            UnlockState::all(),
            Act1::Overgrowth,
        )
        .expect("T6S0 starts")
        .in_test_mode();
        run.apply(RunAction::AncientOption(0))
            .expect("Phial Holster is ported");
        run.state
            .relics
            .push(RunRelic::new(RelicKind::TestUnportedTrophy));
        run.forget_journal();
        let node = run
            .legal_actions()
            .into_iter()
            .find(|a| matches!(a, RunAction::ChooseNode(_)))
            .expect("a first node");
        run.apply(node).expect("the first room is a fight");
        let Phase::Combat(combat) = &mut run.phase else {
            panic!("a fight");
        };
        combat.with_engine(|eng| {
            for e in eng.s.enemies.clone() {
                eng.s.creature_mut(e).hp = 1;
            }
            Ok(())
        });
        run.forget_journal();
        let mut wins = 0;
        for action in run.legal_actions() {
            let RunAction::Combat(a) = &action else {
                continue;
            };
            let Phase::Combat(combat) = &run.phase else {
                unreachable!();
            };
            let won = combat.clone().apply(a.clone()) == Ok(Status::Won);
            wins += usize::from(won);
            assert_eq!(run.accepts(&action), !won, "{action:?}");
            assert_eq!(run.clone().apply(action).is_ok(), !won);
        }
        assert!(wins > 0, "a play that wins");
    }
}
