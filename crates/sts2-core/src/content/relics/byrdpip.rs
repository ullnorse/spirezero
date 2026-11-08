//! `Byrdpip` (`Models.Relics/Byrdpip.cs`): Event rarity, given by the Hatch rest option. Obtaining
//! it turns every `ByrdonisEgg` of the deck into `ByrdSwoop` (`CardCmd.TransformTo`, in deck
//! order); every combat then adds the `Byrdpip` pet (`BeforeCombatStart`). The skin
//! (`new Rng(Owner, Id, 0)`, a fresh stream) is cosmetic.

use crate::combat::hooks::Hooks;
use crate::combat::{Engine, Flow, RelicId};
use crate::content::monsters::{self, MonsterKind};
use crate::content::relics::RelicModel;
use crate::run::{RunCard, RunCtx, RunError};

pub struct Byrdpip;

impl RelicModel for Byrdpip {
    fn name(&self) -> &'static str {
        "Byrdpip"
    }

    fn after_obtained(&self, ctx: &mut RunCtx<'_>, _me: usize) -> Result<(), RunError> {
        let s = ctx.state();
        // Combat is never in progress here: the relic comes from a rest site.
        let eggs = s.deck.iter().filter(|c| c.id == "BYRDONIS_EGG").count();
        for _ in 0..eggs {
            let i = s
                .deck
                .iter()
                .position(|c| c.id == "BYRDONIS_EGG")
                .expect("an egg is left");
            s.transform_to(i, RunCard::new("BYRD_SWOOP"));
        }
        Ok(())
    }
}

#[sts2_macros::hooks]
impl Hooks for Byrdpip {
    type Me = RelicId;

    fn before_combat_start(&self, eng: &mut Engine<'_>, _me: RelicId) -> Flow {
        // `SummonPet`: `PlayerCmd.AddPet<Byrdpip>(Owner)`.
        eng.add_pet(MonsterKind::Byrdpip(monsters::Byrdpip));
        Ok(())
    }
}
