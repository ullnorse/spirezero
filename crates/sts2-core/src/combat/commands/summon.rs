//! `OstyCmd.Summon` (`OstyCmd.cs:35-91`) and `PlayerCmd.AddPet<T>` (`PlayerCmd.cs:244-264`).

use crate::combat::engine::{Engine, Event, Flow};
use crate::combat::hooks::HookSet;
use crate::combat::state::{Creature, CreatureId, PowerKinds, Side};
use crate::content::monsters::{Monster, MonsterKind, Osty};
use crate::content::powers::PowerKind;
use crate::decimal::Decimal;

impl Engine<'_> {
    /// `OstyCmd.Summon(summoner, amount)`: grow a living Osty, revive a dead one with max HP
    /// reset to `amount`, or create Osty with `DieForYouPower`.
    pub(crate) fn summon_osty(&mut self, amount: Decimal) -> Flow {
        // Hook.ModifySummonAmount: no in-scope listener.
        if amount == Decimal::ZERO {
            return Ok(());
        }
        if self.s.is_osty_alive() {
            let osty = self.s.osty().expect("Osty is alive");
            self.gain_max_hp(osty, amount)?;
        } else {
            let osty = if let Some(osty) = self.s.osty() {
                // Reviving: AddPetInternal is idempotent; Osty never left `pets`.
                osty
            } else {
                let osty = self.add_pet(MonsterKind::Osty(Osty));
                self.apply_power(osty, PowerKind::DieForYou, Decimal::ONE, None)?;
                osty
            };
            self.set_max_hp(osty, amount)?;
            self.heal(osty, amount)?;
            // Hook.AfterOstyRevived (when reviving): no in-scope listener.
        }
        self.emit(Event::Summoned {
            amount: amount.to_int_truncating(),
        });
        // Hook.AfterSummon: no in-scope listener.
        Ok(())
    }

    /// `PlayerCmd.AddPet<T>`: create the pet on the player side at its initial HP (no `Niche`
    /// draw: only the enemy side rolls a unique HP), then add it to the player's pets and to the
    /// allies (`CreatureCmd.Add`).
    pub(crate) fn add_pet(&mut self, kind: MonsterKind) -> CreatureId {
        let id =
            CreatureId(u32::try_from(self.s.creatures.len()).expect("creature count fits u32"));
        let owner = self.s.player.creature;
        let hp = kind.max_initial_hp(self.s.ascension);
        let rng = crate::combat::monster_rng(self.s, id);
        self.s.push_creature(Creature {
            id,
            side: Side::Player,
            hp,
            max_hp: hp,
            block: 0,
            powers: Vec::new(),
            removed_powers: Vec::new(),
            power_kinds: PowerKinds::default(),
            hooks: HookSet::EMPTY,
            monster_hooks: HookSet::EMPTY,
            monster: Some(Monster::new(kind, rng)),
            pet_owner: Some(owner),
            slot: None,
            infinite_hp_display: false,
        });
        self.s.player.pets.push(id);
        self.s.push_ally(id);
        // Hook.AfterCreatureAdded: no in-scope listener; pets never roll moves.
        id
    }
}
