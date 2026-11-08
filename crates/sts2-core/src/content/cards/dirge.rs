//! `Dirge` (`Models.Cards/Dirge.cs`): cost X, Exhaust; summons Osty for 3 (4 upgraded) X times,
//! then shuffles X new Souls (upgraded if Dirge is) into the draw pile at random positions.

use crate::combat::commands::Position;
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, PileType};
use crate::content::cards::{
    CardKeyword, CardKind, CardModel, CardRarity, CardType, CardVar, Keywords, TargetType, Var,
};

pub struct Dirge;

#[sts2_macros::hooks]
impl Hooks for Dirge {
    type Me = CardId;
}

impl CardModel for Dirge {
    fn name(&self) -> &'static str {
        "Dirge"
    }

    fn cost(&self) -> i32 {
        0
    }

    fn has_energy_cost_x(&self) -> bool {
        true
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

    fn keywords(&self) -> Keywords {
        Keywords::NONE.with(CardKeyword::Exhaust)
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Summon, 3).upgrade(1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let upgraded = eng.s.card(play.card).is_upgraded();
        let summon = eng.s.card(play.card).var(Var::Summon);
        // A negative X (only from a ModifyXValue listener) loops zero times.
        let x = usize::try_from(eng.hooked().resolve_energy_x_value(play.card)).unwrap_or(0);
        for _ in 0..x {
            eng.summon_osty(summon)?;
        }
        let souls: Vec<CardId> = (0..x).map(|_| eng.create_card(CardKind::Soul)).collect();
        if upgraded {
            for &soul in &souls {
                eng.upgrade_card(soul);
            }
        }
        let player = eng.s.player.creature;
        eng.add_generated_cards_to_combat(&souls, PileType::Draw, Some(player), Position::Random)
    }
}
