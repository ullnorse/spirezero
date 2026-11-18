/* The C ABI of sts2-ffi (crates/sts2-ffi/src/lib.rs has the full contract).
 *
 * Buffers are engine-owned: they keep their address from sts2_envs_new to sts2_envs_free and
 * are written only during a call on the batch. Errors are return codes (negative STS2_ERR_*)
 * or null pointers; sts2_last_error() has the calling thread's last message. One call at a
 * time per batch. */
#ifndef STS2_H
#define STS2_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define STS2_ABI_VERSION 11

#define STS2_OK 0
#define STS2_ERR_NULL (-1)
#define STS2_ERR_LENGTH (-2)
#define STS2_ERR_ILLEGAL (-3)
#define STS2_ERR_ARGUMENT (-4)
#define STS2_ERR_PANIC (-5)
#define STS2_ERR_POISONED (-6)

typedef struct Sts2Envs Sts2Envs;

typedef struct Sts2StepStats {
    uint64_t actions;
    uint64_t unmapped;
    uint32_t ended;
    uint32_t won;
    uint32_t capped;
    uint32_t reset;
} Sts2StepStats;

typedef struct Sts2Layout {
    uint64_t obs_bytes;
    uint64_t static_bytes;
    uint64_t mask_words;
    uint64_t actions;
    uint64_t creatures_at;
    uint64_t creature_bytes;
    uint64_t cards_at;
    uint64_t card_bytes;
    uint64_t hand_at;
    uint64_t intents_at;
    uint64_t intent_bytes;
    uint64_t choice_at;
} Sts2Layout;

uint32_t sts2_abi_version(void);
/* 16 hex digits: the hash of the sources the library was built from (static). */
const char *sts2_build_id(void);
uint64_t sts2_vocab_hash(void);
const char *sts2_last_error(void);
int32_t sts2_layout(Sts2Layout *out);

/* start_source_path: a JSON-lines start corpus, or NULL for the RL bench's W1-all starts. */
Sts2Envs *sts2_envs_new(size_t n, const char *start_source_path, uint64_t seed, size_t threads);
Sts2Envs *sts2_envs_new_from_corpus(size_t n, const uint8_t *corpus, size_t corpus_len,
                                    uint64_t seed, size_t threads);
void sts2_envs_free(Sts2Envs *envs);

/* Returns the number of episodes that ended (their envs were reset), or STS2_ERR_*. */
int32_t sts2_envs_step(Sts2Envs *envs, const uint16_t *actions, size_t n);
int32_t sts2_envs_reset_all(Sts2Envs *envs);
int32_t sts2_envs_stats(const Sts2Envs *envs, Sts2StepStats *out);

size_t sts2_envs_len(const Sts2Envs *envs);
size_t sts2_envs_obs_bytes(const Sts2Envs *envs);
const uint8_t *sts2_envs_obs_ptr(const Sts2Envs *envs);
const uint8_t *sts2_envs_static_ptr(const Sts2Envs *envs);
const uint64_t *sts2_envs_mask_ptr(const Sts2Envs *envs);
const uint8_t *sts2_envs_done_ptr(const Sts2Envs *envs);
const uint8_t *sts2_envs_won_ptr(const Sts2Envs *envs);
const int16_t *sts2_envs_final_hp_ptr(const Sts2Envs *envs);
int32_t sts2_envs_set_blindfold(Sts2Envs *envs, uint8_t on, uint64_t seed);

/* The run env (stage 9): whole runs as episodes over the run space (the 140 combat indices,
 * then the run-level ones from run_base). */
typedef struct Sts2RunEnvs Sts2RunEnvs;

