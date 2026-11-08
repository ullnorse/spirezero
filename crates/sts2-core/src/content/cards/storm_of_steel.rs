//! `StormOfSteel` (`Models.Cards/StormOfSteel.cs`): cost 1, Skill, Rare; discards the whole hand
//! (Sly cards autoplay), then creates that many Shivs in the hand, upgraded if this card is.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::shiv;
use crate::content::cards::{CardModel, CardRarity, CardType, TargetType};

pub struct StormOfSteel;

#[sts2_macros::hooks]
impl Hooks for StormOfSteel {
    type Me = CardId;
}

impl CardModel for StormOfSteel {
    fn name(&self) -> &'static str {
        "Storm of Steel"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Rare
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let hand = eng.s.player.hand.to_vec();
        let hand_size = hand.len();
        eng.discard(&hand)?;
        let shivs = shiv::create_in_hand(eng, hand_size)?;
        if eng.s.card(play.card).is_upgraded() {
            for card in shivs {
                eng.upgrade_card(card);
            }
        }
        Ok(())
    }
}
