//! `ThievingHopper` (`Models.Monsters/ThievingHopper.cs`): gains `EscapeArtistPower` (5) when
//! added, then Thievery (steals a card from the run deck, then attacks), Flutter
//! (`FlutterPower` 5), Hat Trick, Nab and Escape, which it repeats. Thievery takes, from the
//! draw and discard piles' deck cards, a random card (`Rng.CombatCardGeneration`) of the first
//! non-empty priority: Uncommon; Common, Rare or Event; Basic or Quest (all not `Imbued`);
//! Ancient or `Imbued`; else any. The card leaves combat and the deck and is held by a
//! `SwipePower`, which returns it as a reward if the hopper dies. `IsHovering` and the stolen
//! card's display are visual.
//!
//! `Imbued` is the only enchantment that `ShouldStartAtBottomOfDrawPile` (`flags/enchantments.tsv`),
//! so the `Imbued` tests read that flag.

use super::machine::{MoveTable, StateDef, StateIdx};
use super::{DEADLY_ENEMIES, MonsterModel, TOUGH_ENEMIES, attack_players, if_ascension};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CombatState, CreatureId, Engine, Flow};
use crate::content::cards::CardRarity;
use crate::content::powers::{PowerKind, swipe};
use crate::decimal::Decimal;
use crate::rng::RunRngType;

const THIEVERY: StateIdx = 0;
const NAB: StateIdx = 1;
const HAT_TRICK: StateIdx = 2;
const FLUTTER: StateIdx = 3;
const ESCAPE: StateIdx = 4;

/// The `EscapeArtistPower` and `FlutterPower` amounts.
const ESCAPE_ARTIST_AMOUNT: i32 = 5;
const FLUTTER_AMOUNT: i32 = 5;

static MOVES: MoveTable = MoveTable {
    states: &[
        StateDef::Move {
            id: "THIEVERY_MOVE",
            follow_up: FLUTTER,
        },
        StateDef::Move {
            id: "NAB_MOVE",
            follow_up: ESCAPE,
        },
        StateDef::Move {
            id: "HAT_TRICK_MOVE",
            follow_up: NAB,
        },
        StateDef::Move {
            id: "FLUTTER_MOVE",
            follow_up: HAT_TRICK,
        },
        StateDef::Move {
            id: "ESCAPE_MOVE",
            follow_up: ESCAPE,
        },
    ],
    initial: THIEVERY,
};

/// `c.Enchantment is Imbued`: the card's enchantment `ShouldStartAtBottomOfDrawPile`.
fn is_imbued(s: &CombatState, card: CardId) -> bool {
    s.card(card)
        .enchantment
        .is_some_and(|e| e.kind.should_start_at_bottom_of_draw_pile())
}

/// `_stealPriorities[priority](card)`.
fn has_priority(s: &CombatState, card: CardId, priority: usize) -> bool {
    let rarity = s.card(card).kind.rarity();
    let imbued = is_imbued(s, card);
    match priority {
        0 => !imbued && rarity == CardRarity::Uncommon,
        1 => {
            !imbued
                && matches!(
                    rarity,
                    CardRarity::Common | CardRarity::Rare | CardRarity::Event
                )
        }
        2 => !imbued && matches!(rarity, CardRarity::Basic | CardRarity::Quest),
        _ => rarity == CardRarity::Ancient || imbued,
    }
}

/// The card Thievery steals from the player, if any.
fn card_to_steal(s: &mut CombatState) -> Option<CardId> {
    let list: Vec<CardId> = s
        .player
        .draw
        .iter()
        .chain(&s.player.discard)
        .copied()
        .filter(|&c| s.card(c).deck_version().is_some())
        .collect();
    let options = (0..4)
        .map(|p| -> Vec<CardId> {
            list.iter()
                .copied()
                .filter(|&c| has_priority(s, c, p))
                .collect()
        })
        .find(|cards| !cards.is_empty())
        .unwrap_or(list);
    s.rng[RunRngType::CombatCardGeneration]
        .next_item(&options)
        .copied()
}

/// `ThievingHopper`'s per-combat fields: none.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ThievingHopper;

#[sts2_macros::hooks]
impl Hooks for ThievingHopper {
    type Me = CreatureId;
}

impl MonsterModel for ThievingHopper {
    fn name(&self) -> &'static str {
        "Thieving Hopper"
    }

    fn min_initial_hp(&self, ascension: u8) -> i32 {
        if_ascension(ascension, TOUGH_ENEMIES, 84, 79)
    }

    fn max_initial_hp(&self, ascension: u8) -> i32 {
        self.min_initial_hp(ascension)
    }

    fn moves(&self) -> &'static MoveTable {
        &MOVES
    }

    fn after_added_to_room(&self, eng: &mut Engine<'_>, me: CreatureId) -> Flow {
        let amount = Decimal::from(ESCAPE_ARTIST_AMOUNT);
        eng.apply_power(me, PowerKind::EscapeArtist, amount, Some(me))
    }

    fn perform_move(&self, eng: &mut Engine<'_>, me: CreatureId, state: StateIdx) -> Flow {
        let ascension = eng.s.ascension;
        match state {
            THIEVERY => {
                let mut stolen = Vec::new();
                for target in eng.s.player_creatures() {
                    if eng.s.creature(target).is_dead() {
                        continue;
                    }
                    if let Some(card) = card_to_steal(eng.s) {
                        eng.remove_from_combat(&[card])?;
                        stolen.push(card);
                    }
                }
                for card in stolen {
                    swipe::steal(eng, me, card)?;
                }
                attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 19, 17))
            }
            NAB => attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 16, 14)),
            HAT_TRICK => attack_players(eng, me, if_ascension(ascension, DEADLY_ENEMIES, 23, 21)),
            FLUTTER => {
                let amount = Decimal::from(FLUTTER_AMOUNT);
                eng.apply_power(me, PowerKind::Flutter, amount, Some(me))
            }
            ESCAPE => {
                eng.escape(me);
                Ok(())
            }
            _ => unreachable!("not a ThievingHopper move: {state}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::state::Card;
    use crate::content::cards::CardKind;
    use crate::content::enchantments::{Enchantment, EnchantmentKind};

    #[test]
    fn imbued_cards_are_stolen_last() {
        // ThievingHopper.cs:33-68: an Imbued Uncommon is in no priority but the Ancient-or-Imbued
        // one; a plain Uncommon is in the first.
        let mut s =
            crate::combat::Combat::new(crate::combat::CombatConfig::necrobinder("94LYYA8YDC", 0))
                .state()
                .clone();
        let id = CardId(u32::try_from(s.cards.len()).expect("fits"));
        s.cards.push(Card::new(id, CardKind::Backstab, 0));
        assert_eq!(s.card(id).kind.rarity(), CardRarity::Uncommon);
        assert!(!is_imbued(&s, id));
        assert!(has_priority(&s, id, 0));
        s.card_mut(id).enchantment = Some(Enchantment::new(EnchantmentKind::TestImbued, 1));
        assert!(is_imbued(&s, id));
        assert_eq!((0..4).find(|&p| has_priority(&s, id, p)), Some(3));
    }
}
