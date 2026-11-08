//! `Reave` (`Models.Cards/Reave.cs`): cost 1, 10 damage (13 upgraded), then shuffles 1 new Soul
//! into the draw pile at a random position; an upgraded Reave upgrades that Soul first.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets, Position};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, PileType};
use crate::content::cards::{CardKind, CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct Reave;

#[sts2_macros::hooks]
impl Hooks for Reave {
    type Me = CardId;
}

impl CardModel for Reave {
    fn name(&self) -> &'static str {
        "Reave"
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
            CardVar::new(Var::Cards, 1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Reave needs a target");
        let upgraded = eng.s.card(play.card).is_upgraded();
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        let souls: Vec<CardId> = (0..eng.s.card(play.card).int_var(Var::Cards))
            .map(|_| eng.create_card(CardKind::Soul))
            .collect();
        if upgraded {
            for &soul in &souls {
                eng.upgrade_card(soul);
            }
        }
        eng.add_generated_cards_to_combat(&souls, PileType::Draw, Some(player), Position::Random)
    }
}
