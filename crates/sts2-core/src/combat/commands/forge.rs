//! `ForgeCmd.Forge` (`Commands/ForgeCmd.cs:36-80`).

use crate::combat::engine::{Engine, Flow};
use crate::combat::hooks::Listener;
use crate::combat::state::{CardId, PileType};
use crate::content::cards::{CardKind, sovereign_blade};
use crate::decimal::Decimal;

impl Engine<'_> {
    /// `ForgeCmd.Forge(amount, player, source)` for the only player: nothing once combat is over
    /// or ending. Without a Sovereign Blade outside the Exhaust pile, a new one is created and
    /// added to the hand as a generated card (`AddGeneratedCardToCombat`, creator the player);
    /// then every blade, exhausted ones included, gains `amount` damage
    /// (`IncreaseSovereignBladeDamage`), and `AfterForge` runs. Returns the blades outside the
    /// Exhaust pile.
    ///
    /// `GetSovereignBlades` skips dupes (`IsDupe`).
    pub(crate) fn forge(&mut self, amount: Decimal, source: Option<Listener>) -> Flow<Vec<CardId>> {
        if self.hooked().is_over_or_ending() {
            return Ok(Vec::new());
        }
        let mut blades = self.sovereign_blades(false);
        if blades.is_empty() {
            // `CreatedThroughForge = true` only changes the blade's VFX.
            let blade = self.create_card(CardKind::SovereignBlade);
            let me = self.s.player.creature;
            self.add_generated_card_to_combat(blade, PileType::Hand, Some(me))?;
            blades.push(blade);
        }
        for blade in self.sovereign_blades(true) {
            sovereign_blade::add_damage(self.s.card_mut(blade), amount);
        }
        self.after_forge(amount, source)?;
        Ok(blades)
    }

    /// `GetSovereignBlades(player, includeExhausted)`: the blades in `AllCards` (pile order).
    fn sovereign_blades(&self, include_exhausted: bool) -> Vec<CardId> {
        PileType::LISTENER_ORDER
            .into_iter()
            .filter(|&p| include_exhausted || p != PileType::Exhaust)
            .flat_map(|p| self.s.player.pile(p).iter().copied())
            .filter(|&c| {
                let card = self.s.card(c);
                card.kind == CardKind::SovereignBlade && !card.is_dupe
            })
            .collect()
    }
}
