//! `EchoingSlash` (`Models.Cards/EchoingSlash.cs`): cost 1, 10 damage (13 upgraded) to all enemies, repeated once per kill.

use crate::combat::commands::{Attack, AttackDamage, AttackId, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, ValueProps};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::decimal::Decimal;

pub struct EchoingSlash;

#[sts2_macros::hooks]
impl Hooks for EchoingSlash {
    type Me = CardId;
}

impl CardModel for EchoingSlash {
    fn name(&self) -> &'static str {
        "Echoing Slash"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Attack
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Uncommon
    }

    fn target(&self) -> TargetType {
        TargetType::AllEnemies
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Damage, 10).upgrade(3)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;

        // AttackCommand.CreateContextAsync (see `omnislice.rs`): `BeforeAttack` right away,
        // `AfterAttack` on disposal, each `CreatureCmd.Damage` added as a hit.
        let mut context = Attack::new(
            player,
            AttackTargets::AllOpponents,
            AttackDamage::Fixed(Decimal::ZERO),
            Some(play.card),
        );
        context.id = Some(AttackId(eng.s.next_attack_id));
        eng.s.next_attack_id += 1;
        eng.before_attack(&context)?;

        let mut all_results = Vec::new();
        let mut attack_count = 1_u32;
        while attack_count > 0 {
            attack_count -= 1;
            let targets = eng.hooked().hittable_enemies();
            let results = eng.damage(
                &targets,
                damage,
                ValueProps::MOVE,
                Some(player),
                Some(play.card),
            )?;
            context.hits += 1;
            // Every kill earns another round.
            let kills = results.iter().filter(|r| r.was_target_killed).count();
            attack_count += u32::try_from(kills).expect("few enemies");
            all_results.extend(results);
        }
        eng.after_attack(&context, &all_results)
    }
}
