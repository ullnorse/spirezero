//! `ManifestAuthority` (`Models.Cards/ManifestAuthority.cs`): Regent uncommon skill, cost 1; gain 7
//! block (8 upgraded), then add a random colorless card to the hand, upgraded if this card is.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, PileType, ValueProps};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var, pools};

pub struct ManifestAuthority;

#[sts2_macros::hooks]
impl Hooks for ManifestAuthority {
    type Me = CardId;
}

impl CardModel for ManifestAuthority {
    fn name(&self) -> &'static str {
        "Manifest Authority"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn gains_block(&self) -> bool {
        true
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Uncommon
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Block, 7).upgrade(1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let block = eng.s.card(play.card).var(Var::Block);
        let player = eng.s.player.creature;
        eng.gain_block(player, block, ValueProps::MOVE, Some(play))?;
        if let Some(&card) = eng.get_distinct_for_combat(&pools::colorless(), 1)?.first() {
            if eng.s.card(play.card).is_upgraded() {
                eng.upgrade_card(card);
            }
            eng.add_generated_card_to_combat(card, PileType::Hand, Some(player))?;
        }
        Ok(())
    }
}
