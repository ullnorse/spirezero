//! `ScrollBoxes` (`Models.Relics/ScrollBoxes.cs`): `AfterObtained` rolls 2 bundles of 2 Common
//! and 1 Uncommon character cards on the player's `rewards` stream (no card twice across the
//! bundles) and offers them on a choose-a-bundle screen; the chosen bundle joins the deck.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::characters::Character;
use crate::content::relics::RelicModel;
use crate::rng::PlayerRngType;
use crate::run::odds::CardRarity;
use crate::run::rewards::possible_cards;
use crate::run::{CardCreation, CardFilter, CardPool, RunCard, RunCtx, RunError, hooks};

/// Bundles offered (`GenerateRandomBundles`).
const BUNDLES: usize = 2;
/// A Defect's per-bundle roll for 3 `Claw`s: `NextInt(100) < 1`.
const CLAW_ROLL: i32 = 100;
/// Commons per bundle; one Uncommon follows.
const COMMONS: usize = 2;

pub struct ScrollBoxes;

impl RelicModel for ScrollBoxes {
    fn name(&self) -> &'static str {
        "Scroll Boxes"
    }

    /// `GenerateRandomBundles(Owner)`: the possible Commons and Uncommons (each options set
    /// through `ModifyCardRewardCreationOptions`), then per bundle 2 Commons and 1 Uncommon by
    /// `NextItem` among the cards no bundle has yet. A Defect's bundle is first rolled to be 3
    /// `Claw`s instead (`NextInt(100) < 1`).
    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        let s = ctx.state();
        let possible = |rarity| {
            let options =
                CardCreation::non_combat_uniform(CardPool::Character, CardFilter::Rarity(rarity));
            possible_cards(
                s.character,
                hooks::modify_card_reward_creation_options(s, options),
            )
        };
        let commons = possible(CardRarity::Common);
        let uncommons = possible(CardRarity::Uncommon);
        let mut used: Vec<&'static str> = Vec::new();
        let mut bundles = Vec::with_capacity(BUNDLES);
        let is_defect = s.character == Character::Defect;
        let rng = &mut s.player_rng[PlayerRngType::Rewards];
        for _ in 0..BUNDLES {
            if is_defect && rng.next_int(CLAW_ROLL) < 1 {
                bundles.push(vec![RunCard::new("CLAW"); COMMONS + 1]);
                continue;
            }
            let mut bundle = Vec::with_capacity(COMMONS + 1);
            let mut left: Vec<_> = commons.iter().filter(|c| !used.contains(&c.id)).collect();
            for _ in 0..COMMONS {
                let card = *rng.next_item(&left).expect("a Common to bundle");
                left.retain(|c| c.id != card.id);
                bundle.push(RunCard::new(card.id));
                used.push(card.id);
            }
            let left: Vec<_> = uncommons.iter().filter(|c| !used.contains(&c.id)).collect();
            let card = *rng.next_item(&left).expect("an Uncommon to bundle");
            bundle.push(RunCard::new(card.id));
            used.push(card.id);
            bundles.push(bundle);
        }
        ctx.choose_a_bundle(bundles);
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for ScrollBoxes {
    type Me = RelicId;
}
