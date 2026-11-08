//! `CaptureSpirit` (`Models.Cards/CaptureSpirit.cs`): cost 1, deals 3 (4 upgraded) unblockable
//! unpowered damage, then shuffles 3 (4) new Souls into the draw pile at random positions.

use crate::combat::commands::Position;
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, PileType, ValueProps};
use crate::content::cards::{CardKind, CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct CaptureSpirit;

#[sts2_macros::hooks]
impl Hooks for CaptureSpirit {
    type Me = CardId;
}

impl CardModel for CaptureSpirit {
    fn name(&self) -> &'static str {
        "Capture Spirit"
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
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Damage, 3)
                .props(
                    ValueProps::UNBLOCKABLE
                        .with(ValueProps::UNPOWERED)
                        .with(ValueProps::MOVE),
                )
                .upgrade(1),
            CardVar::new(Var::Cards, 3).upgrade(1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("CaptureSpirit needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let count = eng.s.card(play.card).int_var(Var::Cards);
        let player = eng.s.player.creature;
        eng.damage(
            &[target],
            damage,
            ValueProps::UNBLOCKABLE
                .with(ValueProps::UNPOWERED)
                .with(ValueProps::MOVE),
            Some(player),
            Some(play.card),
        )?;
        // Soul.Create makes every Soul first; `AddGeneratedCardsToCombat` then adds each.
        let souls: Vec<CardId> = (0..count)
            .map(|_| eng.create_card(CardKind::Soul))
            .collect();
        eng.add_generated_cards_to_combat(&souls, PileType::Draw, Some(player), Position::Random)
    }
}
