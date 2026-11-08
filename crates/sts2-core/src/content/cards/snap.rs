//! `Snap` (`Models.Cards/Snap.cs`): cost 1, Osty hits for 7 (10 upgraded), then a hand card
//! chosen among those without Retain gains Retain. Does nothing for the hit without a living
//! Osty (the choice still happens).

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, Prompt, PromptText};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, TargetType, Var,
};

pub struct Snap;

#[sts2_macros::hooks]
impl Hooks for Snap {
    type Me = CardId;
}

impl CardModel for Snap {
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Snap"
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

    fn is_osty_attack(&self) -> bool {
        true
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::OstyDamage, 7).upgrade(3)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Snap needs a target");
        // Osty.CheckMissingWithAnim.
        if eng.s.is_osty_alive() {
            let damage = eng.s.card(play.card).var(Var::OstyDamage);
            let osty = eng.s.osty().expect("Osty is alive");
            eng.attack(&Attack::new(
                osty,
                AttackTargets::Single(target),
                AttackDamage::Fixed(damage),
                Some(play.card),
            ))?;
        }
        // CardSelectCmd.FromHand(prefs 1, filter: !c.Keywords.Contains(Retain)).
        let hooked = eng.hooked();
        let options: Vec<CardId> = hooked
            .s
            .player
            .hand
            .iter()
            .copied()
            .filter(|&c| !hooked.has_keyword(c, CardKeyword::Retain))
            .collect();
        if let Some(&card) = eng
            .choose_cards(Prompt::card(play.card, PromptText::Own), options, 1, 1)?
            .first()
        {
            eng.apply_keyword(card, CardKeyword::Retain);
        }
        Ok(())
    }
}
