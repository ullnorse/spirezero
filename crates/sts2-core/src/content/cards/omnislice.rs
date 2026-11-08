//! `Omnislice` (`Models.Cards/Omnislice.cs`): cost 0, 8 damage (11 upgraded). Every other
//! hittable enemy then takes the damage dealt to the target (blocked, lost and overkill). The
//! two damage calls are grouped as one attack with an `AttackContext`.

use crate::combat::commands::{Attack, AttackDamage, AttackId, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, ValueProps};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::decimal::Decimal;

pub struct Omnislice;

#[sts2_macros::hooks]
impl Hooks for Omnislice {
    type Me = CardId;
}

impl CardModel for Omnislice {
    fn name(&self) -> &'static str {
        "Omnislice"
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
        const VARS: &[CardVar] = &[CardVar::new(Var::Damage, 8).upgrade(3)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Omnislice needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;

        // AttackCommand.CreateContextAsync: `new AttackCommand(0m).FromCard(..)
        // .TargetingAllOpponents(..)`, with `BeforeAttack` right away and `AfterAttack` on
        // disposal. `AddHit` raises the command's hit count (it starts at 1) and adds results.
        let mut context = Attack::new(
            player,
            AttackTargets::AllOpponents,
            AttackDamage::Fixed(Decimal::ZERO),
            Some(play.card),
        );
        context.id = Some(AttackId(eng.s.next_attack_id));
        eng.s.next_attack_id += 1;
        eng.before_attack(&context)?;

        let mut results = eng.damage(
            &[target],
            damage,
            ValueProps::MOVE,
            Some(player),
            Some(play.card),
        )?;
        context.hits += 1;
        if let Some(first) = results.first().copied() {
            // GetTeammatesOf(receiver).Except(target).Where(IsHittable).
            let others: Vec<_> = eng
                .s
                .hittable_enemies()
                .into_iter()
                .filter(|&e| {
                    e != target && eng.s.creature(e).side == eng.s.creature(first.receiver).side
                })
                .collect();
            if !others.is_empty() {
                let total = first.blocked + first.unblocked + first.overkill;
                let splash = eng.damage(
                    &others,
                    Decimal::from(total),
                    ValueProps::UNPOWERED.with(ValueProps::MOVE),
                    Some(player),
                    Some(play.card),
                )?;
                context.hits += 1;
                results.extend(splash);
            }
        }
        eng.after_attack(&context, &results)
    }
}
