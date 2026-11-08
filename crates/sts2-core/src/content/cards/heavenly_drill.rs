//! `HeavenlyDrill` (`Models.Cards/HeavenlyDrill.cs`): Regent rare attack, cost X; deal 8 damage (10 upgraded) X times, X doubled when X is at least its Energy var (4).

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, CombatState, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct HeavenlyDrill;

#[sts2_macros::hooks]
impl Hooks for HeavenlyDrill {
    type Me = CardId;
}

impl CardModel for HeavenlyDrill {
    fn name(&self) -> &'static str {
        "Heavenly Drill"
    }

    fn cost(&self) -> i32 {
        0
    }

    fn has_energy_cost_x(&self) -> bool {
        true
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
            CardVar::new(Var::Damage, 8).upgrade(2),
            CardVar::new(Var::Energy, 4),
        ];
        VARS
    }

    /// `ShouldGlowGoldInternal`: the Energy is at least the Energy var.
    fn glows_gold(&self, s: &CombatState, me: CardId) -> bool {
        s.player.energy >= s.card(me).int_var(Var::Energy)
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("HeavenlyDrill needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let mut hits = eng.hooked().resolve_energy_x_value(play.card);
        if hits >= eng.s.card(play.card).int_var(Var::Energy) {
            hits *= 2;
        }
        // A negative X (only from a ModifyXValue listener) means no hits, like the C# loop.
        let hits = u32::try_from(hits).unwrap_or(0);
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
        Ok(())
    }
}
