//! What a relic shows: its counter (`ShowCounter`, `DisplayAmount`) and the `DynamicVars` its
//! text reads. Display only; nothing here changes the game.

use super::{RelicData, RelicKind};
use crate::combat::{CombatState, HistoryKind, HistoryTag, Side};
use crate::content::cards::CardType;
use crate::content::tables::{self, ModelVar};
use crate::decimal::Decimal;
use crate::run::RunState;

/// `RelicStatus` (`Entities.Relics/RelicStatus.cs`): how the relic's icon shows it. `Active`
/// pulses (it is about to, or is ready to, trigger); `Disabled` is greyed out (used up).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RelicStatus {
    /// Neither.
    #[default]
    Normal,
    /// The icon pulses.
    Active,
    /// The icon is greyed out.
    Disabled,
}

impl RelicKind {
    /// `CanonicalVars` (every var, with `None` for a `StringVar` or a base the relic sets
    /// itself).
    #[must_use]
    pub fn canonical_vars(self) -> &'static [ModelVar] {
        tables::relic_vars(self.id())
    }

    /// `DynamicVars` of an owned relic with `data`, by name, as its text shows them: the
    /// canonical ones plus the bases the relic keeps in step with its fields (the teas'
    /// `Combats`, `WingedBoots.Rooms`, `WongosMysteryTicket.RemainingCombats`). A `StringVar`
    /// has no number and is left out.
    #[must_use]
    pub fn vars(self, data: &RelicData) -> Vec<(&'static str, Decimal)> {
        self.canonical_vars()
            .iter()
            .filter_map(|v| {
                let set = match (data, v.name) {
                    (
                        RelicData::BoneTea(super::bone_tea::Data { combats_left })
                        | RelicData::EmberTea(super::ember_tea::Data { combats_left })
                        | RelicData::TeaOfDiscourtesy(super::tea_of_discourtesy::Data {
                            combats_left,
                        }),
                        "Combats",
                    ) => Some(*combats_left),
                    (RelicData::WingedBoots(d), "Rooms") => Some(3 - d.times_used),
                    // `AfterCombatEnd` sets it; until then it is the canonical 5.
                    (RelicData::WongosMysteryTicket(d), "RemainingCombats")
                        if d.combats_finished > 0 =>
                    {
                        Some((5 - d.combats_finished).max(0))
                    }
                    _ => None,
                };
                Some((v.name, set.map(Decimal::from).or(v.value)?))
            })
            .collect()
    }

    /// The `IntValue` of canonical var `name`.
    fn int_var(self, name: &str) -> i32 {
        self.canonical_vars()
            .iter()
            .find(|v| v.name == name)
            .and_then(|v| v.value)
            .unwrap_or_else(|| panic!("{self:?} has no {name} var"))
            .to_int_truncating()
    }

    /// The number on an owned relic's icon: `DisplayAmount` if `ShowCounter`, for a relic with
    /// `data`; `combat` is the fight in progress, if any. `IsActivating` (the counter's flash)
    /// is never on and an owned relic is never canonical.
    #[must_use]
    pub fn counter(self, data: &RelicData, combat: Option<&CombatState>) -> Option<i32> {
        let in_combat = combat.is_some_and(|c| c.in_progress);
        let turn = combat.map(|c| i32::try_from(c.player.turn_number).unwrap_or(i32::MAX));
        let modulo = |n: i32, var: &str| n % self.int_var(var);
        match (self, data) {
            (Self::BookOfFiveRings, RelicData::BookOfFiveRings(d)) => {
                Some(modulo(d.cards_added, "Cards"))
            }
            (Self::EmberTea, RelicData::EmberTea(d)) => Some(d.combats_left.max(0)),
            (Self::GalacticDust, RelicData::GalacticDust(d)) => {
                Some(modulo(d.stars_spent, "Stars"))
            }
            (Self::Pendulum, RelicData::Pendulum(d)) => Some(d.turns_seen),
            (Self::HappyFlower | Self::FakeHappyFlower, RelicData::HappyFlower(d)) => {
                Some(d.turns_seen)
            }
            (Self::PollinousCore, RelicData::PollinousCore(d)) => Some(d.turns_seen),
            (Self::Girya, RelicData::Girya(d)) => Some(d.times_lifted),
            (Self::SilverCrucible, RelicData::SilverCrucible(d)) => {
                let cards = self.int_var("Cards");
                (cards > 0 && d.times_used < cards).then_some(cards - d.times_used)
            }
            (Self::Nunchaku, RelicData::Nunchaku(d)) => Some(modulo(d.attacks_played, "Cards")),
            (Self::JossPaper, RelicData::JossPaper(d)) => Some(d.cards_exhausted),
            // `Status == Normal` until `AfterSideTurnStart` of turn 3 activates it.
            (Self::PaelsFlesh, _) => turn.filter(|&t| in_combat && t < 3),
            (Self::PenNib, RelicData::PenNib(d)) => Some(d.attacks_played % 10),
            (Self::LetterOpener, RelicData::LetterOpener(d)) => {
                in_combat.then(|| modulo(d.skills_played_this_turn, "Cards"))
            }
            (Self::BrilliantScarf, RelicData::BrilliantScarf(d)) => (in_combat
                && d.cards_played_this_turn < self.int_var("Cards"))
            .then_some(d.cards_played_this_turn),
            (Self::Kunai, RelicData::Kunai(d)) => {
                in_combat.then(|| modulo(d.attacks_played_this_turn, "Cards"))
            }
            (Self::Shuriken, RelicData::Shuriken(d)) => {
                in_combat.then(|| modulo(d.attacks_played_this_turn, "Cards"))
            }
            (Self::OrnamentalFan, RelicData::OrnamentalFan(d)) => {
                in_combat.then(|| modulo(d.attacks_played_this_turn, "Cards"))
            }
            (Self::Kusarigama, RelicData::Kusarigama(d)) => {
                in_combat.then(|| modulo(d.attacks_played_this_turn, "Cards"))
            }
            (Self::Metronome, RelicData::Metronome(d)) => {
                let orbs = self.int_var("OrbCount");
                (in_combat && d.orbs_channeled < orbs).then_some(d.orbs_channeled.min(orbs))
            }
            (Self::TuningFork, RelicData::TuningFork(d)) => Some(d.skills_played),
            (Self::PaelsTooth, RelicData::PaelsTooth(d)) => {
                let n = i32::try_from(d.len()).unwrap_or(i32::MAX);
                (n > 0).then_some(n)
            }
            (Self::PaelsLegion, RelicData::PaelsLegion(d)) => {
                (in_combat && d.cooldown > 0).then_some(d.cooldown)
            }
            (Self::FishingRod, RelicData::FishingRod(d)) => Some(modulo(d.combats_seen, "Combats")),
            (Self::IronClub, RelicData::IronClub(d)) => Some(modulo(d.cards_played, "Cards")),
            (Self::Pocketwatch, RelicData::Pocketwatch(d)) => {
                in_combat.then_some(d.cards_played_this_turn)
            }
            (Self::PumpkinCandle, RelicData::PumpkinCandle(d)) => Some(d.kindle_count),
            (Self::PaelsWing, RelicData::PaelsWing(d)) => {
                Some(modulo(d.rewards_sacrificed, "Sacrifices"))
            }
            (Self::ToyBox, RelicData::ToyBox(d)) => {
                (!self.is_used_up(data)).then(|| modulo(d.combats_seen, "Combats"))
            }
            (Self::StoneCalendar, _) => {
                turn.filter(|&t| in_combat && t < self.int_var("DamageTurn"))
            }
            (Self::SwordOfStone, RelicData::SwordOfStone(d)) => Some(d.elites_defeated),
            (Self::VelvetChoker, RelicData::VelvetChoker(d)) => {
                in_combat.then_some(d.cards_played_this_turn)
            }
            (Self::WingedBoots, RelicData::WingedBoots(d)) => {
                (!self.is_used_up(data)).then_some(3 - d.times_used)
            }
            (Self::WongosMysteryTicket, RelicData::WongosMysteryTicket(d)) => {
                Some(5 - d.combats_finished).filter(|&n| n > 0)
            }
            (Self::LastingCandy, RelicData::LastingCandy(d)) => Some(d.combat_rewards_seen % 2),
            _ => None,
        }
    }

    /// The `Status` of an owned relic with `data` in `run`, as its icon shows it; `combat` is
    /// the fight in progress, if any. The game stores `Status` and sets it in hooks; the engine
    /// does not, so this derives what the last hook left from the fields the relic keeps and the
    /// fight's or run's state. `IsActivating` (the activation flash) is never on, so it counts
    /// as `Normal`. Every `Normal` reset the C# does at the end of a combat holds out of one.
    /// Not derivable: `RegalPillow` (`Active` in a rest site until its heal is taken, which
    /// only the run's rest-site phase knows) is always `Normal`.
    #[must_use]
    #[expect(clippy::too_many_lines, reason = "one arm per relic, like `counter`")]
    pub fn status(
        self,
        data: &RelicData,
        combat: Option<&CombatState>,
        run: &RunState,
    ) -> RelicStatus {
        use RelicStatus::{Active, Disabled, Normal};
        let in_combat = combat.is_some_and(|c| c.in_progress);
        let turn = combat.map_or(0, |c| {
            i32::try_from(c.player.turn_number).unwrap_or(i32::MAX)
        });
        // A player turn has started in the fight in progress (`AfterSideTurnStart` ran).
        let started = in_combat && turn >= 1;
        let player_side = combat.is_some_and(|c| c.side == Side::Player);
        let status = |active: bool| if active { Active } else { Normal };
        // `x % n == n - 1` over the `Cards` var.
        let before_nth = |count: i32| {
            let n = self.int_var("Cards");
            n > 0 && count % n == n - 1
        };
        let (hp, max_hp) = combat.map_or((0, 0), |c| {
            let owner = c.creature(c.player.creature);
            (owner.hp, owner.max_hp)
        });
        match (self, data) {
            (Self::UnsettlingLamp, RelicData::UnsettlingLamp(d)) => {
                status(in_combat && !d.is_finished_triggering)
            }
            // `AfterRoomEntered`: the current point is a parent of the boss point.
            (Self::Pantograph, _) => status(
                run.coord()
                    .is_some_and(|here| run.map.boss().parents.contains(&here)),
            ),
            (Self::HappyFlower | Self::FakeHappyFlower, RelicData::HappyFlower(d)) => {
                status(started && d.turns_seen == self.int_var("Turns") - 1)
            }
            (Self::Pendulum, RelicData::Pendulum(d)) => {
                status(started && d.turns_seen == self.int_var("Turns") - 1)
            }
            (Self::PollinousCore, RelicData::PollinousCore(d)) => {
                status(started && d.turns_seen == self.int_var("Turns") - 1)
            }
            (Self::PenNib, RelicData::PenNib(d)) => status(d.attacks_played == 9),
            // Below the `HpThreshold` percent of max HP, in a fight.
            (Self::RedSkull, _) => {
                status(in_combat && hp * 100 <= max_hp * self.int_var("HpThreshold"))
            }
            (Self::MeatOnTheBone, _) => {
                status(in_combat && hp <= max_hp * self.int_var("HpThreshold") / 100)
            }
            (Self::RainbowRing, RelicData::RainbowRing(d)) => {
                status(in_combat && d.activations > 0)
            }
            (Self::BeltBuckle, _) => status(combat.is_some_and(|c| {
                c.in_progress
                    && c.player.potions.iter().all(Option::is_none)
                    && c.player.unported_potions.iter().all(Option::is_none)
            })),
            (Self::IronClub, RelicData::IronClub(d)) => status(before_nth(d.cards_played)),
            (Self::Nunchaku, RelicData::Nunchaku(d)) => status(before_nth(d.attacks_played)),
            // Active from the start of the fight until the first block is gained.
            (Self::Vambrace, RelicData::Vambrace(d)) => {
                status(in_combat && d.triggering_card.is_none())
            }
            // From the start of the owner's turn until an Attack is played in it.
            (Self::RippleBasin, _) => status(
                started
                    && player_side
                    && !combat.is_some_and(|c| {
                        c.history_this_turn(HistoryTag::CardPlayFinished)
                            .iter()
                            .any(|e| {
                                matches!(
                                    e.kind,
                                    HistoryKind::CardPlayFinished {
                                        card_type: CardType::Attack,
                                        ..
                                    }
                                )
                            })
                    }),
            ),
            (Self::VenerableTeaSet | Self::FakeVenerableTeaSet, RelicData::VenerableTeaSet(d)) => {
                status(d.gain_energy_in_next_combat)
            }
            (Self::LetterOpener, RelicData::LetterOpener(d)) => {
                status(in_combat && before_nth(d.skills_played_this_turn))
            }
            (Self::ThrowingAxe, RelicData::ThrowingAxe(d)) => {
                status(in_combat && !d.used_this_combat)
            }
            // From the start of the `DamageTurn`th turn until it ends.
            (Self::StoneCalendar, _) => {
                status(started && player_side && turn == self.int_var("DamageTurn"))
            }
            (Self::Kunai, RelicData::Kunai(d)) => {
                status(in_combat && before_nth(d.attacks_played_this_turn))
            }
            (Self::Shuriken, RelicData::Shuriken(d)) => {
                status(in_combat && before_nth(d.attacks_played_this_turn))
            }
            (Self::OrnamentalFan, RelicData::OrnamentalFan(d)) => {
                status(in_combat && before_nth(d.attacks_played_this_turn))
            }
            (Self::Kusarigama, RelicData::Kusarigama(d)) => {
                status(in_combat && before_nth(d.attacks_played_this_turn))
            }
            (Self::BurningSticks, RelicData::BurningSticks(d)) => {
                status(in_combat && !d.was_used_this_combat)
            }
            (Self::JossPaper, RelicData::JossPaper(d)) => {
                status(d.cards_exhausted == self.int_var("ExhaustAmount") - 1)
            }
            // Active once the cooldown has run out at a turn start.
            (Self::PaelsLegion, RelicData::PaelsLegion(d)) => status(started && d.cooldown <= 0),
            (Self::TuningFork, RelicData::TuningFork(d)) => {
                status(d.skills_played == self.int_var("Cards") - 1)
            }
            (Self::Metronome, RelicData::Metronome(d)) => {
                status(in_combat && d.orbs_channeled == self.int_var("OrbCount") - 1)
            }
            (Self::GalacticDust, RelicData::GalacticDust(d)) => {
                status(d.stars_spent == self.int_var("Stars") - 1)
            }
            (Self::Pocketwatch, RelicData::Pocketwatch(d)) => {
                status(started && d.cards_played_this_turn <= self.int_var("CardThreshold"))
            }
            (Self::BrilliantScarf, RelicData::BrilliantScarf(d)) => {
                status(in_combat && d.cards_played_this_turn == self.int_var("Cards") - 1)
            }
            // From the owner's turn start until a (non-auto) card is played in it; on the
            // enemies' turn only a turn in which one was played has not already ended.
            (Self::PaelsEye, RelicData::PaelsEye(d)) => status(
                started
                    && player_side
                    && !d.used_this_combat
                    && d.was_owner_part_of_last_player_turn
                    && !combat.is_some_and(|c| {
                        c.history_this_turn(HistoryTag::CardPlayFinished)
                            .iter()
                            .any(|e| {
                                e.actor == c.player.creature
                                    && matches!(
                                        &e.kind,
                                        HistoryKind::CardPlayFinished { play, .. }
                                            if !play.is_auto_play
                                    )
                            })
                    }),
            ),
            // From the third turn on.
            (Self::PaelsFlesh, _) => status(started && turn >= 3),
            // Active from each turn start until an Attack is played in it (and through the
            // enemies' turn after one was not).
            (Self::ArtOfWar, RelicData::ArtOfWar(d)) => status(
                started && !d.any_attacks_played_this_turn && !d.any_attacks_played_last_turn,
            ),
            (Self::EmotionChip, RelicData::EmotionChip(d)) => status(
                in_combat
                    && d.last_unblocked_hit_turn
                        .is_some_and(|t| i32::try_from(t) == Ok(turn)),
            ),
            (Self::EmberTea, RelicData::EmberTea(d)) if d.combats_left <= 0 => Disabled,
            (Self::BoneTea, RelicData::BoneTea(d)) if d.combats_left <= 0 => Disabled,
            (Self::TeaOfDiscourtesy, RelicData::TeaOfDiscourtesy(d)) if d.combats_left <= 0 => {
                Disabled
            }
            (
                Self::WingedBoots
                | Self::LizardTail
                | Self::MawBank
                | Self::ToyBox
                | Self::WongosMysteryTicket
                | Self::SilkenTress,
                _,
            ) if self.is_used_up(data) => Disabled,
            (Self::LavaRock, RelicData::LavaRock(d)) if d.has_triggered => Disabled,
            (Self::PaelsTooth, RelicData::PaelsTooth(d)) if d.is_empty() => Disabled,
            (Self::SilverCrucible, RelicData::SilverCrucible(d))
                if d.times_used >= self.int_var("Cards") && d.treasure_rooms_entered > 0 =>
            {
                Disabled
            }
            (Self::PumpkinCandle, RelicData::PumpkinCandle(d)) if d.kindle_count <= 0 => Disabled,
            _ => Normal,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::relics::{kunai, winged_boots};

    #[test]
    fn counters_follow_show_counter() {
        let kunai = RelicData::Kunai(kunai::Data {
            attacks_played_this_turn: 4,
        });
        // `ShowCounter => CombatManager.Instance.IsInProgress`; `% Cards` (3).
        assert_eq!(RelicKind::Kunai.counter(&kunai, None), None);
        let boots = |times_used| RelicData::WingedBoots(winged_boots::Data { times_used });
        assert_eq!(RelicKind::WingedBoots.counter(&boots(1), None), Some(2));
        assert_eq!(RelicKind::WingedBoots.counter(&boots(3), None), None);
        assert_eq!(RelicKind::Anchor.counter(&RelicData::None, None), None);
    }

    #[test]
    fn vars_track_the_fields_they_mirror() {
        let boots = RelicData::WingedBoots(winged_boots::Data { times_used: 1 });
        assert_eq!(
            RelicKind::WingedBoots.vars(&boots),
            vec![("Rooms", Decimal::from(2))]
        );
        assert_eq!(
            RelicKind::Anchor.vars(&RelicData::None),
            vec![("Block", Decimal::from(10))]
        );
    }
}
