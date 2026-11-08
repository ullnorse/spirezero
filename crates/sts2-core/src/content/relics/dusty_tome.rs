//! `DustyTome` (`Models.Relics/DustyTome.cs`): `Darv` presets `AncientCard` (`SetupForPlayer`: a
//! random Ancient card of the character's pool, other than the `ArchaicTooth` transcendence
//! cards, on the player's `rewards` stream); `AfterObtained` adds it, upgraded, to the deck.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::cards::{CardRarity, pools};
use crate::content::relics::{RelicData, RelicModel, archaic_tooth};
use crate::rng::PlayerRngType;

use crate::run::{RunCard, RunCtx, RunError, RunState};

pub struct DustyTome;

/// `DustyTome`'s fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `AncientCard` (`ModelId.Entry`).
    pub ancient_card: Option<&'static str>,
}

/// `SetupForPlayer(player)`: the relic's fields, drawing the card on the player's `rewards`
/// stream.
///
/// # Panics
/// If the character's pool has no eligible Ancient card (the game throws).
pub fn setup_for_player(s: &mut RunState) -> RelicData {
    let items: Vec<&'static str> = pools::character(s.character)
        .into_iter()
        .filter(|c| {
            c.rarity == CardRarity::Ancient && !archaic_tooth::TRANSCENDENCE_CARDS.contains(&c.id)
        })
        .map(|c| c.id)
        .collect();
    let card = *s.player_rng[PlayerRngType::Rewards]
        .next_item(&items)
        .expect("the character has Ancient cards");
    RelicData::DustyTome(Data {
        ancient_card: Some(card),
    })
}

impl RelicModel for DustyTome {
    fn name(&self) -> &'static str {
        "Dusty Tome"
    }

    fn init_data(&self) -> RelicData {
        RelicData::DustyTome(Data::default())
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, me: usize) -> Result<(), RunError> {
        let s = ctx.state();
        let RelicData::DustyTome(data) = s.relics[me].data else {
            unreachable!("a DustyTome has DustyTome data");
        };
        // `ModelDb.GetById(null)` throws: only `Darv` offers it, set up.
        let id = data
            .ancient_card
            .ok_or(RunError::UnsupportedRelic("DUSTY_TOME"))?;
        let mut card = RunCard::new(id);
        card.upgrade();
        s.add_card(card);
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for DustyTome {
    type Me = RelicId;
}
