#!/usr/bin/env bash
# Usage: [POLICY=random|greedy|shopper] [ASC=0..10] [CHARACTER=Necrobinder] [ACT1=overgrowth|underdocks] [SEED_PREFIX=RL2S]
#        [SEED_FIRST=0] [RUN_LOADOUT=<file>] tools/diff_run.sh <n_seeds>
# For seeds RL2S0..RL2S<n-1> (the prefix is SEED_PREFIX; final_check.sh rotates it by date; SEED_FIRST=k starts at
# RL2S<k>, so SEED_PREFIX=ROT261024S SEED_FIRST=193 tools/diff_run.sh 1 replays seed ROT261024S193 alone): play a
# seeded policy (random by default; greedy plays to survive,
# see crates/sts2-diff/src/greedy.rs) over a whole run on the Rust engine
# (Neow, map choices, combats, card rewards, rest sites, events, card selections; gold, relic and
# potion rewards and treasure relics are claimed like sts2-cli does) choosing only actions whose
# content is ported, until none is left (an unported encounter, Neow option, event, relic, card or
# room type),
# it dies, it wins (the last boss beaten: TheArchitect's room, "victory"), or it uses
# MAX_ACTIONS (stop=<reason> act=<act reached> ..., see `sts2-diff run-gen`); replay the same actions on the referee in run mode and compare
# the traces line by line on the modeled fields (including gold, potions, odds and every stream).
# Also counts the event rooms (ported events, not Neow) and shops entered, and the runs that
# reached each act's boss and beat it, and the acts reached, each also on matching seeds.
# EVENT=<ID> forces that event (the referee's `enter_event`) the first time the run waits on the map after Neow and any
# RUN_LOADOUT, takes its first-page option number <seed index> % <options> and stops (event_done) back on the map:
# `EVENT=ABYSSAL_BATHS tools/diff_run.sh 6` diffs every option without a natural run. A run loadout's "event" does the same;
# with the loadout's "continue": true the run goes on after the event instead of stopping (C24: TinkerTime's MadScience).
# ANCIENT=1 (or a run loadout's "ancient": true) takes option <seed index> % <options> of the first ancient after the
# loadout and stops (ancient_done) back on the map; with "act_index" that ancient is the act's first room.
# ASC (default 0) is the ascension both sides play at. CHARACTER (default Necrobinder; Ironclad, Silent, Defect,
# Regent) is the character both sides play (the referee's "character"). ACT1 forces Act 1 on both sides (the
# referee's "act1"); default: the run loadout's "act1", else overgrowth (the referee never rolls it). REAL_SHOP=1 plays both sides out of test mode (real shop potion
# price roll, Cauldron and CallingBell rewards); needs a referee build with referee/patcher.
# Each matching seed also leaves <seed>.hits.json (the entity ids it touched; `tools/hits.sh` aggregates them): run-gen
# writes <seed>.hits.pending.json, which becomes <seed>.hits.json only once the seed's verdict is a match, so content a run
# reached after a divergence, or in a run the referee could not check, is never counted as hit.
# RUN_LOADOUT is a run loadout (crates/sts2-diff/run_loadouts/, format in crates/sts2-diff/src/loadout.rs):
# cards, gold and relics (obtained through RelicCmd.Obtain, so AfterObtained runs on both sides) handed over
# once the run first waits on the map after Neow, through the referee's `run_loadout` command.
# PREFER=ID,TAG,.. (random policy; --prefer-relics): buy those relics, use Lift/Dig/Hatch, and take event options with those callback tags.
# REFEREE_FRESH=1 starts a referee process per seed instead of one for all (referee/client.sh).
# DIFF_NO_BUILD=1 skips the cargo build (tools/regress_runs.sh builds once); the binary is target/regress/sts2-diff (the `regress` profile, Cargo.toml).
# A diverging seed is saved as a run fixture candidate, ${FIXTURE_DIR:-target/fixture-candidates}/<seed>-<character>-<act1>-<policy>-a<ASC>.json,
# and the command that pins it (copies it into crates/sts2-diff/run_fixtures/) is printed after the DIVERGENCE line.
# The shopper policy plays like greedy but shops: at each step in a shop it leaves with chance 1 in 4,
# else buys a random affordable card, relic or potion (when a slot is free) or removes a card; the
# summary counts those purchases, all and on matching seeds.
# A matching seed's two traces are deleted (actions, hits and stops stay); DIFF_KEEP=1 keeps them.
# A stop is not a failure; only a divergence is. Work files stay in
# ${DIFF_OUT:-target/diff}/run.
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
n=${1:?usage: tools/diff_run.sh <n_seeds>}
max_actions=${MAX_ACTIONS:-3000}
policy=${POLICY:-random}
ascension=${ASC:-0}
prefix=${SEED_PREFIX:-RL2S}
first=${SEED_FIRST:-0}
character=${CHARACTER:-Necrobinder}
act_args=()
[ -z "${ACT1:-}" ] || act_args=(--act1 "$ACT1")
# REAL_SHOP=1: the real-game branches of the TestMode gameplay sites (shop potion prices, Cauldron), see
# docs/specs/test-mode.md: the engine runs out of test mode and the referee gets "real_shop_prices"/"real_cauldron"/"real_calling_bell".
real_args=()
[ -z "${REAL_SHOP:-}" ] || real_args=(--real-game 1)
out=${DIFF_OUT:-$root/target/diff}/run
mkdir -p "$out"
[ -n "${DIFF_NO_BUILD:-}" ] || cargo build -q --manifest-path "$root/Cargo.toml" --profile regress -p sts2-diff
bin=$root/target/regress/sts2-diff
. "$root/referee/client.sh"

