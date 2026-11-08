//! `Ftl` (`Models.Cards/Ftl.cs`): cost 0, 5 damage (6 upgraded); draw 1 card if fewer than `PlayMax` (3, 4 upgraded) cards
//! finished playing this turn.

use crate::combat::commands::{Attack, AttackDamage, AttackTargets};
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, CombatState, Engine, Flow, HistoryTag};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct Ftl;

#[sts2_macros::hooks]
impl Hooks for Ftl {
    type Me = CardId;
}

impl CardModel for Ftl {
    fn name(&self) -> &'static str {
        "FTL"
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

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Damage, 5).upgrade(1),
            CardVar::new(Var::Named("PlayMax"), 3).upgrade(1),
            CardVar::new(Var::Cards, 1),
        ];
        VARS
    }

    /// `ShouldGlowGoldInternal`: `CanDrawCard`.
    fn glows_gold(&self, s: &CombatState, me: CardId) -> bool {
        can_draw_card(s, me)
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let target = play.target.expect("Ftl needs a target");
        let damage = eng.s.card(play.card).var(Var::Damage);
        let player = eng.s.player.creature;
        eng.attack(&Attack::new(
            player,
            AttackTargets::Single(target),
            AttackDamage::Fixed(damage),
            Some(play.card),
        ))?;
        if can_draw_card(eng.s, play.card) {
            let cards = eng.s.card(play.card).var(Var::Cards);
            eng.draw(cards, false)?;
        }
        Ok(())
    }
}

/// `CanDrawCard`: fewer than `PlayMax` `CardPlaysFinished` of the owner this turn (in `OnPlay`,
/// this play is not finished yet).
fn can_draw_card(s: &CombatState, me: CardId) -> bool {
    let finished = s.history_this_turn(HistoryTag::CardPlayFinished).len();
    i32::try_from(finished).expect("a count fits i32") < s.card(me).int_var(Var::Named("PlayMax"))
}
