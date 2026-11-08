//! `LeafyPoultice` (`Models.Relics/LeafyPoultice.cs`): `AfterObtained` loses 12 max HP, then
//! transforms the first Basic Strike and the first Basic Defend of the deck together
//! (`CardCmd.Transform`) on the player's `Transformations` stream.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::rng::PlayerRngType;
use crate::run::{RunCtx, RunError};

/// `MaxHpVar(12m)`.
const MAX_HP: i32 = 12;

pub struct LeafyPoultice;

impl RelicModel for LeafyPoultice {
    fn name(&self) -> &'static str {
        "Leafy Poultice"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        let s = ctx.state();
        s.lose_max_hp(MAX_HP);
        let basic =
            |f: fn(&crate::run::RunCard) -> bool| s.deck.iter().position(|c| c.is_basic() && f(c));
        let cards: Vec<usize> = [basic(|c| c.is_strike()), basic(|c| c.is_defend())]
            .into_iter()
            .flatten()
            .collect();
        let mut rng = s.player_rng[PlayerRngType::Transformations].clone();
        s.transform_many_to_random(&cards, &mut rng)?;
        s.player_rng[PlayerRngType::Transformations] = rng;
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for LeafyPoultice {
    type Me = RelicId;
}
