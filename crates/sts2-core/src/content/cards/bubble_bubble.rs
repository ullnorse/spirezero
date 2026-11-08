//! `BubbleBubble` (`Models.Cards/BubbleBubble.cs`): cost 1, applies 9 Poison (12 upgraded) to one enemy. Only if the target already has Poison.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, CombatState, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;

pub struct BubbleBubble;

#[sts2_macros::hooks]
impl Hooks for BubbleBubble {
    type Me = CardId;
}

impl CardModel for BubbleBubble {
    fn name(&self) -> &'static str {
        "Bubble Bubble"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Uncommon
    }

    fn target(&self) -> TargetType {
        TargetType::AnyEnemy
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Power(PowerKind::Poison), 9).upgrade(3)];
        VARS
    }

    /// `ShouldGlowGoldInternal`: a hittable enemy has Poison.
    fn glows_gold(&self, s: &CombatState, _me: CardId) -> bool {
        s.hittable_enemies()
            .iter()
            .any(|&e| s.creature(e).has_power(PowerKind::Poison))
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("BubbleBubble needs a target");
        let amount = eng.s.card(play.card).var(Var::Power(PowerKind::Poison));
        let player = eng.s.player.creature;
        // `cardPlay.Target.HasPower<PoisonPower>()`.
        if !eng.s.creature(target).has_power(PowerKind::Poison) {
            return Ok(());
        }
        eng.apply_power_from_card(
            Some(play.card),
            target,
            PowerKind::Poison,
            amount,
            Some(player),
        )
    }
}
