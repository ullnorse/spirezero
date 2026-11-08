//! `UpMySleeve` (`Models.Cards/UpMySleeve.cs`): cost 2, Skill, Uncommon; creates 3 Shivs in the
//! hand (4 upgraded), then costs 1 less for the rest of the combat. `TimesPlayedThisCombat` is
//! only written, never read.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::shiv;
use crate::content::cards::{CardData, CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct UpMySleeve;

#[sts2_macros::hooks]
impl Hooks for UpMySleeve {
    type Me = CardId;
}

impl CardModel for UpMySleeve {
    fn name(&self) -> &'static str {
        "Up My Sleeve"
    }

    fn cost(&self) -> i32 {
        2
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Uncommon
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Cards, 3).upgrade(1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let cards = eng.s.card(play.card).int_var(Var::Cards);
        for _ in 0..cards {
            shiv::create_in_hand(eng, 1)?;
        }
        let card = eng.s.card_mut(play.card);
        let times_played = match card.data {
            CardData::UpMySleeve(n) => n,
            _ => 0,
        };
        card.data = CardData::UpMySleeve(times_played + 1);
        eng.s.energy_cost_mut(play.card).add_this_combat(-1);
        Ok(())
    }
}
