//! C28: an enchantment's own vars previewed as the card's (`NCard.UpdateVisuals` runs
//! `UpdateDynamicVarPreview` over `Enchantment.DynamicVars` too).

use super::*;
use crate::content::cards::PreviewMode;
use crate::content::enchantments::{Enchantment, EnchantmentKind};

fn enchantment_var(c: &Combat, card: CardId, name: &str) -> Decimal {
    c.state
        .enchantment_preview(card, None, PreviewMode::Normal)
        .into_iter()
        .find(|&(n, _)| n == name)
        .expect("the enchantment has the var")
        .1
}

#[test]
fn adroit_block_counts_dexterity_in_the_hand() {
    // Adroit: `BlockVar(0, Move)` set to `Amount`; `BlockVar.UpdateCardPreview` runs
    // `Hook.ModifyBlock` for a hand card.
    let mut c = new_combat();
    let p = player(&c);
    with_engine(&mut c, |eng| {
        eng.apply_power(p, PowerKind::Dexterity, Decimal::from(2), Some(p))
    });
    let defend = give_card(&mut c, CardKind::DefendNecrobinder, 0);
    c.edit(|s| s.card_mut(defend).enchantment = Some(Enchantment::new(EnchantmentKind::Adroit, 3)));
    assert_eq!(enchantment_var(&c, defend, "Block"), Decimal::from(5));
    c.edit(|s| {
        s.remove_from_pile(defend);
        s.insert_into_pile(defend, PileType::Draw, None);
    });
    assert_eq!(enchantment_var(&c, defend, "Block"), Decimal::from(3));
    let card = *c.state.card(defend);
    assert_eq!(
        card.enchantment_preview_out_of_combat(),
        vec![("Block", Decimal::from(3))]
    );
}

#[test]
fn tezcataras_ember_damage_goes_through_its_own_enchantment() {
    // `DamageVar.UpdateCardPreview` adds the card's `EnchantDamageAdditive` to the
    // enchantment's own `DamageVar(3)`: 3 + 3, then Strength in the hand.
    let mut c = new_combat();
    let p = player(&c);
    with_engine(&mut c, |eng| {
        eng.apply_power(p, PowerKind::Strength, Decimal::from(2), Some(p))
    });
    let strike = give_card(&mut c, CardKind::StrikeNecrobinder, 0);
    c.state.card_mut(strike).enchantment =
        Some(Enchantment::new(EnchantmentKind::TezcatarasEmber, 1));
    assert_eq!(enchantment_var(&c, strike, "Damage"), Decimal::from(8));
    let card = *c.state.card(strike);
    assert_eq!(
        card.enchantment_preview_out_of_combat(),
        vec![("Damage", Decimal::from(6))]
    );
    // An enchantment without vars has none.
    c.state.card_mut(strike).enchantment = Some(Enchantment::new(EnchantmentKind::Sharp, 2));
    assert_eq!(
        c.state
            .enchantment_preview(strike, None, PreviewMode::Normal),
        []
    );
}
