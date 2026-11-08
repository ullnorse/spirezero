//! `ThrummingHatchet` (`Models.Cards/ThrummingHatchet.cs`): cost 1; 11 damage (14 upgraded). At the
//! start of each player turn, if it finished a play last turn, it returns to the hand from
//! wherever it is.
//!
//! Same tracking as `Bolas` (see there): the card keeps the player turn number of its latest
//! finished play, in the shared [`bolas::Data`] held by `CardData::Bolas`.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets, Position};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, CombatState, Engine, Flow, PileType};
use crate::content::cards::{
    CardData, CardModel, CardRarity, CardType, CardVar, TargetType, Var, bolas,
};

pub struct ThrummingHatchet;

fn last_finished_turn(s: &CombatState, me: CardId) -> u32 {
    match s.card(me).data {
        CardData::Bolas(data) => data.last_finished_turn,
        _ => 0,
    }
}

#[sts2_macros::hooks]
impl Hooks for ThrummingHatchet {
    type Me = CardId;

    fn after_card_played(&self, eng: &mut Engine<'_>, me: CardId, play: &CardPlay) -> Flow {
        // The history entry is `CardPlayFinishedEntry` of this card.
        if play.card == me {
            let last_finished_turn = eng.s.player.turn_number;
            eng.s.card_mut(me).data = CardData::Bolas(bolas::Data { last_finished_turn });
        }
        Ok(())
    }

    fn before_hand_draw(&self, eng: &mut Engine<'_>, me: CardId) -> Flow {
        let last = last_finished_turn(eng.s, me);
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

impl CardModel for ThrummingHatchet {
    fn name(&self) -> &'static str {
        "Thrumming Hatchet"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Attack
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Uncommon
    }

    fn target(&self) -> TargetType {
        TargetType::AnyEnemy
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Damage, 11).upgrade(3)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("ThrummingHatchet needs a target");
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
