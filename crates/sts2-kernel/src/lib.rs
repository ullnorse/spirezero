//! An experimental fight kernel (task PF6): a `Copy`, fixed-capacity, heap-free fight state with
//! one table-driven interpreter, bit-exact with sts2-core on what it supports.
//!
//! A fight starts from an sts2-core [`sts2_core::Combat`] ([`Fight::from_core`]: sts2-core sets the
//! combat up and plays its first turn start), then [`Fight::legal_actions`] and [`Fight::step`]
//! mirror [`sts2_core::Combat::legal_actions`] and [`sts2_core::Combat::apply`], in the same
//! order, so a policy that picks by index plays the same fight on both. Anything outside the
//! supported content is refused by `from_core`, and a fight that outgrows a capacity mid-way
//! stops as [`Status::Unsupported`]. `tests/differential.rs` checks every step against sts2-core.
//! Design, scope and numbers: `docs/kernel-prototype.md`.

mod batch;
mod engine;
mod num;
mod project;
mod state;
mod tables;

pub use batch::{Lane, run_lockstep};
pub use num::{Factors, scaled_trunc};
pub use project::Unsupported;
pub use state::{
    Action, Card, Creature, Fight, Forced, MAX_CARDS, MAX_CREATURES, MAX_HAND, MAX_POWERS, Monster,
    NONE, PLAYER, Power, Status, pile,
};
pub use tables::{Affliction, Kernel, P};

use engine::Eng;

/// At most this many legal actions (10 hand cards on up to 5 enemies, plus ending the turn).
pub const MAX_ACTIONS: usize = 64;

/// A list of actions in [`Fight::legal_actions`] order.
#[derive(Clone, Copy, Debug)]
pub struct Actions {
    items: [Action; MAX_ACTIONS],
    len: usize,
}

impl Actions {
    /// An empty list.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            items: [Action::EndTurn; MAX_ACTIONS],
            len: 0,
        }
    }

    const fn push(&mut self, action: Action) {
        self.items[self.len] = action;
        self.len += 1;
    }

    /// The actions.
    #[must_use]
    pub fn as_slice(&self) -> &[Action] {
        &self.items[..self.len]
    }

    /// How many there are.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Whether there are none.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl Default for Actions {
    fn default() -> Self {
        Self::new()
    }
}

/// An action [`Fight::step`] refused; the fight is unchanged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Illegal;

impl Fight {
    /// Every action [`Fight::step`] accepts now, in sts2-core's order: each hand card (in hand
    /// order) on each valid target (in `Enemies` order), then ending the turn; or each pick of a
    /// pending choice.
    #[must_use]
    pub fn legal_actions(&self, k: &Kernel) -> Actions {
        let mut out = Actions::new();
        self.legal_actions_into(k, &mut out);
        out
    }

    /// [`Fight::legal_actions`] into `out` (cleared first), which a caller can reuse across steps.
    pub fn legal_actions_into(&self, k: &Kernel, out: &mut Actions) {
        out.len = 0;
        match self.status {
            Status::Won | Status::Lost | Status::Unsupported => {}
            Status::Decision => {
                for &card in self.hand() {
                    out.push(Action::Pick(card));
                }
            }
            Status::PlayerTurn => {
                // `Fight::can_play` for every hand card, with what does not depend on the card
                // worked out once.
                let closed = self.over_or_ending();
                let player = self.player();
                let ringing = self.played_this_turn && player.has(P::Ringing) && !closed;
                let tangled = if closed { 0 } else { player.amount(P::Tangled) };
                let mut targets = [0; MAX_CREATURES];
                let mut n_targets = 0;
                for &e in self.enemies() {
                    if self.creature(e).alive() {
                        targets[n_targets] = e;
                        n_targets += 1;
                    }
                }
                for &card in self.hand() {
                    let c = self.card(card);
                    let spec = &k.cards[usize::from(c.spec)];
                    let mut cost = spec.cost;
                    if cost >= 0 && c.affliction == Affliction::Entangled {
                        cost += tangled;
                    }
                    if spec.unplayable
                        || self.energy < cost.max(0)
                        || (ringing && c.affliction == Affliction::Ringing)
                    {
                        continue;
                    }
                    if spec.targeted {
                        for &target in &targets[..n_targets] {
                            out.push(Action::Play { card, target });
                        }
                    } else {
                        out.push(Action::Play { card, target: NONE });
                    }
                }
                out.push(Action::EndTurn);
            }
        }
    }

    /// Whether `action` is legal now.
    #[must_use]
    pub fn is_legal(&self, k: &Kernel, action: Action) -> bool {
        match (self.status, action) {
            (Status::PlayerTurn, Action::EndTurn) => true,
            (Status::PlayerTurn, Action::Play { card, target }) => {
                usize::from(card) < usize::from(self.n_cards)
                    && self.card(card).pile == pile::HAND
                    && (target == NONE || usize::from(target) < usize::from(self.n_creatures))
                    && self.can_play(k, card, target)
            }
            (Status::Decision, Action::Pick(card)) => self.hand().contains(&card),
            _ => false,
        }
    }

    /// Applies a player action and returns where the fight stands.
    ///
    /// # Errors
    /// If the action is not legal now; the fight is then unchanged.
    pub fn step(&mut self, k: &Kernel, action: Action) -> Result<Status, Illegal> {
        if !self.is_legal(k, action) {
            return Err(Illegal);
        }
        Ok(self.step_unchecked(k, action))
    }

    /// [`Fight::step`] for an action taken from [`Fight::legal_actions`].
    pub fn step_unchecked(&mut self, k: &Kernel, action: Action) -> Status {
        let result = Eng { f: self, k }.act(action);
        self.status = match (result, self.status) {
            (Err(engine::Stop), _) => Status::Unsupported,
            (Ok(()), Status::Won | Status::Lost) => self.status,
            (Ok(()), _) if self.pending != NONE => Status::Decision,
            (Ok(()), _) => Status::PlayerTurn,
        };
        self.status
    }
}

#[cfg(test)]
mod tests {
    use super::{Fight, Monster, Power};

    #[test]
    fn the_fight_state_stays_small() {
        // A copy is one memcpy; keep it within a few cache lines per creature.
        let size = size_of::<Fight>();
        assert!(size <= 2_048, "Fight is {size} bytes");
        assert!(size_of::<Power>() <= 16);
        assert!(size_of::<Monster>() <= 16);
    }
}
