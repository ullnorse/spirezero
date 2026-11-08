//! The text form of [`RelicData`] ([`crate::text`]): the variant's name, then its fields in
//! declaration order. The match arms destructure every field without `..`, so a field added
//! to a relic's `Data` does not compile until it is listed here.

use super::RelicData;
use crate::text::{Reader, Text, TextError, Writer};

/// `Text` for `RelicData` over its variants: `Variant(module { field, ... })`.
macro_rules! relic_data_text {
    ($($variant:ident($module:ident { $($field:ident),* }),)*) => {
        impl Text for RelicData {
            fn write(&self, w: &mut Writer) {
                match self {
                    Self::None => w.token("None"),
                    $(Self::$variant(super::$module::Data { $($field),* }) => {
                        w.token(stringify!($variant));
                        $($field.write(w);)*
                    })*
                    #[cfg(test)]
                    Self::TestMapMarker(super::test_map_marker::Data { marked }) => {
                        w.token("TestMapMarker");
                        marked.write(w);
                    }
                }
            }

            fn read(r: &mut Reader<'_>) -> Result<Self, TextError> {
                let variant = r.token()?;
                if variant == "None" {
                    return Ok(Self::None);
                }
                $(if variant == stringify!($variant) {
                    return Ok(Self::$variant(super::$module::Data { $($field: Text::read(r)?),* }));
                })*
                #[cfg(test)]
                if variant == "TestMapMarker" {
                    return Ok(Self::TestMapMarker(super::test_map_marker::Data {
                        marked: Text::read(r)?,
                    }));
                }
                Err(TextError(format!("unknown relic data {variant:?}")))
            }
        }
    };
}

relic_data_text! {
    ArchaicTooth(archaic_tooth { starter_card, ancient_card }),
    ArtOfWar(art_of_war { any_attacks_played_last_turn, any_attacks_played_this_turn }),
    BeatingRemnant(beating_remnant { damage_received_this_turn }),
    BeltBuckle(belt_buckle { dexterity_applied }),
    BingBong(bing_bong { cards_to_skip, adding_clone }),
    BoneTea(bone_tea { combats_left }),
    BookOfFiveRings(book_of_five_rings { cards_added }),
    BrilliantScarf(brilliant_scarf { cards_played_this_turn }),
    BurningSticks(burning_sticks { was_used_this_combat }),
    CentennialPuzzle(centennial_puzzle { used_this_combat }),
    DemonTongue(demon_tongue { triggered_this_turn }),
    DustyTome(dusty_tome { ancient_card }),
    EmberTea(ember_tea { combats_left }),
    EmotionChip(emotion_chip { last_unblocked_hit_turn }),
    FishingRod(fishing_rod { combats_seen }),
    FurCoat(fur_coat { fur_coat_act_index, cols, rows, len, coords_set }),
    GalacticDust(galactic_dust { stars_spent }),
    Girya(girya { times_lifted }),
    GoldenCompass(golden_compass { golden_path_act }),
    HappyFlower(happy_flower { turns_seen }),
    IronClub(iron_club { cards_played }),
    JossPaper(joss_paper { cards_exhausted, ethereal_count }),
    Kunai(kunai { attacks_played_this_turn }),
    Kusarigama(kusarigama { attacks_played_this_turn }),
    LastingCandy(lasting_candy { combat_rewards_seen }),
    LavaLamp(lava_lamp { took_damage_this_combat }),
    LavaRock(lava_rock { has_triggered }),
    LetterOpener(letter_opener { skills_played_this_turn }),
    LizardTail(lizard_tail { was_used }),
    MawBank(maw_bank { has_item_been_bought }),
    Metronome(metronome { orbs_channeled }),
    MiniRegent(mini_regent { used_this_turn }),
    MusicBox(music_box { was_used_this_turn, card_being_played }),
    Nunchaku(nunchaku { attacks_played }),
    Orichalcum(orichalcum { should_trigger }),
    OrnamentalFan(ornamental_fan { attacks_played_this_turn }),
    PaelsEye(paels_eye { used_this_combat, was_owner_part_of_last_player_turn }),
    PaelsLegion(paels_legion { cooldown, triggered_block_last_turn, affected_card_play }),
    PaelsTears(paels_tears { had_leftover_energy }),
    PaelsTooth(paels_tooth { cards }),
    PaelsWing(paels_wing { rewards_sacrificed }),
    Pendulum(pendulum { turns_seen }),
    PenNib(pen_nib { attacks_played, attack_to_double }),
    Permafrost(permafrost { activated_this_combat }),
    Pocketwatch(pocketwatch { cards_played_this_turn, cards_played_last_turn }),
    PollinousCore(pollinous_core { turns_seen }),
    PumpkinCandle(pumpkin_candle { kindle_count }),
    RainbowRing(rainbow_ring { attacks, skills, powers, activations }),
    RedSkull(red_skull { strength_applied }),
    Regalite(regalite { used_this_turn }),
    RuinedHelmet(ruined_helmet { used_this_combat }),
    SeaGlass(sea_glass { character }),
    Shuriken(shuriken { attacks_played_this_turn }),
    SilkenTress(silken_tress { is_used }),
    SilverCrucible(silver_crucible { times_used, treasure_rooms_entered }),
    SwordOfStone(sword_of_stone { elites_defeated }),
    TeaOfDiscourtesy(tea_of_discourtesy { combats_left }),
    ThrowingAxe(throwing_axe { used_this_combat }),
    TouchOfOrobas(touch_of_orobas { starter_relic, upgraded_relic }),
    ToyBox(toy_box { combats_seen }),
    TuningFork(tuning_fork { skills_played }),
    UnsettlingLamp(unsettling_lamp { triggering_card, doubled_temporary_stat, is_finished_triggering }),
    Vambrace(vambrace { triggering_card, block_gained_this_combat }),
    VelvetChoker(velvet_choker { cards_played_this_turn }),
    VenerableTeaSet(venerable_tea_set { gain_energy_in_next_combat }),
    WingedBoots(winged_boots { times_used }),
    WongosMysteryTicket(wongos_mystery_ticket { combats_finished, gave_relic }),
}
