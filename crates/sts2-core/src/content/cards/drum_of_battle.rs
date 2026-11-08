//! `DrumOfBattle` (`Models.Cards/DrumOfBattle.cs`): draw 2. Whenever this card is exhausted, gain 2
//! energy (3 upgraded) once per play count it would have (`GeneratePlayCount`).

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{CardModel, CardRarity, CardType, CardVar, TargetType, Var};

pub struct DrumOfBattle;

#[sts2_macros::hooks]
impl Hooks for DrumOfBattle {
    type Me = CardId;

    fn after_card_exhausted(
        &self,
        eng: &mut Engine<'_>,
        me: CardId,
        card: CardId,
        _caused_by_ethereal: bool,
    ) -> Flow {
        if card != me {
            return Ok(());
        }
        // GeneratePlayCount(CombatState, null) (`CardModel.cs:2029`).
        let c = eng.s.card(me);
        let replays = c.enchantment.map_or(c.base_replay_count, |e| {
            e.kind.model().enchant_play_count(e, c.base_replay_count)
        });
        let (play_count, modifiers) = eng.hooked().modify_card_play_count(me, None, replays + 1);
        eng.after_modifying_card_play_count(me, &modifiers)?;
        let energy = eng.s.card(me).var(Var::Energy);
        for _ in 0..play_count {
            eng.gain_energy(energy)?;
        }
        Ok(())
    }
}

impl CardModel for DrumOfBattle {
    fn name(&self) -> &'static str {
        "Drum of Battle"
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

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[
            CardVar::new(Var::Cards, 2),
            CardVar::new(Var::Energy, 2).upgrade(1),
        ];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        eng.draw(eng.s.card(play.card).var(Var::Cards), false)?;
        Ok(())
    }
}
