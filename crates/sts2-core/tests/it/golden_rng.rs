//! `rng.json`: every `Rng` method, from fresh generators, plus the static weighted pick and the
//! argument-error cases. The Gaussians go through the host libm (`ln`, `sqrt`, `sin`, `cos`); the
//! golden values were generated on the game's .NET runtime and these tests compare them bit for bit.

use std::panic::{AssertUnwindSafe, catch_unwind};

use crate::common::{State, load};
use serde::Deserialize;
use serde_json::Value;
use sts2_core::Rng;
use sts2_core::rng::weighted_pick;

#[derive(Deserialize)]
struct Fixture {
    seeds: Vec<SeedCases>,
    weighted_static: Vec<WeightedStatic>,
}

#[derive(Deserialize)]
struct SeedCases {
    seed: u64,
    series: Vec<Series>,
    errors: Vec<ErrorCase>,
}

#[derive(Deserialize)]
struct Series {
    name: String,
    args: Vec<Value>,
    out: Vec<Value>,
    state_after: Option<Vec<State>>,
    #[serde(rename = "final")]
    final_state: State,
}

#[derive(Deserialize)]
struct ErrorCase {
    call: String,
    result: String,
    #[serde(rename = "final")]
    final_state: State,
}

#[derive(Deserialize)]
struct WeightedStatic {
    weights: Vec<u32>,
    inputs: Vec<u32>,
    out: Vec<i32>,
}

/// Typed access to a series' arguments.
struct Args<'a>(&'a [Value]);

impl Args<'_> {
    fn i32(&self, i: usize) -> i32 {
        i32::try_from(self.0[i].as_i64().expect("integer arg")).expect("i32 arg")
    }

    fn u32(&self, i: usize) -> u32 {
        u32::try_from(self.u64(i)).expect("u32 arg")
    }

    fn u64(&self, i: usize) -> u64 {
        self.0[i].as_u64().expect("unsigned arg")
    }

    fn f32(&self, i: usize) -> f32 {
        f32::from_bits(self.u32(i))
    }

    fn f64(&self, i: usize) -> f64 {
        f64::from_bits(self.u64(i))
    }

    fn list(&self, i: usize) -> &[Value] {
        self.0[i].as_array().expect("list arg")
    }

    fn i32_list(&self, i: usize) -> Vec<i32> {
        self.list(i)
            .iter()
            .map(|v| i32::try_from(v.as_i64().expect("int")).expect("i32"))
            .collect()
    }

    fn f32_list(&self, i: usize) -> Vec<f32> {
        self.list(i)
            .iter()
            .map(|v| f32::from_bits(u32::try_from(v.as_u64().expect("bits")).expect("u32")))
            .collect()
    }
}

/// `Rng.NextGaussianDouble(mean, stdDev, min, max)` (`Rng.cs:227-249`): Box-Muller with the cosine
/// branch, re-drawn until the standard-scale value is in `[0, 1]`. The engine needs only the integer
/// variant, so this lives here to check `ln`/`sqrt`/`cos` against the game's values too.
fn next_gaussian_double(rng: &mut Rng, mean: f64, std_dev: f64, min: f64, max: f64) -> f64 {
    loop {
        let d = 1.0 - rng.next_double();
        let u = 1.0 - rng.next_double();
        let r = (-2.0 * d.ln()).sqrt();
        let value = mean + r * (std::f64::consts::TAU * u).cos() * std_dev;
        if (0.0..=1.0).contains(&value) {
            return value * (max - min) + min;
        }
    }
}

