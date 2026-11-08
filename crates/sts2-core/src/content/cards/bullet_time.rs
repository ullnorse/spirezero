//! `BulletTime` (`Models.Cards/BulletTime.cs`): cost 3 (2 upgraded); every non-X card in hand is
//! free this turn, then `NoDrawPower`.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, TargetType};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

pub struct BulletTime;

#[sts2_macros::hooks]
impl Hooks for BulletTime {
    type Me = CardId;
}

impl CardModel for BulletTime {
    fn name(&self) -> &'static str {
        "Bullet Time"
    }

    fn cost(&self) -> i32 {
        3
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Rare
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn on_upgrade(&self, card: &mut crate::combat::state::Card) {
        card.energy_cost.upgrade_by(-1);
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let hand: Vec<CardId> = eng.s.player.hand.to_vec();
        for card in hand {
            if !eng.s.card(card).energy_cost.costs_x {
                eng.s.set_to_free_this_turn(card);
            }
        }
        let player = eng.s.player.creature;
        eng.apply_power_from_card(
            Some(play.card),
            player,
            PowerKind::NoDraw,
            Decimal::ONE,
            Some(player),
        )
    }
}
