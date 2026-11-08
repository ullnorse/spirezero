//! `PhotonCut` (`Models.Cards/PhotonCut.cs`): Regent common attack, cost 1; deal 10 damage (13
//! upgraded), draw 1 card (2 upgraded), then put 1 card from the hand on top of the draw pile.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets, Position};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, PileType, Prompt, PromptText};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct PhotonCut;

#[sts2_macros::hooks]
impl Hooks for PhotonCut {
    type Me = CardId;
}

impl CardModel for PhotonCut {
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Photon Cut"
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
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Damage, 10).upgrade(3),
            CardVar::new(Var::Cards, 1).upgrade(1),
            CardVar::new(Var::Named("PutBack"), 1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("PhotonCut needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        let cards = eng.s.card(play.card).var(Var::Cards);
        eng.draw(cards, false)?;
        // CardSelectCmd.FromHand(CardSelectorPrefs(prompt, PutBack)).
        let put_back = eng.s.card(play.card).count_var_len(Var::Named("PutBack"));
        let options = eng.s.player.hand.to_vec();
        for card in eng.choose_cards(
            Prompt::card(play.card, PromptText::Own),
            options,
            put_back,
            put_back,
        )? {
            eng.add_to_pile(card, PileType::Draw, Position::Top)?;
        }
        Ok(())
    }
}
