# Sourced by tools/final_check.sh and tools/final_matrix.sh: the steps of the "done" check that both share.
cachedir() { n="$HOME/.cache/spirezero-$1" o="$HOME/.cache/spirebot-$1"; if [ ! -e "$n" ] && [ -d "$o" ]; then echo "$o"; else echo "$n"; fi; }  # old name while only it exists
# The caller sets FINAL_DIR (the frozen checkout), FINAL_LOGS (where logs go) and optionally FINAL_LOG_PREFIX
# (prepended to every log name), FINAL_LABEL_PREFIX (prepended to every result line), FINAL_SLOT (a command that
# wraps each diff_run.sh pass, e.g. the checkout's tools/slot.sh to share the machine-wide referee slots).

# The acts of the matrix axis: Act 1 (overgrowth, underdocks), then Hive (Act 2) and Glory (Act 3). Every act has a coverage line;
# FINAL_RUN_ACTS are the ones with run passes (diff_run.sh). Act 1 passes are natural runs (they go on into Hive and Glory since the engine
# plays them, XC9); Hive and Glory passes start in their act through the act jump with the character's run loadout
# crates/sts2-diff/act_loadouts/<act>-<character>.json (an end-of-previous-act deck, HP, gold and relics: the nearest community run
# to the medians, built by tools/act_loadouts.py).
FINAL_ACTS="overgrowth underdocks hive glory"
FINAL_RUN_ACTS="overgrowth underdocks hive glory"

# Seed prefix of the rotating pass: it changes daily, so repeated checks do not replay the same seeds.
final_rotating() { echo "ROT$(date +%y%m%d)S"; }

# final_freeze <repo root> <dir> <ref>: a clean detached worktree of <ref> at <dir>; non-zero if it cannot be made (the
# caller must stop: an old directory left at <dir> would be verified instead).
final_freeze() {
    git -C "$1" worktree remove --force "$2" 2>/dev/null
    git -C "$1" worktree add -q --detach "$2" "$3"
}

# final_build: the engine binary every diff_run.sh pass runs (target/regress/sts2-diff), built once.
final_build() {
    cargo build -q --manifest-path "$FINAL_DIR/Cargo.toml" --profile regress -p sts2-diff
}

# final_gate <log>: the gate's fmt/clippy/test lines on one line; returns the gate's exit status.
final_gate() {
    local rc=0
    (cd "$FINAL_DIR" && cargo xtask gate) > "$1" 2>&1 || rc=$?
    grep -E '^(fmt|clippy|test):' "$1" | tr '\n' ' '
    echo
    return $rc
}

# final_coverage <character> <act> <log>: the `total` line of `cargo xtask coverage`
# (kind, ported, in scope, missing, excluded); returns coverage's exit status (non-zero if anything is missing).
final_coverage() {
    local rc=0
    (cd "$FINAL_DIR" && cargo xtask coverage --character "$1" --act "$2") > "$3" 2>&1 || rc=$?
    grep '^total' "$3" || echo "total: no result line: $(tail -1 "$3")"
    return $rc
}

