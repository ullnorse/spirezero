//! `Fetch` (`Models.Cards/Fetch.cs`): cost 0, Osty hits for 3 (6 upgraded); the first play of
//! this card each turn also draws 1 card. Does nothing without a living Osty.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, CombatState, Engine, Flow, HistoryKind, HistoryTag};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct Fetch;

#[sts2_macros::hooks]
impl Hooks for Fetch {
    type Me = CardId;
}

impl CardModel for Fetch {
    fn name(&self) -> &'static str {
        "Fetch"
    }

    fn cost(&self) -> i32 {
        0
    }

    fn card_type(&self) -> CardType {
        CardType::Attack
    }

    fn rarity(&self) -> CardRarity {
        CardRarity::Uncommon
    }

    fn target(&self) -> TargetType {
        TargetType::AnyEnemy
    }

    fn is_osty_attack(&self) -> bool {
        true
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::OstyDamage, 3).upgrade(3),
            CardVar::new(Var::Cards, 1),
        ];
        VARS
    }

    /// `ShouldGlowGoldInternal`: `!HasBeenPlayedThisTurn`.
    fn glows_gold(&self, s: &CombatState, me: CardId) -> bool {
        !has_been_played_this_turn(s, me)
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Fetch needs a target");
        // Osty.CheckMissingWithAnim.
        if !eng.s.is_osty_alive() {
            return Ok(());
        }
        let damage = eng.s.card(play.card).var(Var::OstyDamage);
        let osty = eng.s.osty().expect("Osty is alive");
        eng.attack(&Attack::new(
            osty,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        if !has_been_played_this_turn(eng.s, play.card) {
            eng.draw(eng.s.card(play.card).var(Var::Cards), false)?;
        }
        Ok(())
    }
}

/// `HasBeenPlayedThisTurn`: a finished play of this card this turn.
fn has_been_played_this_turn(s: &CombatState, card: CardId) -> bool {
    let me = s.player.creature;
    s.history_this_turn(HistoryTag::CardPlayFinished)
        .iter()
        .any(|e| {
            e.actor == me
                && matches!(&e.kind, HistoryKind::CardPlayFinished { play, .. } if play.card == card)
        })
}
