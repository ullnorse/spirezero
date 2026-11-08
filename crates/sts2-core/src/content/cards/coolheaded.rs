//! `Coolheaded` (`Models.Cards/Coolheaded.cs`): cost 1; channel 1 Frost, draw 1 (2 upgraded).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, OrbKind};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};
use crate::decimal::Decimal;

pub struct Coolheaded;

#[sts2_macros::hooks]
impl Hooks for Coolheaded {
    type Me = CardId;
}

impl CardModel for Coolheaded {
    fn name(&self) -> &'static str {
        "Coolheaded"
    }

    fn cost(&self) -> i32 {
        1
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Common
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Cards, 1).upgrade(1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        eng.channel(OrbKind::Frost)?;
        // Read after the channel: an evoke that kills Dampen's last caster re-upgrades this card.
        let cards = eng.s.card(play.card).int_var(Var::Cards);
        eng.draw(Decimal::from(cards), false)?;
        Ok(())
    }
}
