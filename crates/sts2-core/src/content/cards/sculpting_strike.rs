//! `SculptingStrike` (`Models.Cards/SculptingStrike.cs`): cost 1, 9 damage (12 upgraded), then a
//! hand card chosen among those without a local Ethereal gains Ethereal. (Its `CanonicalKeywords`
//! is empty; the Ethereal in the inventory is a hover tip.)

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, Prompt, PromptText};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, TargetType, Var,
};

pub struct SculptingStrike;

#[sts2_macros::hooks]
impl Hooks for SculptingStrike {
    type Me = CardId;
}

impl CardModel for SculptingStrike {
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Sculpting Strike"
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

    fn is_strike(&self) -> bool {
        true
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Damage, 9).upgrade(3)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("SculptingStrike needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        // CardSelectCmd.FromHand(prefs 1, filter: no local Ethereal keyword; a global one, such
        // as `HexPower`'s, still allows the card).
        let options: Vec<CardId> = eng
            .s
            .player
            .hand
            .iter()
            .copied()
            .filter(|&c| !eng.s.card(c).has_local(CardKeyword::Ethereal))
            .collect();
        if let Some(&card) = eng
            .choose_cards(Prompt::card(play.card, PromptText::Own), options, 1, 1)?
            .first()
        {
            eng.apply_keyword(card, CardKeyword::Ethereal);
        }
        Ok(())
    }
}
