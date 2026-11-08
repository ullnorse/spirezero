//! `HeirloomHammer` (`Models.Cards/HeirloomHammer.cs`): Regent rare attack, cost 2; deal 20
//! damage (25 upgraded), then choose a card in the hand whose visual pool is colorless and add
//! `Repeat` (1) clones of it to the hand.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, PileType, Prompt, PromptText};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct HeirloomHammer;

#[sts2_macros::hooks]
impl Hooks for HeirloomHammer {
    type Me = CardId;
}

impl CardModel for HeirloomHammer {
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Heirloom Hammer"
    }

    fn cost(&self) -> i32 {
        2
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
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Damage, 20).upgrade(5),
            CardVar::new(Var::Repeat, 1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("HeirloomHammer needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        // CardSelectCmd.FromHand(CardSelectorPrefs(prompt, 1), VisualCardPool.IsColorless).
        let options: Vec<_> = eng
            .s
            .player
            .hand
            .iter()
            .copied()
            .filter(|&c| eng.s.card(c).kind.is_colorless())
            .collect();
        if let Some(&selection) = eng
            .choose_cards(Prompt::card(play.card, PromptText::Own), options, 1, 1)?
            .first()
        {
            for _ in 0..eng.s.card(play.card).count_var(Var::Repeat) {
                let card = eng.create_clone(selection);
                eng.add_generated_card_to_combat(card, PileType::Hand, Some(player))?;
            }
        }
        Ok(())
    }
}
