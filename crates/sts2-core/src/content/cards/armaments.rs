//! `Armaments` (`Models.Cards/Armaments.cs`): 5 block, then upgrade one upgradable card of the hand
//! (the player's pick); upgraded, it upgrades every upgradable card of the hand instead.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, Prompt, PromptText, ValueProps};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct Armaments;

#[sts2_macros::hooks]
impl Hooks for Armaments {
    type Me = CardId;
}

impl CardModel for Armaments {
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Armaments"
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
        CardRarity::Common
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Block, 5)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let player = eng.s.player.creature;
        let block = eng.s.card(play.card).var(Var::Block);
        eng.gain_block(player, block, ValueProps::MOVE, Some(play))?;
        let upgradable: Vec<CardId> = eng
            .s
            .player
            .hand
            .iter()
            .copied()
            .filter(|&c| eng.s.card(c).is_upgradable())
            .collect();
        if eng.s.card(play.card).is_upgraded() {
            for card in upgradable {
                eng.upgrade_card(card);
            }
            return Ok(());
        }
        // CardSelectCmd.FromHandForUpgrade: the only candidate is taken without a prompt.
        if let Some(&card) = eng
            .choose_cards(
                Prompt::card(play.card, PromptText::UpgradeFromHand),
                upgradable,
                1,
                1,
            )?
            .first()
        {
            eng.upgrade_card(card);
        }
        Ok(())
    }
}
