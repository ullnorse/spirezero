//! `HiddenGem` (`Models.Cards/HiddenGem.cs`): cost 1; a random draw-pile card gets 2 (3
//! upgraded) extra replays. It prefers an Attack, Skill or Power that is playable and has no
//! replays yet. Cannot be generated in combat.

use crate::combat::hooks::Hooks;
use crate::combat::{CardId, CardPlay, Engine, Flow};
use crate::content::cards::{
    CardKeyword, CardModel, CardRarity, CardType, CardVar, TargetType, Var,
};
use crate::rng::RunRngType;

pub struct HiddenGem;

#[sts2_macros::hooks]
impl Hooks for HiddenGem {
    type Me = CardId;
}

impl CardModel for HiddenGem {
    fn name(&self) -> &'static str {
        "Hidden Gem"
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

    fn vars(&self) -> &'static [CardVar] {
        const VARS: &[CardVar] = &[CardVar::new(Var::Named("Replay"), 2).upgrade(1)];
        VARS
    }

    fn on_play(&self, eng: &mut Engine<'_>, play: &CardPlay) -> Flow {
        let replay = eng.s.card(play.card).count_var(Var::Named("Replay"));
        let hooked = eng.hooked();
        let draw = &hooked.s.player.draw;
        if draw.is_empty() {
            return Ok(());
        }
        // Not Unplayable, not a Curse or Quest (`type - 5 <= 1`), and no replays yet
        // (`GetEnchantedReplayCount`: only `BaseReplayCount` is in scope).
        let candidates: Vec<CardId> = draw
            .iter()
            .copied()
            .filter(|&c| {
                let card = hooked.s.card(c);
                !hooked.has_keyword(c, CardKeyword::Unplayable)
                    && !matches!(card.card_type(), CardType::Curse | CardType::Quest)
                    && card.base_replay_count < 1
            })
            .collect();
        // `type - 1 <= 2`: Attack, Skill or Power.
        let preferred: Vec<CardId> = candidates
            .iter()
            .copied()
            .filter(|&c| {
                matches!(
                    eng.s.card(c).card_type(),
                    CardType::Attack | CardType::Skill | CardType::Power
                )
            })
            .collect();
        let items = if preferred.is_empty() {
            &candidates
        } else {
            &preferred
        };
        let picked = eng.s.rng[RunRngType::CombatCardSelection]
            .next_item(items)
            .copied();
        if let Some(card) = picked {
            // CardCmd.Preview has no game effect.
            eng.s.card_mut(card).base_replay_count += replay;
        }
        Ok(())
    }
}
