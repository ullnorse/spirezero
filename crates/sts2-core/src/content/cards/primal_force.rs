//! `PrimalForce` (`Models.Cards/PrimalForce.cs`): transform every Attack of the hand into a
//! `GiantRock` (upgraded, an upgraded one).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardKind, CardModel, CardRarity, CardType, TargetType};

pub struct PrimalForce;

#[sts2_macros::hooks]
impl Hooks for PrimalForce {
    type Me = CardId;
}

impl CardModel for PrimalForce {
    fn name(&self) -> &'static str {
        "Primal Force"
    }

    fn cost(&self) -> i32 {
        0
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

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let upgraded = eng.s.card(play.card).is_upgraded();
        // `c.IsTransformable` only fails for an Eternal card in the deck.
        let attacks: Vec<CardId> = eng
            .s
            .player
            .hand
            .iter()
            .copied()
            .filter(|&c| eng.s.card(c).card_type() == CardType::Attack)
            .collect();
        for attack in attacks {
            // CreateCard<GiantRock>, `CardCmd.Upgrade`, `CardCmd.Transform`: the transform creates
            // the replacement itself (the last card made), which nothing observes before the upgrade.
            let created = eng.s.cards.len();
            eng.transform_to(attack, CardKind::GiantRock)?;
            if upgraded && eng.s.cards.len() > created {
                eng.upgrade_card(CardId(u32::try_from(created).expect("card count fits u32")));
            }
        }
        Ok(())
    }
}
