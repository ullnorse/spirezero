//! `Bolas` (`Models.Cards/Bolas.cs`): cost 0; 3 damage (4 upgraded). At the start of each player
//! turn, if it finished a play last turn, it returns to the hand from wherever it is.
//!
//! The game reads `CardPlaysFinished` with `HappenedLastPlayerTurn`, which the combat history does
//! not index (it keeps this turn's entries and combat-wide counts). The card keeps the player turn
//! number of its latest finished play instead: a play last turn is a latest play last turn, since
//! none can come later before the hand draw. A clone (`CreateClone`) would copy that field; no
//! in-scope effect clones a Bolas.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets, Position};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, CombatState, Engine, Flow, PileType};
use crate::content::cards::{CardData, CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct Bolas;

/// `Bolas`'s tracking of its finished plays.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// The player's `TurnNumber` at its latest finished play; 0 before the first.
    pub last_finished_turn: u32,
}

fn data(s: &CombatState, me: CardId) -> Data {
    match s.card(me).data {
        CardData::Bolas(data) => data,
        CardData::None => Data::default(),
        _ => unreachable!("a Bolas holds Bolas data"),
    }
}

#[sts2_macros::hooks]
impl Hooks for Bolas {
    type Me = CardId;

    fn after_card_played(&self, eng: &mut Engine<'_>, me: CardId, play: &CardPlay) -> Flow {
        // The history entry is `CardPlayFinishedEntry` of this card.
        if play.card == me {
            let last_finished_turn = eng.s.player.turn_number;
            eng.s.card_mut(me).data = CardData::Bolas(Data { last_finished_turn });
        }
        Ok(())
    }

    fn before_hand_draw(&self, eng: &mut Engine<'_>, me: CardId) -> Flow {
        let last = data(eng.s, me).last_finished_turn;
        // `HappenedLastPlayerTurn`: the entry's turn is `TurnNumber - 1`.
        if last != 0
            && last + 1 == eng.s.player.turn_number
            && eng.s.pile_of(me) != Some(PileType::Hand)
        {
            eng.add_to_pile(me, PileType::Hand, Position::Bottom)?;
        }
        Ok(())
    }
}

impl CardModel for Bolas {
    fn name(&self) -> &'static str {
        "Bolas"
    }

    fn cost(&self) -> i32 {
        0
    }

    fn card_type(&self) -> CardType {
        CardType::Attack
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Rare
    }

    fn target(&self) -> TargetType {
        TargetType::AnyEnemy
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Damage, 3).upgrade(1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Bolas needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        Ok(())
    }
}