# final_pass <n> <character> <act> <policy> <ascension> <seed prefix> [real]: one tools/diff_run.sh pass (a prefix
# `cover:<name>` is a Hive or Glory cover pass: seeds RL2S, the run loadout crates/sts2-diff/act_loadouts/cover/<name>.json,
# at most FINAL_COVER_SEEDS (default 6) seeds, see tools/act_cover.py; real: out of
# test mode, REAL_SHOP=1) with its own log and work dir (its hits files are kept in target/final-hits/<pass name>/, or
# target/final-hits-hive/<pass name>/ for a Hive act-jump pass, final-hits-glory for Glory); prints its result line, the runs that
# reached Act 2 (`act2`), the Hive boss (`hive_boss`) and beat it (`hive_beaten`), the Glory boss (`glory_boss`), the A10 second boss
# (`boss2`) and the victories (`victory`), and the first divergences. A passing
# pass deletes its work dir; a diverging one leaves it, and its fixture candidates go to
# target/fixture-candidates/<pass name>/ of the frozen checkout. Only matching seeds leave hits files (tools/diff_run.sh).
# Returns 1 if the pass failed (diff_run.sh's exit status, or no result line), 0 if it matched or its act is skipped;
# final_matrix.sh reads the logs instead (tools/final_table.py).
final_pass() {
    local n=$1 character=$2 act=$3 policy=$4 asc=$5 prefix=$6 real=${7:-}
    local name=$character-$act-$policy-a$asc-$prefix label="A$asc $policy $prefix" cover=
    case $prefix in
        cover:*)
            cover=${prefix#cover:}
            prefix=RL2S
            name=$character-$act-$policy-a$asc-cover-$cover
            label="A$asc $policy cover $cover"
            # A forced event or ancient takes option <seed> % <options>: it needs the full seed count.
            case $cover in *-event-* | *-ancient) ;; *) [ "$n" -le "${FINAL_COVER_SEEDS:-6}" ] || n=${FINAL_COVER_SEEDS:-6} ;; esac
            ;;
    esac
    if [ -n "$real" ]; then
        name=$character-$act-$policy-realshop
        label="A0 $policy real-shop-prices"
    fi
    local log=$FINAL_LOGS/${FINAL_LOG_PREFIX:-}$name.log out=$FINAL_DIR/target/final-work/$name hits=final-hits
    local act_env=(ACT1="$act")
    rm -rf "$out"
    case " $FINAL_RUN_ACTS " in
        *" $act "*) ;;
        *)
            echo "skipped: no $act runs yet" > "$log"
            echo "${FINAL_LABEL_PREFIX:-}$label: skipped, $act runs are not supported yet"
            return 0
            ;;
    esac
    if [ "$act" = hive ] || [ "$act" = glory ]; then
        act_env=(RUN_LOADOUT="$FINAL_DIR/crates/sts2-diff/act_loadouts/${cover:+cover/}${cover:-$act-${character,,}}.json")
        hits=final-hits-$act
    fi
    env CHARACTER="$character" "${act_env[@]}" POLICY="$policy" ASC="$asc" SEED_PREFIX="$prefix" ${real:+REAL_SHOP=1} \
        DIFF_OUT="$out" DIFF_NO_BUILD=1 FIXTURE_DIR="$FINAL_DIR/target/fixture-candidates/$name" \
        ${FINAL_SLOT:-} "$FINAL_DIR/tools/diff_run.sh" "$n" > "$log" 2>&1
    local status=$?
    echo "${FINAL_LABEL_PREFIX:-}$label: $(grep 'run seeds:' "$log" || echo "no result line: $(tail -1 "$log")"); act2 $(grep -cE '; stop=.* act=[2-9] floor=' "$log"), hive_boss $(grep -cE '; stop=.* boss=[^,]*,(reached|beaten)' "$log"), hive_beaten $(grep -cE '; stop=.* boss=[^,]*,beaten' "$log"), glory_boss $(grep -cE '; stop=.* boss=[^,]*,[^,]*,(reached|beaten)' "$log"), boss2 $(grep -cE '; stop=.* boss=[^,]*,[^,]*,[^,]*,(reached|beaten)' "$log"), victory $(grep -c '; stop=victory ' "$log")"
    grep -E 'DIVERGENCE|pin:' "$log" | head -5
    # The hits files (tools/hits.sh) outlive the work dir.
    mkdir -p "$FINAL_DIR/target/$hits/$name"
    mv "$out"/run/*.hits.json "$FINAL_DIR/target/$hits/$name/" 2>/dev/null
    if grep -q 'run seeds:' "$log" && ! grep -qE 'diverging: [1-9]' "$log"; then rm -rf "$out"; fi
    [ "$status" -eq 0 ] && grep -q 'run seeds:' "$log"
}

# final_save_candidates <repo root>: the frozen checkout's divergences as fixture candidates under <repo root>/target.
final_save_candidates() {
    [ -d "$FINAL_DIR/target/fixture-candidates" ] && [ "$FINAL_DIR" != "$1" ] || return 0
    mkdir -p "$1/target/fixture-candidates"
    cp -r "$FINAL_DIR"/target/fixture-candidates/. "$1/target/fixture-candidates/"
}

# final_hits <log> [hive|glory]: the zero-hit check (`cargo xtask hits --check docs/hits-allowlist.md`, see tools/hits_sweep.sh) over the
# hit files of the regress runs (target/regress/work) and of every Act 1 final_pass (target/final-hits); entities only later
# acts can reach are advisory (`--acts overgrowth,underdocks`). With `hive`: the same check (`--acts hive`) over the Hive act-jump passes
# (target/final-hits-hive: the median loadout and the cover loadouts of tools/act_cover.py) and regress's forced fights (target/regress/work: every
# ported encounter, whatever its act, and the combat loadouts), so an entity Hive lists must be hit by a matching run of one of them; Act 1 and
# Glory-only entities are advisory. `glory` is the same over target/final-hits-glory (`--acts glory`). Prints the per-kind hit counts, or the entities with no hit, on one line. Returns its exit status (non-zero
# if a ported entity has no hit and is not listed).
final_hits() {
    local rc=0 acts=overgrowth,underdocks dirs=(target/regress/work target/final-hits)
    case ${2:-} in
        hive | glory)
            acts=$2
            dirs=(target/final-hits-$2 target/regress/work)
            ;;
    esac
    (cd "$FINAL_DIR" && tools/hits.sh --min 3 --summary --effects --check docs/hits-allowlist.md --acts "$acts" "${dirs[@]}") > "$1" 2>&1 || rc=$?
    if [ $rc -eq 0 ]; then
        grep -E '^(card|relic|potion|power|monster|encounter|event|enchantment|affliction): [0-9]+/' "$1" | tr '\n' ';'
    else
        grep -A4 -E '[1-9][0-9]* with 0 hits' "$1" | head -8 | tr '\n' ';'
    fi
    echo
    return $rc
}

# final_history <log>: the community-run replay (docs/history-replay.md): 300 runs per character x Act 1 from the local DB
# (HISTORY_DB, default ~/.cache/spirezero-data/sts2fun/sts2_public.db); prints its `history-replay:` line, or says it was
# skipped without the DB. Returns tools/replay_history.py's exit status (non-zero for engine mismatches not explained by
# an unported relic or map).
final_history() {
    local db=${HISTORY_DB:-$(cachedir data)/sts2fun/sts2_public.db} rc=0
    if [ ! -f "$db" ]; then
        echo "history-replay: skipped (no $db)"
        return 0
    fi
    (cd "$FINAL_DIR" && python3 tools/replay_history.py --db "$db" --sample 300) > "$1" 2>&1 || rc=$?
    grep -E '^history-replay:' "$1" || echo "history-replay: no result line: $(tail -1 "$1")"
    return $rc
}
