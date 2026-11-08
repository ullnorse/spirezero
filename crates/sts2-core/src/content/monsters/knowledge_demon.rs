//! `KnowledgeDemon` (`Models.Monsters/KnowledgeDemon.cs`): opens with Curse of Knowledge, then
//! Slap (attack) -> Knowledge Overwhelming (three hits) -> Ponder (attack, heals 30 per player,
//! Strength) -> Curse of Knowledge again while it has been cast fewer than three times, else Slap.
//! Curse of Knowledge makes each target's player choose one of two cards on the enemy turn
//! (`FromChooseACardScreen`, skippable): `Disintegration` (6, 7 or 8 damage, by the cast) and,
//! by the cast, `MindRot`, `Sloth` or `WasteAway`. The chosen card is never added to a pile; its
//! `OnChosen` applies its power. `IsBurnt`, the talk lines and the music are visual.

use super::machine::{MoveTable, StateDef, StateIdx, when};
use super::{DEADLY_ENEMIES, MonsterKind, MonsterModel, TOUGH_ENEMIES, update_kind};
use super::{attack_players, attack_players_hits, if_ascension};
use crate::combat::hooks::Hooks;
use crate::combat::{CombatState, CreatureId, Engine, Flow, PromptSource};
use crate::content::cards::{self, CardKind};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

const CURSE_OF_KNOWLEDGE_BRANCH: StateIdx = 0;
const CURSE_OF_KNOWLEDGE: StateIdx = 1;
const SLAP: StateIdx = 2;
const PONDER: StateIdx = 3;
const KNOWLEDGE_OVERWHELMING: StateIdx = 4;

/// `_knowledgeOverwhelmingRepeat`.
const KNOWLEDGE_OVERWHELMING_REPEAT: u32 = 3;
/// `_ponderHeal`, per player.
const PONDER_HEAL: i32 = 30;
/// `_disintegrationDamageValues`, by cast.
const DISINTEGRATION_DAMAGE: [i32; 3] = [6, 7, 8];
/// `_curseOfKnowledgeSets`' second card, by cast (the first is always `Disintegration`).
const CURSE_SETS: [CardKind; 3] = [CardKind::MindRot, CardKind::Sloth, CardKind::WasteAway];

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Conditional {
            id: "CurseOfKnowledgeBranch",
            branches: &[
                when(CURSE_OF_KNOWLEDGE, curses_left),
                when(SLAP, no_curses_left),
            ],
        },
        StateDef::Move {
            id: "CURSE_OF_KNOWLEDGE_MOVE",
            follow_up: SLAP,
        },
        StateDef::Move {
            id: "SLAP_MOVE",
            follow_up: KNOWLEDGE_OVERWHELMING,
        },
        StateDef::Move {
            id: "PONDER_MOVE",
            follow_up: CURSE_OF_KNOWLEDGE_BRANCH,
        },
        StateDef::Move {
            id: "KNOWLEDGE_OVERWHELMING_MOVE",
            follow_up: PONDER,
        },
    ],
    initial: CURSE_OF_KNOWLEDGE,
};

/// Per-combat fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct KnowledgeDemon {
    /// `_curseOfKnowledgeCounter`: Curse of Knowledge casts so far.
    pub curse_of_knowledge_counter: u8,
}

fn counter(s: &CombatState, id: CreatureId) -> usize {
    match s.creature(id).monster.as_ref().map(|m| m.kind) {
        Some(MonsterKind::KnowledgeDemon(k)) => usize::from(k.curse_of_knowledge_counter),
        _ => unreachable!("the owner is a KnowledgeDemon"),
    }
}

fn curses_left(s: &CombatState, id: CreatureId) -> bool {
    counter(s, id) < CURSE_SETS.len()
}

fn no_curses_left(s: &CombatState, id: CreatureId) -> bool {
    !curses_left(s, id)
}

/// `CurseOfKnowledgeMove`: `ChooseCurse` for each target, then the counter goes up
/// (`IsLiveCombat` is always true for a real combat).
fn curse_of_knowledge(eng: &mut Engine<'_>, me: CreatureId) -> Flow {
    let index = counter(eng.s, me);
    assert!(
        index < CURSE_SETS.len(),
        "There are no valid sets at this index {index}"
    );
    for target in eng.s.player_creatures() {
        choose_curse(eng, me, target, index)?;
    }
    update_kind(eng, me, |kind| match kind {
        MonsterKind::KnowledgeDemon(k) => k.curse_of_knowledge_counter += 1,
        _ => unreachable!("the owner is a KnowledgeDemon"),
    });
    Ok(())
}

/// `ChooseCurse(target)`: the set's cards are created for the target's player (`CreateCard`;
/// `Disintegration`'s `DisintegrationPower` var set to the cast's damage), then one may be
/// chosen and its `OnChosen` runs.
fn choose_curse(eng: &mut Engine<'_>, me: CreatureId, target: CreatureId, index: usize) -> Flow {
    if eng.s.creature(target).is_dead() {
        return Ok(());
    }
    let damage = Decimal::from(DISINTEGRATION_DAMAGE[index]);
    let disintegration = eng.create_card(CardKind::Disintegration);
    let other = eng.create_card(CURSE_SETS[index]);
    let Some(chosen) = eng.choose_a_card(PromptSource::Monster(me), vec![disintegration, other])?
    else {
        return Ok(());
    };
    match eng.s.card(chosen).kind {
        CardKind::Disintegration => cards::disintegration::on_chosen(eng, chosen, damage),
        CardKind::MindRot => cards::mind_rot::on_chosen(eng, chosen),
        CardKind::Sloth => cards::sloth::on_chosen(eng, chosen),
        CardKind::WasteAway => cards::waste_away::on_chosen(eng, chosen),
        kind => unreachable!("not a Curse of Knowledge card: {kind:?}"),
    }
}

#[sts2_macros::hooks]
impl Hooks for KnowledgeDemon {
    type Me = CreatureId;
}

impl MonsterModel for KnowledgeDemon {
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Knowledge Demon"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 399, 379)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        self.min_initial_hp(ascension)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        let ascension = eng.s.ascension;
        match state {
            CURSE_OF_KNOWLEDGE => curse_of_knowledge(eng, me),
            SLAP => attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 18, 17)),
            KNOWLEDGE_OVERWHELMING => {
                let damage = if_ascension(ascension, DEADLY_ENEMIES, 9, 8);
                attack_players_hits(eng, me, damage, KNOWLEDGE_OVERWHELMING_REPEAT)
            }
            PONDER => {
                attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 13, 11))?;
                let players =
                    i32::try_from(eng.s.player_creatures().len()).expect("a handful of players");
                eng.heal(me, Decimal::from(PONDER_HEAL * players))?;
                let amount = if_ascension(ascension, DEADLY_ENEMIES, 3, 2);
                eng.apply_power(me, PowerKind::Strength, Decimal::from(amount), Some(me))
            }
            _ => unreachable!("not a KnowledgeDemon move: {state}"),
        }
    }
}
