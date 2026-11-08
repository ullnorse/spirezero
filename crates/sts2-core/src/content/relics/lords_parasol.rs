//! `LordsParasol` (`Models.Relics/LordsParasol.cs`): Ancient (Vakuu); `AfterRoomEntered` of a
//! merchant room runs `PurchaseEverything`: every entry bought with `ignoreCost` (no gold lost,
//! `AfterItemPurchased` sees 0 spent) in order, the character cards (out-of-stock ones skipped),
//! the colorless cards, the relics, the potions (a refused one stays), then the card removal,
//! not cancelable. In the game it starts after the room is shown (`TaskHelper.RunSafely` behind
//! UI waits), so after every relic's `AfterRoomEntered`.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::run::{RunState, Shop, ShopEntry};

pub struct LordsParasol;

impl RelicModel for LordsParasol {
    fn name(&self) -> &'static str {
        "Lord's Parasol"
    }

    fn after_merchant_room_entered(&self, _s: &RunState, _me: usize, shop: &mut Shop) {
        // `Shop::cards` are the character cards, then the colorless ones.
        let cards = (0..shop.cards.len()).map(ShopEntry::Card);
        let relics = (0..shop.relics.len()).map(ShopEntry::Relic);
        let potions = (0..shop.potions.len()).map(ShopEntry::Potion);
        shop.free_purchases
            .extend(cards.chain(relics).chain(potions));
        shop.free_purchases.push(ShopEntry::Removal);
    }
}

#[sts2_macros::hooks]
impl Hooks for LordsParasol {
    type Me = RelicId;
}
