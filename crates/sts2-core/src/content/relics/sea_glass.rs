//! `SeaGlass` (`Models.Relics/SeaGlass.cs`): `Orobas` presets `CharacterId`; `AfterObtained`
//! creates 5 Common, 5 Uncommon and 5 Rare cards of that character's pool (uniform odds,
//! `NoCardPoolModifications`, each batch one `CreateForReward`) and adds the ones the player
//! picks from the grid (any number, `MinSelect` 0). Without a character it defaults to Ironclad.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::characters::Character;
use crate::content::relics::{RelicData, RelicModel};
use crate::run::odds::CardRarity;
use crate::run::{CardCreation, CardFilter, CardPool, RunCtx, RunError};

/// `CardsVar(15)`, divided over the three rarities.
const CARDS: usize = 15;
/// The batches' rarities, in order.
const RARITIES: [CardRarity; 3] = [CardRarity::Common, CardRarity::Uncommon, CardRarity::Rare];

pub struct SeaGlass;

/// `SeaGlass`'s fields.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Data {
    /// `CharacterId`.
    pub character: Option<Character>,
}

/// The relic's fields with `CharacterId` set to `character`.
#[must_use]
pub const fn for_character(character: Character) -> RelicData {
    RelicData::SeaGlass(Data {
        character: Some(character),
    })
}

impl RelicModel for SeaGlass {
    fn name(&self) -> &'static str {
        "Sea Glass"
    }

    fn init_data(&self) -> RelicData {
        RelicData::SeaGlass(Data::default())
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, me: usize) -> Result<(), RunError> {
        let s = ctx.state();
        let RelicData::SeaGlass(data) = &mut s.relics[me].data else {
            unreachable!("a SeaGlass has SeaGlass data");
        };
        let character = *data.character.get_or_insert(Character::Ironclad);
        let mut cards = Vec::with_capacity(CARDS);
        for rarity in RARITIES {
            let options = CardCreation::non_combat_uniform(
                CardPool::Of(character),
                CardFilter::Rarity(rarity),
            )
            .without_pool_modifications();
            cards.extend(ctx.create_cards(CARDS / RARITIES.len(), options));
        }
        ctx.select_cards_to_add_up_to(cards);
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for SeaGlass {
    type Me = RelicId;
}
