//! `IAmInvincible` (`Models.Cards/IAmInvincible.cs`): Regent rare skill, cost 1; gain 10 block (13
//! upgraded). While it is the top card of the draw pile it plays itself when the turn's play
//! phase ends, `AfterAutoPostPlayPhaseEntered`.

use crate::combat::commands::Position;
use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow, ValueProps};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct IAmInvincible;

#[sts2_macros::hooks]
impl Hooks for IAmInvincible {
    type Me = CardId;

    fn after_auto_post_play_phase_entered(&self, eng: &mut Engine<'_>, me: CardId) -> Flow {
        // `player == Owner`: the one player.
        if eng.s.player.draw.first() == Some(&me) {
            eng.auto_play_from_draw_pile(1, Position::Top, false)?;
        }
        Ok(())
    }
}

impl CardModel for IAmInvincible {
    /// Auto-plays the top draw-pile card; an auto-played card can open its own choice (Headbutt, Burning Pact),
    /// so the combat must snapshot its steps while this is in play.
    fn may_prompt(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "I Am Invincible"
    }

    fn cost(&self) -> i32 {
        1
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

    fn gains_block(&self) -> bool {
        true
    }

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Block, 10).upgrade(3)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let block = eng.s.card(play.card).var(Var::Block);
        let player = eng.s.player.creature;
        eng.gain_block(player, block, ValueProps::MOVE, Some(play))?;
        Ok(())
    }
}
