//! `StoneHumidifier` (`Models.Relics/StoneHumidifier.cs`): after a rest-site heal, 5 max HP.

use crate::combat::RelicId;
use crate::combat::hooks::Hooks;
use crate::content::relics::RelicModel;
use crate::run::RunState;

/// `MaxHpVar(5m)`.
const MAX_HP: i32 = 5;

pub struct StoneHumidifier;

impl RelicModel for StoneHumidifier {
    fn name(&self) -> &'static str {
        "Stone Humidifier"
    }

    fn after_rest_site_heal(&self, s: &mut RunState, _me: usize, _is_mimicked: bool) {
        s.gain_max_hp(MAX_HP);
    }
}

#[sts2_macros::hooks]
impl Hooks for StoneHumidifier {
    type Me = RelicId;
}
