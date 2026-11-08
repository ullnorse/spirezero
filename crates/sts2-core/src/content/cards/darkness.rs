//! `Darkness` (`Models.Cards/Darkness.cs`): cost 1; channel 1 Dark, then trigger the passive of
//! every Dark orb once (twice upgraded), bypassing the trigger-count hooks.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, Orb, OrbKind};
use crate::content::cards::{CardModel, CardRarity, CardType, TargetType};

pub struct Darkness;

#[sts2_macros::hooks]
impl Hooks for Darkness {
    type Me = CardId;
}

impl CardModel for Darkness {
    fn name(&self) -> &'static str {
        "Darkness"
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
        TargetType::Myself
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let trigger_count = if eng.s.card(play.card).is_upgraded() {
            2
        } else {
            1
        };
        eng.channel(OrbKind::Dark)?;
        // The `Where` is lazy, so the queue is read as the loop runs; no passive changes it.
        let dark: Vec<Orb> = eng
            .s
            .player
            .orbs
            .iter()
            .copied()
            .filter(|o| o.kind == OrbKind::Dark)
            .collect();
        for orb in dark {
            for _ in 0..trigger_count {
                eng.orb_passive(orb, None, false)?;
            }
        }
        Ok(())
    }
}
