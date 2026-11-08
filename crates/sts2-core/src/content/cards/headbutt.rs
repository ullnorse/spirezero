//! `Headbutt` (`Models.Cards/Headbutt.cs`): cost 1; 9 damage (12 upgraded), then move a card of
//! the player's choice from the discard pile to the top of the draw pile.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets, Position};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, PileType, Prompt, PromptText};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct Headbutt;

#[sts2_macros::hooks]
impl Hooks for Headbutt {
    type Me = CardId;
}

impl CardModel for Headbutt {
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Headbutt"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Attack
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Common
    }

    fn target(&self) -> TargetType {
        TargetType::AnyEnemy
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Damage, 9).upgrade(3)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Headbutt needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        // CardSelectCmd.FromCombatPile(Discard, CardSelectorPrefs(prompt, 1)).FirstOrDefault().
        let discard = eng.s.player.discard.to_vec();
        if let Some(&card) = eng
            .choose_cards(Prompt::card(play.card, PromptText::Own), discard, 1, 1)?
            .first()
        {
            eng.add_to_pile(card, PileType::Draw, Position::Top)?;
        }
        Ok(())
    }
}
