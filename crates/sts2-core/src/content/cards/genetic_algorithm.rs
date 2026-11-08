//! `GeneticAlgorithm` (`Models.Cards/GeneticAlgorithm.cs`): Defect rare skill, cost 1, Exhaust;
//! gain `CurrentBlock` block (1 + `IncreasedBlock`), then raise `IncreasedBlock` by 3 (4
//! upgraded). The play also buffs the card's deck version (`DeckVersion.BuffFromPlay`): the deck
//! card's counter is its `IncreasedBlock`, so the next fight's copy starts with the new block.
//! `AfterDowngraded` only recomputes the block from it, which the card derives anyway.

use crate::combat::hooks::Hooks;
use crate::combat::state::Card;
use crate::combat::{CardId, CardPlay, Engine, Flow, ValueProps};
use crate::content::cards::{
    CardData, CardKeyword, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};
use crate::decimal::Decimal;

pub struct GeneticAlgorithm;

/// `GeneticAlgorithm`'s saved field.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `IncreasedBlock`.
    pub increased_block: i32,
}

fn increased_block(card: &Card) -> i32 {
    match card.data {
        CardData::GeneticAlgorithm(data) => data.increased_block,
        CardData::None => 0,
        _ => unreachable!("a GeneticAlgorithm holds GeneticAlgorithm data"),
    }
}

/// The saved fields of a deck card whose `IncreasedBlock` is `counter`, which the combat clone
/// copies.
pub fn data_from_deck(counter: u32) -> CardData {
    CardData::GeneticAlgorithm(Data {
        increased_block: i32::try_from(counter).expect("IncreasedBlock fits i32"),
    })
}

#[sts2_macros::hooks]
impl Hooks for GeneticAlgorithm {
    type Me = CardId;
}

impl CardModel for GeneticAlgorithm {
    fn name(&self) -> &'static str {
        "Genetic Algorithm"
    }

    fn cost(&self) -> i32 {
        1
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

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Exhaust)
    }

    fn gains_block(&self) -> bool {
        true
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Block, 1),
            CardVar::new(Var::Named("Increase"), 3).upgrade(1),
        ];
        VARS
    }

    /// `DynamicVars.Block.BaseValue = CurrentBlock = 1 + IncreasedBlock`.
    fn instance_var(&self, card: &Card, name: Var, value: Decimal) -> Decimal {
        match name {
            Var::Block => value + Decimal::from(increased_block(card)),
            _ => value,
        }
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let block = eng.s.card(play.card).var(Var::Block);
        let player = eng.s.player.creature;
        eng.gain_block(player, block, ValueProps::MOVE, Some(play))?;
        // BuffFromPlay.
        let increase = eng.s.card(play.card).count_var(Var::Named("Increase"));
        let increased = increased_block(eng.s.card(play.card)) + increase.cast_signed();
        eng.s.card_mut(play.card).data = CardData::GeneticAlgorithm(Data {
            increased_block: increased,
        });
        // `(DeckVersion as GeneticAlgorithm)?.BuffFromPlay(increase)`.
        if let Some(i) = eng.s.card(play.card).deck_version() {
            eng.s.deck[i].counter += increase;
        }
        Ok(())
    }
}