typedef struct Sts2RunConfig {
    uint32_t seed_kind; /* 0: the seeds_len seeds at seeds; 1: start..end; 2: random from master */
    uint32_t cycle;     /* list or range: 1 to start over at its end */
    const uint64_t *seeds;
    size_t seeds_len;
    uint64_t start;
    uint64_t end;
    uint64_t master;
    int32_t character;  /* index in select order (Ironclad, Silent, Defect, Regent, Necrobinder), or -1 for seed % 5 */
    uint32_t ascension; /* 0 to 10 */
    uint32_t combat;    /* 0 the agent plays fights, 1 greedy resolver, 2 random resolver */
    uint32_t intents;   /* 1 to write the combat section's intents */
    /* The end-to-end options (sts2_rl::RunOptions): */
    uint32_t dedupe;       /* 1 to merge the mask into action classes (one index per class) */
    uint32_t turn_cap;     /* player turns per fight before the run ends capped (6); 0 none */
    uint32_t decision_cap; /* agent decisions per run before it ends capped (7); 0 none */
    uint32_t events;       /* 1 to write the events buffer */
    uint32_t counters;     /* 1 to count saturations and unmapped decisions */
    uint32_t priv_obs;     /* 1 to write the privileged buffer (value head only) */
    uint32_t obs_v2;       /* 1 to write the run observation's v2 section (ABI 8) */
} Sts2RunConfig;

typedef struct Sts2RunStepStats {
    uint64_t actions;   /* engine actions: the agent's, automatic single options, resolved */
    uint64_t decisions;
    uint64_t auto_;
    uint64_t resolved;
    uint64_t refused;   /* listed actions left out of the masks: the engine would refuse them */
    uint64_t unmapped;
    uint32_t ended;
    uint32_t won;
    uint32_t capped;
    uint32_t stuck;
    uint32_t reset;
    uint32_t fight_capped;
    uint32_t decision_capped;
    uint64_t merged;           /* dedupe: indices merged into an earlier member's class */
    uint64_t unmapped_classes; /* dedupe: classes no member of which has an index */
    uint64_t auto_class;       /* dedupe: decisions taken because they had one class */
    uint64_t unmapped_decisions; /* counters: decisions with an action without an index */
    uint64_t saturated[8];     /* counters: any, cards, deck, creatures, powers, choice, relics, map */
} Sts2RunStepStats;

typedef struct Sts2RunEvents {
    uint8_t floors;    /* floors entered in the step */
    uint8_t bosses;    /* bosses killed in the step */
    uint8_t boss_acts; /* bit a: a boss of act index a was killed */
    uint8_t won;       /* 1: the run was won */
} Sts2RunEvents;

typedef struct Sts2RunLayout {
    uint64_t obs_bytes;
    uint64_t priv_bytes;
    uint64_t mask_words;
    uint64_t actions;
    uint64_t run_base;
    uint64_t deck_at;
    uint64_t relics_at;
    uint64_t belt_at;
    uint64_t map_at;
    uint64_t ancient_at;
    uint64_t event_at;
    uint64_t rewards_at;
    uint64_t select_at;
    uint64_t bundles_at;
    uint64_t rest_at;
    uint64_t treasure_at;
    uint64_t shop_at;
    uint64_t combat_at;
    uint64_t combat_static_at;
    uint64_t v2_at;        /* the v2 section (ABI 8), to obs_bytes */
} Sts2RunLayout;

int32_t sts2_run_layout(Sts2RunLayout *out);
Sts2RunEnvs *sts2_run_envs_new(size_t n, const Sts2RunConfig *config, size_t threads);
void sts2_run_envs_free(Sts2RunEnvs *envs);
/* Returns the number of episodes that ended (their envs were reset), or STS2_ERR_*.
   An action of STS2_RUN_HOLD leaves its env as it is for the step. */
