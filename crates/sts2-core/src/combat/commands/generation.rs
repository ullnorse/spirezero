//! Combat card generation (`CardFactory.GetDistinctForCombat`) and the choose-a-card screen
//! (`CardSelectCmd.FromChooseACardScreen`).

use crate::combat::engine::{Engine, Flow, Prompt, PromptSource, PromptText};
use crate::combat::state::CardId;
use crate::content::cards::pools::{self, PoolCard};
use crate::rng::RunRngType;
use crate::shuffle::unstable_shuffle;

impl Engine<'_> {
    /// `CardFactory.GetDistinctForCombat(player, cards, count, Rng.CombatCardGeneration)`
    /// (`CardFactory.cs:119-129`): `FilterForCombat`, then `TakeRandom` (an `UnstableShuffle`
    /// of the whole filtered list, then the first `count`), each created with
    /// `CombatState.CreateCard` (in no pile; add them with
    /// [`Engine::add_generated_card_to_combat`]). `cards` comes from `GetUnlockedCards`, so
    /// `FilterForPlayerCount` changes nothing. A picked card that is not ported stops the
    /// combat ([`Unsupported`](crate::combat::Unsupported)) after the shuffle, as the game would create it.
    pub(crate) fn get_distinct_for_combat(
        &mut self,
        cards: &[PoolCard],
        count: usize,
    ) -> Flow<Vec<CardId>> {
        let mut options = pools::filter_for_combat(cards);
        unstable_shuffle(
            &mut options,
            &mut self.s.rng[RunRngType::CombatCardGeneration],
        );
        options
            .into_iter()
            .take(count)
            .map(|c| Ok(self.create_card(pools::ported(c)?)))
            .collect()
    }

    /// `CardSelectCmd.FromChooseACardScreen(cards, player, canSkip)` (`CardSelectCmd.cs:252`):
    /// pick at most one of up to 3 cards that are in no pile; the selector is always asked for
    /// 0 to 1 cards, whatever `canSkip` (which only changes the screen). No cards is a
    /// soft-lock report and `None`.
    pub(crate) fn choose_a_card(
        &mut self,
        source: PromptSource,
        cards: Vec<CardId>,
    ) -> Flow<Option<CardId>> {
        assert!(cards.len() <= 3, "FromChooseACardScreen only takes 3 cards");
        let prompt = Prompt::new(source, PromptText::ChooseACard);
        Ok(self.choose_cards(prompt, cards, 0, 1)?.first().copied())
    }
}
