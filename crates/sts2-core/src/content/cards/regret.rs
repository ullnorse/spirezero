//! `Regret` (`Models.Cards/Regret.cs`): curse, unplayable. It counts the cards in its pile when
//! the player's side turn ends (`BeforeSideTurnEnd`, only while it is in hand); then, with it
//! still in hand at `OnTurnEndInHand`, the owner takes that many unblockable, unpowered damage.

use crate::combat::hooks::Hooks;
use crate::combat::state::PileType;
use crate::combat::{CardId, CardPlay, CreatureId, Engine, Flow, Side, ValueProps};
use crate::content::cards::{
    CardData, CardKeyword, CardModel, CardRarity, CardType, Keywords, TargetType,
};
use crate::decimal::Decimal;

pub struct Regret;

/// `_cardsInHand`.
fn cards_in_hand(eng: &Engine<'_>, me: CardId) -> i32 {
    match eng.s.card(me).data {
        CardData::Regret(count) => count,
        _ => 0,
    }
}

#[sts2_macros::hooks]
impl Hooks for Regret {
    type Me = CardId;

    fn before_side_turn_end(
        &self,
        eng: &mut Engine<'_>,
        me: CardId,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if !participants.contains(&eng.s.player.creature) {
            return Ok(());
        }
        if eng.s.pile_of(me) != Some(PileType::Hand) {
            return Ok(());
        }
        // Pile.Cards.Count: the whole hand, this card included.
        let count = i32::try_from(eng.s.player.hand.len()).expect("a hand fits in i32");
        eng.s.card_mut(me).data = CardData::Regret(count);
        Ok(())
    }
}

impl CardModel for Regret {
    fn name(&self) -> &'static str {
        "Regret"
    }

    fn cost(&self) -> i32 {
        -1
    }

    fn card_type(&self) -> CardType {
        CardType::Curse
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Curse
    }

    fn target(&self) -> TargetType {
        TargetType::None
    }

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Unplayable)
    }

    fn max_upgrade_level(&self) -> u8 {
        0
    }

    fn on_play(&self, _eng: &mut Engine<'_>, _play: &CardPlay) -> Flow {
        Ok(())
    }

    fn has_turn_end_in_hand_effect(&self) -> bool {
        true
    }

    /// `CreatureCmd.Damage(Owner.Creature, CardsInHand, Unblockable | Unpowered | Move, this,
    /// null)`, then `CardsInHand = 0`.
    fn on_turn_end_in_hand(&self, eng: &mut Engine<'_>, me: CardId) -> Flow {
        let owner = eng.s.player.creature;
        let count = cards_in_hand(eng, me);
        eng.damage(
            &[owner],
            Decimal::from(count),
            ValueProps::UNBLOCKABLE
                .with(ValueProps::UNPOWERED)
                .with(ValueProps::MOVE),
            Some(owner),
            Some(me),
        )?;
        eng.s.card_mut(me).data = CardData::Regret(0);
        Ok(())
    }
}