matching=0
diverging=0
events_entered=0
events_matched=0
shops_entered=0
shops_matched=0
bought=(0 0 0 0)
bought_matched=(0 0 0 0)
# Per act (index 0..2): boss reached / beaten, all and on matching seeds; runs that reached the act.
boss_reached=(0 0 0)
boss_reached_matched=(0 0 0)
boss_beaten=(0 0 0)
boss_beaten_matched=(0 0 0)
acts=(0 0 0)
acts_matched=(0 0 0)
floors=()
acts_seen=()
: > "$out/stops.txt"
for ((i = first; i < first + n; i++)); do
    seed=$prefix$i
    rm -f "$out/$seed.hits.json" "$out/$seed.hits.pending.json"
    stop=$("$bin" run-gen --seed "$seed" --policy-seed "$i" --max-actions "$max_actions" --policy "$policy" --ascension "$ascension" --character "$character" "${act_args[@]}" "${real_args[@]}" ${RUN_LOADOUT:+--run-loadout "$RUN_LOADOUT"} ${PREFER:+--prefer-relics "$PREFER"} ${EVENT:+--event "$EVENT"} ${ANCIENT:+--ancient 1} \
        --hits "$out/$seed.hits.pending.json" --actions "$out/$seed.actions.json" --trace "$out/$seed.rust.jsonl")
    # A step the referee rejects (an action the engine allowed) leaves its trace up to there: compare that,
    # and count the seed as diverging even if the prefix agrees. Only a dead referee process aborts.
    referee_status=0
    referee_trace "$seed" "$out/$seed.actions.json" "$out/$seed.referee.jsonl" || referee_status=$?
    [ "$referee_status" -le 1 ] || exit "$referee_status"
    status=0
    result=$("$bin" compare-run --rust "$out/$seed.rust.jsonl" --referee "$out/$seed.referee.jsonl") || status=$?
    if [ "$referee_status" -eq 1 ] && [ "$status" -eq 0 ]; then
        status=1
        result="DIVERGENCE: the referee rejected a step the engine played ($result)"
    fi
    echo "$seed: $result; $stop"
    # `stop=<reason> act=<n> floor=<n> events=<n> shops=<n> bought=<c>,<r>,<p>,<x> boss=<a1>,<a2>,<a3>,<second>` (each reached|beaten|-; the 4th, the ascension 10 second boss of Act 3, is only in the per-seed line)
    IFS=, read -r -a bosses <<< "${stop##*boss=}"
    rest=${stop% boss=*}
    IFS=, read -r -a buys <<< "${rest##*bought=}"
    rest=${rest% bought=*}
    shops=${rest##*shops=}
    rest=${rest% shops=*}
    events=${rest##*events=}
    rest=${rest% events=*}
    floors+=("${rest##*floor=}")
    rest=${rest% floor=*}
    act=${rest##*act=}
    acts_seen+=("$act")
    reason=${rest#stop=}
    echo "${reason% act=*}" >> "$out/stops.txt"
    events_entered=$((events_entered + events))
    shops_entered=$((shops_entered + shops))
    reached=(0 0 0) beaten=(0 0 0) in_act=(0 0 0)
    for k in 0 1 2; do
        case ${bosses[k]:--} in
            reached) reached[k]=1 ;;
            beaten) reached[k]=1 beaten[k]=1 ;;
        esac
        [ "$act" -le "$k" ] || in_act[k]=1
        boss_reached[k]=$((boss_reached[k] + reached[k]))
        boss_beaten[k]=$((boss_beaten[k] + beaten[k]))
        acts[k]=$((acts[k] + in_act[k]))
    done
    for k in 0 1 2 3; do
        bought[k]=$((bought[k] + buys[k]))
        [ "$status" -ne 0 ] || bought_matched[k]=$((bought_matched[k] + buys[k]))
    done
    if [ "$status" -eq 0 ]; then
        mv "$out/$seed.hits.pending.json" "$out/$seed.hits.json"
    else
        rm -f "$out/$seed.hits.pending.json"
    fi
    case $status in
        0) [ -n "${DIFF_KEEP:-}" ] || rm -f "$out/$seed.rust.jsonl" "$out/$seed.referee.jsonl"
           matching=$((matching + 1)); events_matched=$((events_matched + events)); shops_matched=$((shops_matched + shops))
           for k in 0 1 2; do
               boss_reached_matched[k]=$((boss_reached_matched[k] + reached[k])); boss_beaten_matched[k]=$((boss_beaten_matched[k] + beaten[k]))
               acts_matched[k]=$((acts_matched[k] + in_act[k]))
           done ;;
        1) diverging=$((diverging + 1))
           candidate=${FIXTURE_DIR:-$root/target/fixture-candidates}/$seed-$character-${ACT1:-act1}-$policy-a$ascension.json
           mkdir -p "$(dirname "$candidate")"
           "$bin" run-fixture --seed "$seed" --actions "$out/$seed.actions.json" --referee "$out/$seed.referee.jsonl" \
               --out "$candidate" > /dev/null &&
               echo "  pin: cp ${candidate#"$root"/} crates/sts2-diff/run_fixtures/<name>.json" ;;
        *) echo "$seed: compare failed" >&2; exit "$status" ;;
    esac
