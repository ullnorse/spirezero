//! `Metamorphosis` (`Models.Cards/Metamorphosis.cs`): cost 2, Exhaust; 3 random Attacks (5
//! upgraded) of the character's pool, free this combat, go to random places in the draw pile.
//! `CardFactory.GetForCombat` picks them all (on `CombatCardGeneration`) before any is added.

use crate::combat::commands::Position;
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, PileType};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var, pools,
};

pub struct Metamorphosis;

#[sts2_macros::hooks]
impl Hooks for Metamorphosis {
    type Me = CardId;
}

impl CardModel for Metamorphosis {
    fn name(&self) -> &'static str {
        "Metamorphosis"
    }

    fn cost(&self) -> i32 {
        2
    }

    fn card_type(&self) -> CardType {
        CardType::Skill
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Event
    }

    fn target(&self) -> TargetType {
        TargetType::Myself
    }

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Exhaust)
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Cards, 3).upgrade(2)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let attacks: Vec<_> = pools::character(eng.s.player.character)
            .into_iter()
            .filter(|c| c.card_type == CardType::Attack)
            .collect();
        let count = eng.s.card(play.card).count_var_len(Var::Cards);
        let cards = pools::get_for_combat(eng, &attacks, count)?;
        let player = eng.s.player.creature;
        for card in cards {
            eng.s.set_to_free_this_combat(card);
            eng.add_generated_card_to_combat_at(
                card,
                PileType::Draw,
                Some(player),
                Position::Random,
            )?;
        }
        Ok(())
    }
}
