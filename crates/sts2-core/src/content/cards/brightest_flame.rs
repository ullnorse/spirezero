//! `BrightestFlame` (`Models.Cards/BrightestFlame.cs`): Ancient skill (`Storybook`), cost 0; gain
//! 2 energy (3 upgraded), draw 2 cards (3), then lose 2 max HP (`CreatureCmd.LoseMaxHp`,
//! `isFromCard: true`).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, CreatureId, Engine, Flow, ValueProps};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::decimal::Decimal;

pub struct BrightestFlame;

#[sts2_macros::hooks]
impl Hooks for BrightestFlame {
    type Me = CardId;
}

/// `CreatureCmd.LoseMaxHp(creature, amount, isFromCard: true)` (`CreatureCmd.cs:863-880`): HP
/// above the new max is lost as unblockable, unpowered `Move` damage (no dealer, no card
/// source), then max HP drops to at least 1. The map point's `MaxHpLost` is run history only.
fn lose_max_hp_from_card(eng: &mut Engine<'_>, creature: CreatureId, amount: Decimal) -> Flow {
    let c = eng.s.creature(creature);
    let new_max = Decimal::from(c.max_hp) - amount;
    let hp = Decimal::from(c.hp);
    if new_max < hp {
        eng.damage(
            &[creature],
            hp - new_max,
            ValueProps::UNBLOCKABLE
                .with(ValueProps::UNPOWERED)
                .with(ValueProps::MOVE),
            None,
            None,
        )?;
    }
    eng.set_max_hp(creature, new_max.max(Decimal::ONE))?;
    Ok(())
}

impl CardModel for BrightestFlame {
    fn name(&self) -> &'static str {
        "Brightest Flame"
    }

    fn cost(&self) -> i32 {
        0
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Ancient
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::MaxHp, 2),
            CardVar::new(Var::Energy, 2).upgrade(1),
            CardVar::new(Var::Cards, 2).upgrade(1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let card = eng.s.card(play.card);
        let (energy, cards, max_hp) = (
            card.var(Var::Energy),
            card.var(Var::Cards),
            card.var(Var::MaxHp),
        );
        eng.gain_energy(energy)?;
        eng.draw(cards, false)?;
        let player = eng.s.player.creature;
        lose_max_hp_from_card(eng, player, max_hp)
    }
}