#define STS2_RUN_HOLD 0xFFFF
int32_t sts2_run_envs_step(Sts2RunEnvs *envs, const uint16_t *actions, size_t n);
int32_t sts2_run_envs_reset_all(Sts2RunEnvs *envs);
/* Env i starts a fresh run of seeds[i]; later runs take the source's episodes from `next`. */
int32_t sts2_run_envs_restart(Sts2RunEnvs *envs, const uint64_t *seeds, size_t n, uint64_t next);
int32_t sts2_run_envs_stats(const Sts2RunEnvs *envs, Sts2RunStepStats *out);
size_t sts2_run_envs_len(const Sts2RunEnvs *envs);
const uint8_t *sts2_run_envs_obs_ptr(const Sts2RunEnvs *envs);
const uint8_t *sts2_run_envs_final_obs_ptr(const Sts2RunEnvs *envs);
/* len x priv_bytes; zero unless priv_obs. For the value head only. */
const uint8_t *sts2_run_envs_priv_ptr(const Sts2RunEnvs *envs);
const uint64_t *sts2_run_envs_mask_ptr(const Sts2RunEnvs *envs);
const uint8_t *sts2_run_envs_done_ptr(const Sts2RunEnvs *envs);
const uint8_t *sts2_run_envs_won_ptr(const Sts2RunEnvs *envs);
/* 0 none, 1 died, 2 victory, 3 unported content, 4 stuck, 5 capped (engine actions),
 * 6 fight capped (turn_cap), 7 decision capped (decision_cap), 8 practice end (a practice
 * episode's first decision outside combat: a truncation) */
const uint8_t *sts2_run_envs_end_ptr(const Sts2RunEnvs *envs);
const uint16_t *sts2_run_envs_floor_ptr(const Sts2RunEnvs *envs);
/* dedupe: len x actions member counts of each kept index's class (0 elsewhere) */
const uint8_t *sts2_run_envs_class_counts_ptr(const Sts2RunEnvs *envs);
/* events: len x Sts2RunEvents for the last step */
const Sts2RunEvents *sts2_run_envs_events_ptr(const Sts2RunEnvs *envs);
/* Practice starts (ABI 7, sts2_rl::practice). Cells by index: ELITE_ACT1-3, BOSS_ACT1-3. */
typedef struct Sts2PracticeConfig {
    uint64_t slots;      /* envs 0..slots start practice when their episode ends */
    uint32_t weights[6]; /* relative weight of each cell for one start */
    uint32_t pool;       /* snapshots kept per cell (the most recent) */
    uint32_t max_uses;   /* starts per snapshot (0: no limit) */
    uint32_t min_active; /* entries a cell needs to be drawn from */
    uint32_t capture;    /* 1: normal runs capture snapshots */
    uint64_t seed;       /* the draws' and reseeds' seed (read when first set) */
} Sts2PracticeConfig;

typedef struct Sts2PracticeStats {
    uint64_t pool[6], captured[6], evicted[6], used_up[6], wanted[6], started[6],
        fell_back[6], to_run[6], bad_start[6];
} Sts2PracticeStats;

/* NULL config: off (the pool stays). */
int32_t sts2_run_envs_set_practice(Sts2RunEnvs *envs, const Sts2PracticeConfig *config);
int32_t sts2_run_envs_practice_stats(const Sts2RunEnvs *envs, Sts2PracticeStats *out);
/* the ended episode's mask and privileged buffer (as final_obs) */
const uint64_t *sts2_run_envs_final_mask_ptr(const Sts2RunEnvs *envs);
const uint8_t *sts2_run_envs_final_priv_ptr(const Sts2RunEnvs *envs);
/* per env: 0 a normal run, else practice cell index + 1; of the current / just-ended episode */
const uint8_t *sts2_run_envs_cell_ptr(const Sts2RunEnvs *envs);
const uint8_t *sts2_run_envs_ended_cell_ptr(const Sts2RunEnvs *envs);
/* actions x 4 bytes: entity kind, slot, sub-slot, phase (255 any) per run-space index */
const uint8_t *sts2_run_action_table(void);

/* ABI 11 (RLN29): the content stats, `sts2_rl::content_stats` (0 cards, 1 cards upgraded,
 * 2 relics, 3 potions): rows x width int16 by vocab id; the columns as "name:m|f,...". */
const int16_t *sts2_content_stats(uint32_t table, size_t *rows);
const char *sts2_content_stats_columns(void);

#ifdef __cplusplus
}
#endif

#endif
