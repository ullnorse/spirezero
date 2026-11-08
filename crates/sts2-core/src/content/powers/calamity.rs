//! `CalamityPower` (`Models.Powers/CalamityPower.cs`): after the owner plays an Attack, `Amount`
//! random Attack cards of the character's pool (`CardFactory.GetForCombat`, repeats allowed) are
//! added to the hand.
//!
//! The C# keeps a dictionary from the card to the amount at `BeforeCardPlayed` and only uses it as
//! a "this play started here" marker: `AfterCardPlayed` generates `Amount` (the current amount)
//! cards. It is a fixed array of slots here, like `OblivionPower`'s.

use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CardId, CardPlay, CombatState, Engine, Flow, PileType};
use crate::content::cards::{CardType, pools};
use crate::content::powers::{PowerData, PowerModel, PowerType};

pub struct Calamity;

/// Attack plays at once (nested through `AutoPlay`) that one instance can track.
const SLOTS: usize = 8;

/// `CalamityPower.Data`: `amountsForPlayedCards`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    pub played_cards: [Option<(CardId, i32)>; SLOTS],
}

fn data_mut(s: &mut CombatState, me: PowerRef) -> &mut Data {
    match s.power_data_mut(me.id) {
        PowerData::Calamity(data) => data,
        _ => unreachable!("a Calamity instance holds Calamity data"),
    }
}

impl PowerModel for Calamity {
    /// It adds random Attacks of the pool to the hand (Headbutt among them), which a later
    /// play or Stampede's auto-play can prompt for, so the combat must snapshot while it is up.
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "Calamity"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Buff
    }

    fn init_internal_data(&self) -> PowerData {
        PowerData::Calamity(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for Calamity {
    type Me = PowerRef;

    fn before_card_played(&self, eng: &mut Engine<'_>, me: PowerRef, play: &CardPlay) -> Flow {
        // The player owns every card.
        if eng.s.card(play.card).card_type() != CardType::Attack {
            return Ok(());
        }
        let amount = eng.s.power(me.owner, me.id).amount;
        let slots = &mut data_mut(eng.s, me).played_cards;
        // `Dictionary.Add` throws on a duplicate key.
        assert!(
            slots.iter().flatten().all(|&(card, _)| card != play.card),
            "Calamity: a card is played twice at once"
        );
        let slot = slots
            .iter_mut()
            .find(|slot| slot.is_none())
            .expect("Calamity tracks at most SLOTS nested plays");
        *slot = Some((play.card, amount));
        Ok(())
    }

    fn after_card_played(&self, eng: &mut Engine<'_>, me: PowerRef, play: &CardPlay) -> Flow {
        let slots = &mut data_mut(eng.s, me).played_cards;
        let Some(slot) = slots
            .iter_mut()
            .find(|slot| slot.is_some_and(|(card, _)| card == play.card))
        else {
            return Ok(());
        };
        *slot = None;
        let count = usize::try_from(eng.s.power(me.owner, me.id).amount).unwrap_or(0);
        let attacks: Vec<_> = pools::character(eng.s.player.character)
            .into_iter()
            .filter(|c| c.card_type == CardType::Attack)
            .collect();
        let cards = pools::get_for_combat(eng, &attacks, count)?;
        let player = eng.s.player.creature;
        for card in cards {
            eng.add_generated_card_to_combat(card, PileType::Hand, Some(player))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::content::powers::PowerKind;

    /// Regression: the generated Attacks can include a model that prompts (Headbutt), so a
    /// combat holding only this power must snapshot its steps; before, an `EndTurn` whose
    /// Stampede played such a card panicked with "no model in combat reports `may_prompt`".
    #[test]
    fn calamity_may_prompt() {
        assert!(PowerKind::Calamity.may_prompt());
    }
}
