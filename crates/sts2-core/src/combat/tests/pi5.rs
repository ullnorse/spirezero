//! Ironclad batch 5: paths the referee does not reach.

use super::*;
use crate::content::cards::{CardData, Var, rampage};

#[test]
fn rampage_keeps_its_growth_through_a_downgrade() {
    // Rampage.cs: each play adds `Increase` to the damage and to `_extraDamageFromPlays`;
    // `AfterDowngraded` (reset vars, then add the extra back) leaves `10 + extra`. No Act 1
    // source downgrades a card in a normal fight.
    let mut c = new_combat();
    let e = enemy(&c);
    c.state.player.energy = 9;
    let rampage = give_card(&mut c, CardKind::Rampage, 1);
    let hp = c.state.creature(e).hp;
    c.apply(Action::PlayCard {
        card: rampage,
        target: Some(e),
    })
    .expect("legal");
    // Upgraded: Increase is 10, damage 10 on the first play.
    assert_eq!(c.state.creature(e).hp, hp - 10);
    let data = CardData::Rampage(rampage::Data {
        extra_damage_from_plays: 10,
    });
    assert_eq!(c.state.card(rampage).data, data);
    with_engine(&mut c, |eng| {
        eng.downgrade(rampage);
        Ok(())
    });
    assert_eq!(c.state.card(rampage).upgrade_level, 0);
    assert_eq!(c.state.card(rampage).data, data);
}

#[test]
fn a_downgrade_in_combat_keeps_a_calculated_vars_upgraded_base_value() {
    // `DowngradeInternal` (`CardModel.cs:2149-2162`): the cloned `CalculatedVar` reads the old
    // `CalculationBase` (Dampen's downgrade of NoEscape+1); `Calculate` uses the new one.
    let mut c = new_combat();
    let no_escape = give_card(&mut c, CardKind::NoEscape, 1);
    with_engine(&mut c, |eng| {
        eng.downgrade(no_escape);
        Ok(())
    });
    let doom = Var::Named("CalculatedDoom");
    assert_eq!(c.state.card(no_escape).var(doom), Decimal::from(15));
    assert_eq!(
        c.state.calculate(no_escape, doom, None),
        Some(Decimal::from(10))
    );
    with_engine(&mut c, |eng| {
        eng.upgrade_card(no_escape);
        Ok(())
    });
    assert_eq!(c.state.card(no_escape).stale_calculated, None);
}
