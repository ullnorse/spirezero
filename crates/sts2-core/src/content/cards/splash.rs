//! `Splash` (`Models.Cards/Splash.cs`): cost 1; choose 1 of 3 random Attack cards of the other
//! characters' pools (upgraded if Splash is) to add to the hand, free this turn.
//!
//! Generation stops the combat (`Unsupported`) if it picks a card that is not ported.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, PileType, PromptSource};
use crate::content::cards::{CardModel, CardRarity, CardType, TargetType, pools};
use crate::content::characters::Character;

pub struct Splash;

#[sts2_macros::hooks]
impl Hooks for Splash {
    type Me = CardId;
}

impl CardModel for Splash {
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Splash"
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
        // `CharacterCardPools` minus the owner's, then only the Attack cards.
        let own = eng.s.player.character;
        let attacks: Vec<_> = Character::MODEL_DB_ORDER
            .into_iter()
            .filter(|&c| c != own)
            .flat_map(pools::character)
            .filter(|c| c.card_type == CardType::Attack)
            .collect();
        let options = eng.get_distinct_for_combat(&attacks, 3)?;
        if eng.s.card(play.card).upgrade_level > 0 {
            for &card in &options {
                eng.upgrade_card(card);
            }
        }
        if let Some(card) = eng.choose_a_card(PromptSource::Card(play.card), options)? {
            eng.s.set_to_free_this_turn(card);
            let player = eng.s.player.creature;
            eng.add_generated_card_to_combat(card, PileType::Hand, Some(player))?;
        }
        Ok(())
    }
}
