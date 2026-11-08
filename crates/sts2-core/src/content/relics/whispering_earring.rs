//! `WhisperingEarring` (`Models.Relics/WhisperingEarring.cs`): Ancient (Vakuu); +1 max energy, and
//! on turn 1 the hand is auto-played (up to 13 cards) when the auto-pre-play phase is entered,
//! before the player can act. `VakuuCardSelector` (`Models.Relics/VakuuCardSelector.cs`), the
//! `ICardSelector` it pushes, is `Engine::vakuu_selector`: card choices pick the first options.

use crate::combat::AutoPlayType;
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CombatState, CreatureId, Engine, Flow, RelicId, can_play};
use crate::content::cards::TargetType;
use crate::content::relics::RelicModel;
use crate::decimal::Decimal;

/// `EnergyVar(1)`.
const ENERGY: Decimal = Decimal::ONE;
/// `maxCardsToPlay`.
const MAX_CARDS_TO_PLAY: usize = 13;

pub struct WhisperingEarring;

impl RelicModel for WhisperingEarring {
    /// Auto-plays the hand on turn 1; an auto-played card can open its own choice (Headbutt, Burning Pact),
    /// so the combat must snapshot its steps while this is in play.
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Whispering Earring"
    }
}

/// `GetTarget` (`WhisperingEarring.cs:124-134`): the leftmost hittable enemy for `AnyEnemy`, the
/// owner for `AnyPlayer`; `AnyAlly` (a multiplayer target) and the rest have none.
fn target(eng: &Engine<'_>, card: CardId) -> Option<CreatureId> {
    match eng.s.target_type(card) {
        TargetType::AnyEnemy => eng.hooked().hittable_enemies().first().copied(),
        TargetType::AnyPlayer => Some(eng.s.player.creature),
        _ => None,
    }
}

#[sts2_macros::hooks]
impl Hooks for WhisperingEarring {
    type Me = RelicId;

    fn modify_max_energy(&self, _s: &CombatState, _me: RelicId, amount: Decimal) -> Decimal {
        // `player != Owner` is never true: the only player is the owner.
        amount + ENERGY
    }

    /// Late, to run after every other listener (`Imbued`).
    fn after_auto_pre_play_phase_entered_late(&self, eng: &mut Engine<'_>, _me: RelicId) -> Flow {
        if eng.s.player.turn_number > 1 {
            return Ok(());
        }
        let start_turn = eng.s.player.turn_number;
        eng.vakuu_selector = true;
        let result = (|| {
            for _ in 0..MAX_CARDS_TO_PLAY {
                if eng.hooked().is_over_or_ending()
                    || eng.s.player.ready_to_end_turn
                    || eng.s.player.turn_number != start_turn
                {
                    break;
                }
                let Some(card) = eng
                    .s
                    .player
                    .hand
                    .iter()
                    .copied()
                    .find(|&c| can_play(eng.s, c).is_ok())
                else {
                    break;
                };
                let target = target(eng, card);
                eng.spend_resources(card)?;
                eng.auto_play_with(card, target, AutoPlayType::Default, true)?;
            }
            Ok(())
        })();
        eng.vakuu_selector = false;
        result
    }
}
