//! `GunkUp` (`Models.Cards/GunkUp.cs`): cost 1, 4 damage (5 upgraded) three times, then a `Slimed` into the discard pile.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, PileType};
use crate::content::cards::{CardKind, CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct GunkUp;

#[sts2_macros::hooks]
impl Hooks for GunkUp {
    type Me = CardId;
}

impl CardModel for GunkUp {
    fn name(&self) -> &'static str {
        "Gunk Up"
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
            CardVar::new(Var::Damage, 4).upgrade(1),
            CardVar::new(Var::Repeat, 3),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("GunkUp needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let hits = eng.s.card(play.card).count_var(Var::Repeat);
        let player = eng.s.player.creature;
        eng.attack(
            &Attack::new(
                player,
                AttackTargets::Single(target),
                AttackDamage::Fixed(damage),
                Some(play.card),
            )
            .with_hits(hits),
        )?;
        let card = eng.create_card(CardKind::Slimed);
        eng.add_generated_card_to_combat(card, PileType::Discard, Some(player))?;
        Ok(())
    }
}
