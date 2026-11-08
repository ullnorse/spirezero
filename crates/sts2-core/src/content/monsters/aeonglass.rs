//! `Aeonglass` (`Models.Monsters/Aeonglass.cs`): starts with a `WitheringPresence` aimed at each
//! opponent and 3 `Artifact`, then cycles Ebb (attack, 33 Block) -> Eye Lasers (two hits) ->
//! Increasing Intensity (every `Wither` of the targets is fake-upgraded, then `Wither`s into
//! their discard piles and Strength that grows by one per use). Every `Wither` generated for the
//! combat is fake-upgraded as often as Increasing Intensity was used. The music and the
//! animations are visual.

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{DEADLY_ENEMIES, MonsterKind, MonsterModel, TOUGH_ENEMIES, update_kind};
use super::{add_status_cards, attack_players, attack_players_hits, if_ascension};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CombatState, CreatureId, Engine, Flow, ValueProps};
use crate::content::cards::{CardKind, wither};
use crate::content::powers::tangled::all_cards;
use crate::content::powers::{PowerKind, withering_presence};
use crate::decimal::Decimal;

const EBB: StateIdx = 0;
const EYE_LASERS: StateIdx = 1;
const INCREASING_INTENSITY: StateIdx = 2;

/// `EbbBlock`.
const EBB_BLOCK: i32 = 33;
/// `EyeLasersRepeat`.
const EYE_LASERS_REPEAT: u32 = 2;
/// The `WitheringPresencePower` amount.
const WITHERING_PRESENCE: i32 = 6;
/// The `ArtifactPower` amount.
const ARTIFACT: i32 = 3;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "EBB_MOVE",
            follow_up: EYE_LASERS,
        },
        StateDef::Move {
            id: "EYE_LASERS_MOVE",
            follow_up: INCREASING_INTENSITY,
        },
        StateDef::Move {
            id: "INCREASING_INTENSITY_MOVE",
            follow_up: EBB,
        },
    ],
    initial: EBB,
};

/// Per-combat fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Aeonglass {
    /// `_additionalStrength`.
    pub additional_strength: i32,
    /// `_witherUpgradeCount`.
    pub wither_upgrade_count: u32,
}

fn fields(s: &CombatState, id: CreatureId) -> Aeonglass {
    match s.creature(id).monster.as_ref().map(|m| m.kind) {
        Some(MonsterKind::Aeonglass(a)) => a,
        _ => unreachable!("the owner is an Aeonglass"),
    }
}

fn update(eng: &mut Engine<'_>, id: CreatureId, f: impl FnOnce(&mut Aeonglass)) {
    update_kind(eng, id, |kind| match kind {
        MonsterKind::Aeonglass(a) => f(a),
        _ => unreachable!("the owner is an Aeonglass"),
    });
}

/// `MatchWitherToUpgradeCount`: `WitherUpgradeCount` fake upgrades.
fn match_wither_to_upgrade_count(eng: &mut Engine<'_>, me: CreatureId, card: CardId) {
    for _ in 0..fields(eng.s, me).wither_upgrade_count {
        wither::fake_upgrade(eng.s, card);
    }
}

#[sts2_macros::hooks]
impl Hooks for Aeonglass {
    type Me = CreatureId;

    fn after_card_generated_for_combat(
        &self,
        eng: &mut Engine<'_>,
        me: CreatureId,
        card: CardId,
        _creator: Option<CreatureId>,
    ) -> Flow {
        if eng.s.card(card).kind == CardKind::Wither {
            match_wither_to_upgrade_count(eng, me, card);
        }
        Ok(())
    }
}

impl MonsterModel for Aeonglass {
    fn name(&self) -> &'static str {
        "Aeonglass"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 535, 512)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        self.min_initial_hp(ascension)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    /// `GetOpponentsOf(Creature)`: the player and its pets (Osty).
    fn after_added_to_room(&self, eng: &mut Engine<'_>, me: CreatureId) -> Flow {
        let opponents = eng.s.allies.clone();
        for target in opponents {
            withering_presence::apply(eng, me, target, Decimal::from(WITHERING_PRESENCE))?;
        }
        eng.apply_power(me, PowerKind::Artifact, Decimal::from(ARTIFACT), Some(me))
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        let ascension = eng.s.ascension;
        match state {
            EBB => {
                attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 26, 22))?;
                eng.gain_block(me, Decimal::from(EBB_BLOCK), ValueProps::MOVE, None)?;
                Ok(())
            }
            EYE_LASERS => {
                let damage = if_ascension(ascension, DEADLY_ENEMIES, 12, 11);
                attack_players_hits(eng, me, damage, EYE_LASERS_REPEAT)
            }
            INCREASING_INTENSITY => {
                // Every target's `PlayerCombatState.AllCards`: the only player's cards.
                if !eng.s.player_creatures().is_empty() {
                    for card in all_cards(eng.s) {
                        if eng.s.card(card).kind == CardKind::Wither {
                            wither::fake_upgrade(eng.s, card);
                        }
                    }
                }
                update(eng, me, |a| a.wither_upgrade_count += 1);
                let withers = if_ascension(ascension, DEADLY_ENEMIES, 2, 1);
                add_status_cards(eng, CardKind::Wither, u32::try_from(withers).unwrap_or(0))?;
                let base = if_ascension(ascension, DEADLY_ENEMIES, 4, 3);
                let strength = base + fields(eng.s, me).additional_strength;
                eng.apply_power(me, PowerKind::Strength, Decimal::from(strength), Some(me))?;
                update(eng, me, |a| a.additional_strength += 1);
                Ok(())
            }
            _ => unreachable!("not an Aeonglass move: {state}"),
        }
    }
}
