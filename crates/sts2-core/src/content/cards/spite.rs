//! `Spite` (`Models.Cards/Spite.cs`): cost 0, 5 damage (hits 2 times, 3 upgraded) if you lost HP this turn, else once.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, CombatState, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct Spite;

#[sts2_macros::hooks]
impl Hooks for Spite {
    type Me = CardId;
}

impl CardModel for Spite {
    fn name(&self) -> &'static str {
        "Spite"
    }

    fn cost(&self) -> i32 {
        0
    }

    fn card_type(&self) -> CardType {
        CardType::Attack
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Uncommon
    }

    fn target(&self) -> TargetType {
        TargetType::AnyEnemy
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Damage, 5),
            CardVar::new(Var::Repeat, 2).upgrade(1),
        ];
        VARS
    }

    /// `ShouldGlowGoldInternal`: the owner lost HP this turn.
    fn glows_gold(&self, s: &CombatState, _me: CardId) -> bool {
        s.lost_hp_this_turn(s.player.creature)
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Spite needs a target");
        let player = eng.s.player.creature;
        let hits = if eng.s.lost_hp_this_turn(player) {
            eng.s.card(play.card).count_var(Var::Repeat)
        } else {
            1
        };
        eng.attack(
            &Attack::new(
                player,
                AttackTargets::Single(target),
                AttackDamage::Fixed(eng.s.card(play.card).var(Var::Damage)),
                Some(play.card),
            )
            .with_hits(hits),
        )?;
        Ok(())
    }
}
