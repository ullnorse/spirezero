//! `PlayerCmd.GainStars`, `LoseStars` and `SetStars` (`Commands/PlayerCmd.cs:89-130`), and the
//! `PlayerCombatState.Stars` setter (`Entities.Players/PlayerCombatState.cs:103-117,210-226`).

use crate::combat::engine::{Engine, Flow};
use crate::combat::history::HistoryKind;
use crate::decimal::Decimal;

impl Engine<'_> {
    /// `PlayerCmd.GainStars(amount, player)` for the only player: nothing once combat is
    /// ending or if a listener's `ShouldGainStars` vetoes it; else `PlayerCombatState.GainStars`
    /// (`Stars = (int)max(Stars + amount, 0)`), then `AfterStarsGained((int)amount)`.
    ///
    /// # Panics
    /// For a negative amount, as `PlayerCombatState.GainStars` throws.
    pub(crate) fn gain_stars(&mut self, amount: Decimal) -> Flow {
        if self.hooked().is_ending() || !self.hooked().should_gain_stars(amount) {
            return Ok(());
        }
        assert!(amount >= Decimal::ZERO, "Must not be negative.");
        let stars = (Decimal::from(self.s.player.stars) + amount).max(Decimal::ZERO);
        self.set_player_stars(stars.to_int_truncating());
        self.after_stars_gained(amount.to_int_truncating())
    }

    /// `PlayerCmd.LoseStars(amount, player)` for the only player: nothing once combat is ending;
    /// else `PlayerCombatState.LoseStars`. No hooks.
    pub(crate) fn lose_stars(&mut self, amount: Decimal) {
        if self.hooked().is_ending() {
            return;
        }
        self.lose_stars_internal(amount);
    }

    /// `PlayerCmd.SetStars(amount, player)`: gain or lose the difference.
    #[expect(
        dead_code,
        reason = "no ported model sets stars yet (`PlayerCmd.SetStars` has no caller in the pool)"
    )]
    pub(crate) fn set_stars(&mut self, amount: Decimal) -> Flow {
        if self.hooked().is_ending() {
            return Ok(());
        }
        let stars = Decimal::from(self.s.player.stars);
        if stars < amount {
            self.gain_stars(amount - stars)?;
        } else if stars > amount {
            self.lose_stars(stars - amount);
        }
        Ok(())
    }

    /// `PlayerCombatState.LoseStars` (`Stars = (int)max(Stars - amount, 0)`), as paying for a
    /// card does it (no `IsEnding` check).
    ///
    /// # Panics
    /// For a negative amount, as the game throws.
    pub(crate) fn lose_stars_internal(&mut self, amount: Decimal) {
        assert!(amount >= Decimal::ZERO, "Must not be negative.");
        let stars = (Decimal::from(self.s.player.stars) - amount).max(Decimal::ZERO);
        self.set_player_stars(stars.to_int_truncating());
    }

    /// The `Stars` setter: a change logs `History.StarsModified(delta)`.
    fn set_player_stars(&mut self, stars: i32) {
        let old = self.s.player.stars;
        if old != stars {
            self.s.player.stars = stars;
            self.record(
                self.s.player.creature,
                HistoryKind::StarsModified(stars - old),
            );
        }
    }
}