/// One call of the named series, encoded like the generator does (floats as raw bits,
/// `default` as `0`).
fn call(rng: &mut Rng, name: &str, args: &Args<'_>) -> Value {
    match name {
        "NextUnsignedLong" => rng.next_unsigned_long().into(),
        "NextDouble" => rng.next_double().to_bits().into(),
        "NextInt" => rng.next_int(args.i32(0)).into(),
        "NextInt_default" => rng.next_int(i32::MAX).into(),
        "NextInt_range" => rng.next_int_range(args.i32(0), args.i32(1)).into(),
        "NextBool" => rng.next_bool().into(),
        "NextDouble_range" => rng
            .next_double_range(args.f64(0), args.f64(1))
            .to_bits()
            .into(),
        "NextFloat" => rng.next_float(1.0).to_bits().into(),
        "NextFloat_max" => rng.next_float(args.f32(0)).to_bits().into(),
        "NextFloat_range" => rng
            .next_float_range(args.f32(0), args.f32(1))
            .to_bits()
            .into(),
        "NextUnsignedInt" => rng.next_unsigned_int(args.u32(0)).into(),
        "NextUnsignedInt_default" => rng.next_unsigned_int(u32::MAX).into(),
        "NextUnsignedInt_range" => rng.next_unsigned_int_range(args.u32(0), args.u32(1)).into(),
        "NextUnsignedLong_max" => rng.next_unsigned_long_max(args.u64(0)).into(),
        "NextUnsignedLong_range" => rng
            .next_unsigned_long_range(args.u64(0), args.u64(1))
            .into(),
        "NextGaussianInt" => rng
            .next_gaussian_int(args.i32(0), args.i32(1), args.i32(2), args.i32(3))
            .into(),
        "NextGaussianDouble" => {
            next_gaussian_double(rng, args.f64(0), args.f64(1), args.f64(2), args.f64(3))
                .to_bits()
                .into()
        }
        "NextItem" | "NextItem_single" | "NextItem_empty" => rng
            .next_item(&args.i32_list(0))
            .copied()
            .unwrap_or_default()
            .into(),
        "WeightedNextItem" => {
            let weights = args.f32_list(0);
            let items: Vec<usize> = (0..weights.len()).collect();
            let picked = rng.weighted_next_item(&items, |&i| weights[i]);
            picked.map_or(0, |&i| i + 1).into()
        }
        _ => panic!("unknown series {name}"),
    }
}

#[test]
fn series_match() {
    let fixture: Fixture = load("rng.json");
    let mut checked = 0;
    for seed in &fixture.seeds {
        for series in &seed.series {
            let context = format!("seed {} {}{:?}", seed.seed, series.name, series.args);
            let args = Args(&series.args);
            let mut rng = Rng::new(seed.seed);
            match series.name.as_str() {
                "Rng.Shuffle" => {
                    let len = i32::try_from(args.u64(0)).expect("len");
                    let mut list: Vec<i32> = (0..len).collect();
                    rng.shuffle(&mut list);
                    let expected: Vec<Value> = list.into_iter().map(Value::from).collect();
                    assert_eq!(expected, series.out, "{context}");
                }
                name => {
                    for (i, expected) in series.out.iter().enumerate() {
                        assert_eq!(&call(&mut rng, name, &args), expected, "{context} #{i}");
                        if let Some(states) = &series.state_after {
                            states[i].assert_matches(&rng, &format!("{context} #{i}"));
                        }
                    }
                }
            }
            series.final_state.assert_matches(&rng, &context);
            checked += 1;
        }
    }
    assert!(checked > 0);
}

#[test]
fn argument_errors_match() {
    let fixture: Fixture = load("rng.json");
    for seed in &fixture.seeds {
        for case in &seed.errors {
            let mut rng = Rng::new(seed.seed);
            let result = catch_unwind(AssertUnwindSafe(|| match case.call.as_str() {
                "NextInt(0)" => drop(rng.next_int(0)),
                "NextInt(-1)" => drop(rng.next_int(-1)),
                "NextInt(5,5)" => drop(rng.next_int_range(5, 5)),
                "NextInt(6,5)" => drop(rng.next_int_range(6, 5)),
                "NextUnsignedInt(5,5)" => drop(rng.next_unsigned_int_range(5, 5)),
                "NextUnsignedLong(7,7)" => drop(rng.next_unsigned_long_range(7, 7)),
                "NextFloat(2,1)" => drop(rng.next_float_range(2.0, 1.0)),
                "NextDouble(2,1)" => drop(rng.next_double_range(2.0, 1.0)),
                other => unreachable!("unknown error case {other}"),
            }));
            let context = format!("seed {} {}", seed.seed, case.call);
            assert_eq!(case.result, "ArgumentOutOfRangeException", "{context}");
            assert!(result.is_err(), "{context} should panic");
            case.final_state.assert_matches(&rng, &context);
        }
    }
}

#[test]
fn weighted_static_matches() {
    let fixture: Fixture = load("rng.json");
    for case in &fixture.weighted_static {
        let weights: Vec<f32> = case.weights.iter().map(|&w| f32::from_bits(w)).collect();
        let items: Vec<usize> = (0..weights.len()).collect();
        for (&input, &expected) in case.inputs.iter().zip(&case.out) {
            let picked = weighted_pick(f32::from_bits(input), &items, |&i| weights[i]);
            let picked = picked.map_or(0, |&i| i32::try_from(i + 1).expect("small"));
            assert_eq!(
                picked,
                expected,
                "weights {weights:?} input {}",
                f32::from_bits(input)
            );
        }
    }
}
