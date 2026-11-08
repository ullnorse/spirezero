//! `Cascade` (`Models.Cards/Cascade.cs`): X cost; auto-play the top X cards of the draw pile
//! (X + 1 upgraded).

use crate::combat::commands::Position;
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, TargetType};

pub struct Cascade;

#[sts2_macros::hooks]
impl Hooks for Cascade {
    type Me = CardId;
}

impl CardModel for Cascade {
    /// It auto-plays draw-pile cards, any of which can prompt (Headbutt, Burning Pact), so the
    /// combat must snapshot its steps while Cascade is in play.
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Cascade"
    }

    fn cost(&self) -> i32 {
        -1
    }

    fn has_energy_cost_x(&self) -> bool {
        true
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
        let mut count = eng.hooked().resolve_energy_x_value(play.card);
        if eng.s.card(play.card).is_upgraded() {
            count += 1;
        }
        // A negative X (only from a ModifyXValue listener) plays nothing, like the C# loop.
        eng.auto_play_from_draw_pile(u32::try_from(count).unwrap_or(0), Position::Top, false)
    }
}

#[cfg(test)]
mod tests {
    use crate::content::cards::CardKind;

    /// Regression: a `PlayCard` of Cascade whose auto-played card prompted panicked with "no
    /// model in combat reports `may_prompt`" (seed 1095028, Ironclad A0 run-policy rollout).
    #[test]
    fn cascade_may_prompt() {
        assert!(CardKind::Cascade.may_prompt());
    }
}