done
median=$(printf '%s\n' "${floors[@]}" | sort -n | awk '{a[NR]=$1} END {print (NR % 2) ? a[(NR+1)/2] : (a[NR/2] + a[NR/2+1]) / 2}')
max=$(printf '%s\n' "${floors[@]}" | sort -n | tail -1)
echo "policy: $policy, ascension: $ascension, character: $character, act1: ${ACT1:-default}"
echo "run seeds: $n, matching: $matching, diverging: $diverging, median floor reached: $median, max floor: $max, max act: $(printf '%s\n' "${acts_seen[@]}" | sort -n | tail -1)"
echo "event rooms entered: $events_entered, on matching seeds: $events_matched"
echo "shops entered: $shops_entered, on matching seeds: $shops_matched"
echo "shop buys (cards, relics, potions, removals): ${bought[*]}, on matching seeds: ${bought_matched[*]}"
echo "boss reached: ${boss_reached[0]} (matching ${boss_reached_matched[0]}), beaten and act ended: ${boss_beaten[0]} (matching ${boss_beaten_matched[0]})"
echo "acts reached (1, 2, 3): ${acts[*]}, on matching seeds: ${acts_matched[*]}"
for k in 1 2; do
    echo "act $((k + 1)) boss reached: ${boss_reached[k]} (matching ${boss_reached_matched[k]}), beaten: ${boss_beaten[k]} (matching ${boss_beaten_matched[k]})"
done
echo "stop reasons:"
# A Neow stop lists all three unported options; group those as one reason.
sed -E 's/^neow .*/neow (no ported option)/' "$out/stops.txt" | sort | uniq -c | sort -rn
[ "$diverging" -eq 0 ]
