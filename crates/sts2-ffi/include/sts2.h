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

#define STS2_ABI_VERSION 3

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
} Sts2RunStepStats;

typedef struct Sts2RunLayout {
    uint64_t obs_bytes;
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
} Sts2RunLayout;

int32_t sts2_run_layout(Sts2RunLayout *out);
Sts2RunEnvs *sts2_run_envs_new(size_t n, const Sts2RunConfig *config, size_t threads);
void sts2_run_envs_free(Sts2RunEnvs *envs);
/* Returns the number of episodes that ended (their envs were reset), or STS2_ERR_*. */
int32_t sts2_run_envs_step(Sts2RunEnvs *envs, const uint16_t *actions, size_t n);
int32_t sts2_run_envs_reset_all(Sts2RunEnvs *envs);
int32_t sts2_run_envs_stats(const Sts2RunEnvs *envs, Sts2RunStepStats *out);
size_t sts2_run_envs_len(const Sts2RunEnvs *envs);
const uint8_t *sts2_run_envs_obs_ptr(const Sts2RunEnvs *envs);
const uint8_t *sts2_run_envs_final_obs_ptr(const Sts2RunEnvs *envs);
const uint64_t *sts2_run_envs_mask_ptr(const Sts2RunEnvs *envs);
const uint8_t *sts2_run_envs_done_ptr(const Sts2RunEnvs *envs);
const uint8_t *sts2_run_envs_won_ptr(const Sts2RunEnvs *envs);
/* 0 none, 1 died, 2 victory, 3 unported content, 4 stuck, 5 capped */
const uint8_t *sts2_run_envs_end_ptr(const Sts2RunEnvs *envs);
const uint16_t *sts2_run_envs_floor_ptr(const Sts2RunEnvs *envs);

#ifdef __cplusplus
}
#endif

#endif
