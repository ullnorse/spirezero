//! `ChainsOfBindingPower` (`Models.Powers/ChainsOfBindingPower.cs`): a debuff (`Counter`) that
//! `Queen`'s Puppet Strings puts on the player. On the owner's side's turn, each card the
//! owner's player draws is afflicted with `Bound` (amount = this power's amount) while fewer than
//! `Amount` cards were afflicted with `Bound` by the owner this turn. Once a Bound card is played,
//! no other Bound card can be played until the owner's turn ends, which clears every `Bound`.

use crate::combat::AutoPlayType;
use crate::combat::history::{HistoryKind, HistoryTag};
use crate::combat::hooks::{Hooks, PowerRef};
use crate::combat::{CardId, CardPlay, CombatState, CreatureId, Engine, Flow, Side};
use crate::content::afflictions::AfflictionKind;
use crate::content::powers::tangled::all_cards;
use crate::content::powers::{PowerData, PowerModel, PowerType};
use crate::decimal::Decimal;

pub struct ChainsOfBinding;

/// `ChainsOfBindingPower.Data`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `boundCardPlayed`.
    pub bound_card_played: bool,
}

fn bound_card_played(s: &CombatState, me: PowerRef) -> bool {
    match *s.power_data(me.id) {
        PowerData::ChainsOfBinding(data) => data.bound_card_played,
        _ => unreachable!("a ChainsOfBinding instance holds ChainsOfBinding data"),
    }
}

fn set_bound_card_played(s: &mut CombatState, me: PowerRef, value: bool) {
    s.set_power_data(
        me.id,
        PowerData::ChainsOfBinding(Data {
            bound_card_played: value,
        }),
    );
}

fn is_bound(s: &CombatState, card: CardId) -> bool {
    s.card(card)
        .affliction
        .is_some_and(|a| a.kind == AfflictionKind::Bound)
}

impl PowerModel for ChainsOfBinding {
    fn name(&self) -> &'static str {
        "Chains of Binding"
    }

    fn power_type(&self) -> PowerType {
        PowerType::Debuff
    }

    fn init_internal_data(&self) -> PowerData {
        PowerData::ChainsOfBinding(Data::default())
    }
}

#[sts2_macros::hooks]
impl Hooks for ChainsOfBinding {
    type Me = PowerRef;

    /// `card.Owner == Owner.Player`: the owner is the only player, who owns every card.
    fn after_card_drawn(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        card: CardId,
        _from_hand_draw: bool,
    ) -> Flow {
        let s = &*eng.s;
        if me.owner != s.player.creature
            || s.side != s.creature(me.owner).side
            || !AfflictionKind::Bound.can_afflict(s, card)
        {
            return Ok(());
        }
        // `CardAfflictedEntry`s of this turn whose actor is the owner and affliction `Bound`.
        let afflicted = s
            .history_this_turn(HistoryTag::CardAfflicted)
            .iter()
            .filter(|e| {
                e.actor == me.owner
                    && matches!(
                        e.kind,
                        HistoryKind::CardAfflicted {
                            affliction: AfflictionKind::Bound,
                            ..
                        }
                    )
            })
            .count();
        let amount = s.power(me.owner, me.id).amount;
        if i32::try_from(afflicted).unwrap_or(i32::MAX) < amount {
            // `AfflictAndPreview<Bound>([card], Amount, None)`.
            eng.afflict(card, AfflictionKind::Bound, Decimal::from(amount));
        }
        Ok(())
    }

    /// Dupes do not count (`IsDupe`); every card is the owner's player's when the owner is the
    /// player.
    fn before_card_played(&self, eng: &mut Engine<'_>, me: PowerRef, play: &CardPlay) -> Flow {
        // `card.IsDupe`: a dupe of a Bound card does not count.
        if me.owner == eng.s.player.creature
            && !eng.s.card(play.card).is_dupe
            && is_bound(eng.s, play.card)
        {
            set_bound_card_played(eng.s, me, true);
        }
        Ok(())
    }

    /// `ShouldPlay(card, _)`: the type of play is ignored.
    fn should_play(
        &self,
        s: &CombatState,
        me: PowerRef,
        card: CardId,
        _auto_play: AutoPlayType,
    ) -> bool {
        me.owner != s.player.creature || !is_bound(s, card) || !bound_card_played(s, me)
    }

    fn before_side_turn_end(
        &self,
        eng: &mut Engine<'_>,
        me: PowerRef,
        _side: Side,
        participants: &[CreatureId],
    ) -> Flow {
        if !participants.contains(&me.owner) {
            return Ok(());
        }
        set_bound_card_played(eng.s, me, false);
        // `Owner.Player?.PlayerCombatState?.AllCards`: none for a non-player owner.
        if me.owner == eng.s.player.creature {
            for card in all_cards(eng.s) {
                if is_bound(eng.s, card) {
                    eng.clear_affliction(card);
                }
            }
        }
        Ok(())
    }
}
