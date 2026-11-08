//! `EndOfDays` (`Models.Cards/EndOfDays.cs`): cost 3; 29 Doom (37 upgraded) to all enemies, then
//! kills every enemy whose Doom now covers its HP.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::content::powers::PowerKind;
use crate::content::powers::doom::{doom_kill, doomed_creatures};

pub struct EndOfDays;

#[sts2_macros::hooks]
impl Hooks for EndOfDays {
    type Me = CardId;
}

impl CardModel for EndOfDays {
    fn name(&self) -> &'static str {
        "End of Days"
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
        TargetType::AllEnemies
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Power(PowerKind::Doom), 29).upgrade(8)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let doom = eng.s.card(play.card).var(Var::Power(PowerKind::Doom));
        let player = eng.s.player.creature;
        for enemy in eng.hooked().hittable_enemies() {
            eng.apply_power_from_card(Some(play.card), enemy, PowerKind::Doom, doom, Some(player))?;
        }
        let enemies = eng.hooked().hittable_enemies();
        let doomed = doomed_creatures(eng.s, &enemies);
        doom_kill(eng, &doomed)
    }
}
