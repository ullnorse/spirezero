//! `TinkerTime` (`Models.Events/TinkerTime.cs`): build a `MadScience`. The second page offers 2 of
//! Attack, Skill and Power (`TakeRandom(2, Rng)`), the third 2 of the chosen type's 3 riders (the
//! same), and the chosen rider adds the card to the deck (`CardPileCmd.Add`).

use crate::content::cards::CardType;
use crate::content::cards::mad_science::{self, Rider};
use crate::content::events::{EventAction, EventCtx, EventData, EventModel, EventOption, Page};
use crate::run::{RunCard, RunError};
use crate::shuffle::unstable_shuffle;

pub struct TinkerTime;

/// `TakeRandom(2, Rng)`: `UnstableShuffle` of the whole list, then the first 2.
fn take_two<T>(ctx: &mut EventCtx<'_>, mut items: Vec<T>) -> Vec<T> {
    unstable_shuffle(&mut items, ctx.rng);
    items.truncate(2);
    items
}

/// A rider's name in its `GetRiderLocKey` text key, which is also its option's callback tag.
fn rider_name(rider: Rider) -> &'static str {
    match rider {
        Rider::None => unreachable!("None is not a valid rider"),
        Rider::Sapping => "SAPPING",
        Rider::Violence => "VIOLENCE",
        Rider::Choking => "CHOKING",
        Rider::Energized => "ENERGIZED",
        Rider::Wisdom => "WISDOM",
        Rider::Chaos => "CHAOS",
        Rider::Expertise => "EXPERTISE",
        Rider::Curious => "CURIOUS",
        Rider::Improvement => "IMPROVEMENT",
    }
}

/// `ChooseRiderEffect`'s riders for a card type.
fn riders(card_type: CardType) -> Vec<Rider> {
    match card_type {
        CardType::Attack => vec![Rider::Sapping, Rider::Violence, Rider::Choking],
        CardType::Skill => vec![Rider::Energized, Rider::Wisdom, Rider::Chaos],
        CardType::Power => vec![Rider::Expertise, Rider::Curious, Rider::Improvement],
        _ => unreachable!("TinkerTime offers Attack, Skill and Power"),
    }
}

impl EventModel for TinkerTime {
    fn begin(&self, _ctx: &mut EventCtx<'_>) -> Result<Vec<EventOption>, RunError> {
        Ok(vec![EventOption::new(
            "TINKER_TIME.pages.INITIAL.options.CHOOSE_CARD_TYPE",
            "CHOOSE_CARD_TYPE",
        )])
    }

    fn choose(&self, action: EventAction, ctx: &mut EventCtx<'_>) -> Result<Page, RunError> {
        let card_type = match action.tag {
            "CHOOSE_CARD_TYPE" => {
                let options = vec![
                    EventOption::new(
                        "TINKER_TIME.pages.CHOOSE_CARD_TYPE.options.ATTACK",
                        "ATTACK",
                    ),
                    EventOption::new("TINKER_TIME.pages.CHOOSE_CARD_TYPE.options.SKILL", "SKILL"),
                    EventOption::new("TINKER_TIME.pages.CHOOSE_CARD_TYPE.options.POWER", "POWER"),
                ];
                return Ok(Page::Next(take_two(ctx, options)));
            }
            "ATTACK" => CardType::Attack,
            "SKILL" => CardType::Skill,
            "POWER" => CardType::Power,
            tag => {
                // `RiderChosen(rider)`.
                let EventData::TinkerTime(card_type) = *ctx.data else {
                    unreachable!("a rider is chosen after the card type");
                };
                let rider = Rider::ALL
                    .into_iter()
                    .find(|&r| r != Rider::None && rider_name(r) == tag)
                    .expect("a rider option");
                let data = mad_science::Data {
                    tinker_time_type: Some(card_type),
                    tinker_time_rider: rider,
                };
                let mut card = RunCard::new("MAD_SCIENCE");
                card.counter = mad_science::counter(data);
                ctx.state().add_card(card);
                return Ok(Page::finished("TINKER_TIME", "DONE"));
            }
        };
        // `ChosenCardType = type; ChooseRiderEffect()`.
        *ctx.data = EventData::TinkerTime(card_type);
        let riders = take_two(ctx, riders(card_type));
        Ok(Page::Next(
            riders
                .into_iter()
                .map(|r| {
                    let name = rider_name(r);
                    EventOption::new(
                        format!("TINKER_TIME.pages.CHOOSE_RIDER.options.{name}"),
                        name,
                    )
                })
                .collect(),
        ))
    }
}
