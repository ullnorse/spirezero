#!/usr/bin/env python3
"""Builds godot/build/audio: the game's sounds for the browser frontend's Web Audio player
(web/shell.html, src/audio.gd), from the owner's install. Nothing this writes is committed.

The game plays FMOD Studio events by path (`event:/sfx/block_gain`). Their samples sit in the
pack's FMOD banks, but an event is more than a sample: random playlists, layered instruments,
random pitch and volume, bus mixing and effects. So each event is rendered offline by the
install's own FMOD runtime (libfmodstudio, output WAVWRITER_NRT), a few takes for the random
ones, and encoded to ogg/opus. Music and ambience are rendered the same way with the global
`Progress` parameter set as the game sets it per room (NRunMusicController), as loops.

Also maps each event to its waveforms by walking the bank's object graph (event -> instruments
-> playlists -> waveform instruments -> FSB5 subsounds), for the manifest and the report.

Writes build/audio/<key>.ogg (and <key>.m4a, AAC, for iOS Safari) and build/audio/audio.json:
  {"events": {path: {"takes": [file...], "loop": bool, "samples": [fsb name...]}},
   "music": {name: {"file": ..., "loop": [start_s, end_s]}}}

Usage: godot/audio_import.py [--pck PATH] [--game DIR] [--only REGEX] [--takes N]
Needs ffmpeg (~/tools/bin/ffmpeg or PATH).
"""
import argparse
import array
import ctypes as C
import importlib.util
import json
import os
import re
import shutil
import struct
import subprocess
import sys
import time
import wave


def cache_dir(name):
    """`~/.cache/spirezero-<name>`, or the old `~/.cache/spirebot-<name>` while only that exists."""
    new, old = os.path.expanduser(f"~/.cache/spirezero-{name}"), os.path.expanduser(f"~/.cache/spirebot-{name}")
    return old if not os.path.exists(new) and os.path.isdir(old) else new

HERE = os.path.dirname(os.path.abspath(__file__))
CACHE = cache_dir("data")
PCKS = [os.path.expanduser("~/.local/share/Steam/steamapps/common/Slay the Spire 2/SlayTheSpire2.pck"), CACHE + "/SlayTheSpire2.pck"]
GAME = os.path.expanduser("~/.local/share/Steam/steamapps/common/Slay the Spire 2")
FFMPEG = next((p for p in (os.path.expanduser("~/tools/bin/ffmpeg"), shutil.which("ffmpeg")) if p and os.path.isfile(p)), "ffmpeg")
RATE = 48000
BLOCK = 1024
# NRunMusicController.MusicProgressTrack: the global `Progress` the act's track and ambience
# follow, set per room (GetTrack) and by a few fights (Elite2, CombatEnd).
TRACK_STATES = {"init": 0, "enemy": 1, "merchant": 2, "rest": 3, "unknown": 4, "treasure": 5, "elite": 6,
                "after": 7, "elite2": 8, "merchant_end": 9}
ENEMY, COMBAT_END = TRACK_STATES["enemy"], TRACK_STATES["after"]
# The acts' tracks (ActModel.BgMusicOptions) and ambience (AmbientSfx) by key.
ACT_TRACKS = {"act1_a1": "event:/music/act1_a1_v1", "act1_a2": "event:/music/act1_a2_v2", "act1_b1": "event:/music/act1_b1_v1",
              "act2_a1": "event:/music/act2_a1_v2", "act2_a2": "event:/music/act2_a2_v2",
              "act3_a1": "event:/music/act3_a1_v1", "act3_a2": "event:/music/act3_a2_v1"}
ACT_AMBIENCE = {"act1_ambience": "event:/sfx/ambience/act1_ambience", "act2_ambience": "event:/sfx/ambience/act2_ambience",
                "act3_ambience": "event:/sfx/ambience/act3_ambience"}
# Bosses' own tracks (EncounterModel.CustomBgm) and the per-fight parameter their monsters set
# (UpdateMusicParameter): the values they hold the track at, and the one that ends it (5, the
# Insatiable's 10). The Queen's track is also Aeonglass's.
BOSSES = {
    "vantom": ("event:/music/act1_boss_vantom", "vantom_progress", (0, 1, 2, 3), 5),
    "the_kin": ("event:/music/act1_boss_the_kin", "the_kin_progress", (0, 1), 5),
    "ceremonial_beast": ("event:/music/act1_boss_ceremonial_beast", "ceremonial_beast_progress", (0, 1), 5),
    "soul_fysh": ("event:/music/act1_b_boss_soul_fysh", "soulfysh_progress", (0, 1, 2), 5),
    "waterfall_giant": ("event:/music/act1_b_boss_waterfall_giant", "waterfall_giant_progress", (0, 1, 2), 5),
    "the_insatiable": ("event:/music/act2_boss_the_insatiable", "insatiable_progress", (0,), 10),
    "knowledge_demon": ("event:/music/act2_boss_knowledge_demon", "knowledge_demon_progress", (0,), 5),
    "kaiser_crab": ("event:/music/act2_boss_kaiser_crab", "kaiser_crab_progress", (0, 1, 2), 5),
    "test_subject": ("event:/music/act3_boss_test_subject", "test_subject_progress", (0, 1, 2), 5),
    "queen": ("event:/music/act3_boss_queen", "queen_progress", (1, 2), 5),
}
# Music and ambience loops: (key, event, global Progress or None, seconds of loop, event
# parameters). The tracks don't repeat a short section, so a long stretch is rendered from the
# event's top (as the game starts it) and the next XFADE seconds are mixed into its head: the
# file loops without a seam. The rest site's track state is silent (the campfire's ambience
# plays alone); `Campfire` 1 is the fire gone out (TriggerCampfireGoingOut).
LOOPS = [("menu", "event:/music/menu_update", None, 240, ())]
for _t, _ev in ACT_TRACKS.items():
    for _s, _v in TRACK_STATES.items():
        if _s != "rest":
            LOOPS.append((f"{_t}_{_s}", _ev, _v, 300 if _s in ("enemy", "elite") else 150, ()))
for _a, _ev in ACT_AMBIENCE.items():
    for _s, _v in TRACK_STATES.items():
        if _s not in ("elite2", "merchant_end"):
            LOOPS.append((f"{_a}_{_s}", _ev, _v, 120, (("Campfire", 0.0),) if _s == "rest" else ()))
    LOOPS.append((f"{_a}_rest_out", _ev, TRACK_STATES["rest"], 120, (("Campfire", 1.0),)))
for _b, (_ev, _p, _vals, _end) in BOSSES.items():
    for _v in _vals:
        LOOPS.append((f"boss_{_b}_{_v}", _ev, None, 150, ((_p, float(_v)),)))
# Boss tracks' second parameter (NCeremonialBeastBgVfx.UpdateRingingSfx: the player has Ringing;
# SoulFysh: a Beckon in hand): each value of the progress parameter is also rendered with the
# flag at 1, as `boss_{b}_{v}_{flag}`.
BOSS_FLAGS = {"ceremonial_beast": "ringing", "soul_fysh": "beckon"}
for _b, _flag in BOSS_FLAGS.items():
    _ev, _p, _vals, _end = BOSSES[_b]
    for _v in _vals:
        LOOPS.append((f"boss_{_b}_{_v}_{_flag}", _ev, None, 150, ((_p, float(_v)), (_flag, 1.0))))
# SurroundedPower.FaceDirection sets the Kaiser Crab track's `kaiser_crab_direction` (1 facing
# left, 2 right) on the playing event: each progress value is also rendered with it, as
# `boss_kaiser_crab_{v}_dir{d}`.
BOSS_DIRS = {"kaiser_crab": ("kaiser_crab_direction", (1, 2))}
for _b, (_dp, _dvals) in BOSS_DIRS.items():
    _ev, _p, _vals, _end = BOSSES[_b]
    for _v in _vals:
        for _d in _dvals:
            LOOPS.append((f"boss_{_b}_{_v}_dir{_d}", _ev, None, 150, ((_p, float(_v)), (_dp, float(_d)))))
# Creatures' sound loops (SfxCmd.PlayLoop; the game stops them with their `loop` parameter or by
# the creature leaving): a stretch of the event with its head crossfaded to its end, as
# `sfxloop_{key}`. The Waterfall Giant's parameter is `waterfall_giant_sfx` (stop value 2).
SFX_LOOPS = [("event:/sfx/enemy/enemy_attacks/" + e, secs) for e, secs in (
    ("waterfall_giant/waterfall_giant_ambient", 30), ("orb_plant/orb_plant_idle_loop", 10),
    ("orb_plant/orb_plant_charged_loop", 10), ("cubex_construct/cubex_construct_charge_attack", 12),
    ("egg_layer/egg_layer_idle_loop", 10), ("slumbering_beetle/slumbering_beetle_sleep_loop", 6),
    ("owl_magistrate/owl_magistrate_fly_loop", 6), ("thieving_hopper/thieving_hopper_hover_loop", 10),
    ("kaiser_crab/kaiser_crab_right_buff", 12))]
for _ev, _secs in SFX_LOOPS:
    LOOPS.append(("sfxloop_" + re.sub(r"[^a-z0-9_]+", "_", _ev.split(":/", 1)[1].lower()).strip("_"), _ev, None, _secs, ()))
# One-shots rendered per parameter value, as `{key}_{value}` under the manifest path
# `{event}#{param}={value}`: the cultists' attack grows with their strength
# (CalcifiedCultist/DampCultist.PlayAttackSfx: `enemy_strength` rises 0.2 per attack, range 0-1).
PARAM_SFX = [(f"event:/sfx/enemy/enemy_attacks/cultists/cultists_attack", "enemy_strength", v) for v in (0.0, 0.2, 0.4, 0.6, 0.8, 1.0)]
XFADE = 6
# Seconds slept after every mixed block. The music's layers are streamed from the bank by FMOD's own thread;
# a render running as fast as the CPU allows outruns it, and whole stretches of a track come out silent or
# with only the preloaded blips (a pulse every 0.4 s: "choppy"), different on every run. Measured on
# act1_a1_enemy: 0 s gives silence or pulses; 0.002 s and up (up to real time, 0.0213 s) give the identical,
# continuous render; 0.0005 s is nearly right but starts a layer late. About 10x real time.
PACE = 0.002
# The web player's subset (www/sound.js): what the page plays, not every event the game has.
WEB_SFX = re.compile(r"^event:/(sfx/(block_(gain|break|hit)|buff|debuff|heal|ui/(gain_energy|clicks/(ui_click|ui_hover|ui_back)|gold/gold_1|"
                     r"map/map_(select|open)|deck_(open|close)|treasure/treasure_act\d$|cards/(card_movement_B_(into_discard|into_draw|power)|card_impact_into_single))|"
                     r"characters/\w+/\w+_(attack|cast|die|select)$|enemy/(enemy_fade|enemy_impact_enemy_size/enemy_impact_base)$|npcs/merchant/merchant_welcome)|temp/sfx/game_over)$")
WEB_FILES = {"victory", "player_turn", "enemy_turn", "battle_start_1", "battle_start_2", "card_deal", "deny", "relic_get",
             "gain_potion", "heavy_attack", "blunt_attack", "slash_attack", "card_exhaust"}
WEB_LOOP = re.compile(r"^(menu|act\d_a1_(enemy|elite|unknown|merchant)|boss_\w+_0)$")
LOOP_SECONDS = 90
# Event parameters the game sets: (path regex, name, value or label). Enemy hits always play at
# intensity 2 (SfxCmd.PlayDamage); the menu sets its labelled parameter to "main" (NMainMenu).
PARAMS = [
    (r"^event:/sfx/enemy/enemy_impact_enemy_size/", "EnemyImpact_Intensity", 2.0),
    (r"^event:/music/menu_update$", "menu_progress", "main"),
]
# Endings: (key, event, start parameters, seconds before the switch, seconds after, switched
# parameter, value, the loop that follows). A won fight moves the act's track from Enemy to
# CombatEnd; a boss's death sets its parameter to its end value. The file starts at the switch (the
# player crossfades into it) and hands over to `next` when it runs out.
ENDS = [(f"{t}_end", ev, (), 40, 40, "Progress", COMBAT_END, f"{t}_after") for t, ev in ACT_TRACKS.items()]
ENDS += [(f"boss_{b}_end", ev, ((p, float(vals[0])),), 30, 60, p, end, None) for b, (ev, p, vals, end) in BOSSES.items()]



def read_index(path):
    """The pack's files: path -> (absolute offset, size)."""
    with open(path, "rb") as f:
        magic, fmt, major, minor, patch, flags, base, diro = struct.unpack("<4sIIIIIQQ", f.read(40))
        if magic != b"GDPC" or fmt != 3 or flags & 1:
            sys.exit(f"{path}: not a plain Godot 4.5 pack")
        f.seek(diro)
        (n,) = struct.unpack("<I", f.read(4))
        files = {}
        for _ in range(n):
            (ln,) = struct.unpack("<I", f.read(4))
            name = f.read(ln).rstrip(b"\0").decode()
            off, size = struct.unpack("<QQ", f.read(16))
            f.read(16)
            (fl,) = struct.unpack("<I", f.read(4))
            if not fl & 2:
                files[name] = (off + base, size)
        return files


def pack_index(pck):
    pck = pck or next((p for p in PCKS if os.path.isfile(p)), PCKS[-1])
    return read_index(pck), pck


# --- The bank's object graph -------------------------------------------------------------------

def riff(d, off, end, out):
    """The RIFF tree: LIST nodes {id, off, len, kids}, chunks with kids None."""
    while off + 8 <= end:
        cid = d[off:off + 4]
        (ln,) = struct.unpack_from("<I", d, off + 4)
        if cid in (b"RIFF", b"LIST"):
            node = {"id": d[off + 8:off + 12].decode("latin1"), "off": off + 12, "len": ln - 4, "kids": []}
            riff(d, off + 12, off + 8 + ln, node["kids"])
        else:
            node = {"id": cid.decode("latin1"), "off": off + 8, "len": ln, "kids": None}
        out.append(node)
        off += 8 + ln + (ln & 1)
    return out


def find(nodes, cid):
    for n in nodes:
        if n["id"] == cid:
            yield n
        if n["kids"]:
            yield from find(n["kids"], cid)


def fsb5_names(d):
    """The FSB5 sample bank's subsound names, by index."""
    at = d.find(b"FSB5")
    if at < 0:
        return []
    ver, n, shdr, names = struct.unpack_from("<IIII", d, at + 4)
    hdr = 0x3C if ver == 1 else 0x40
    base = at + hdr + shdr
    offs = struct.unpack_from(f"<{n}I", d, base)
    return [d[base + o:d.index(b"\0", base + o)].decode("latin1") for o in offs]


# Objects an event's sound hangs from; buses, parameters and snapshots are left out so the walk
# stays inside the event.
GRAPH_TYPES = ("EVNT", "TMLN", "MUIT", "WAIT", "MODU", "TRAN", "SPIT", "EVIT", "CMDI")


def event_samples(bank):
    """Event GUID -> (sorted FSB subsound names, largest playlist size) for the bank's events."""
    d = open(bank, "rb").read()
    tree = riff(d, 0, len(d), [])
    objs = {}
    for ty in GRAPH_TYPES:
        for n in find(tree, ty):
            k = n["kids"][0]
            objs[d[k["off"]:k["off"] + 16]] = (ty, n)
    names = fsb5_names(d)
    wavs = {}
    for n in find(tree, "WAV "):
        (i,) = struct.unpack_from("<I", d, n["off"] + 22)
        wavs[d[n["off"]:n["off"] + 16]] = names[i] if i < len(names) else f"#{i}"
    known = set(objs) | set(wavs)
    edges = {g: set() for g in known}
    for g, (_, n) in objs.items():
        blob = d[n["off"]:n["off"] + n["len"]]
        for k in range(16, len(blob) - 15):
            h = blob[k:k + 16]
            if h in known and h != g:
                edges[g].add(h)
                edges[h].add(g)
    events = [g for g, (ty, _) in objs.items() if ty == "EVNT"]
    out = {}
    for ev in events:
        seen, todo, got, widest = {ev}, [ev], set(), 1
        while todo:
            x = todo.pop()
            for y in edges[x]:
                if y in seen or (y in objs and objs[y][0] == "EVNT"):
                    continue
                seen.add(y)
                if y in wavs:
                    got.add(wavs[y])
                    continue
                ty, n = objs[y]
                if ty == "MUIT":
                    for k in n["kids"]:
                        if k["id"] == "PLST":
                            widest = max(widest, (k["len"] - 12) // 20)
                todo.append(y)
        out[ev] = (sorted(got), widest)
    return out


# --- FMOD -------------------------------------------------------------------------------------

class Fmod:
    """The install's FMOD Studio runtime rendering into one wav, non-realtime."""

    def __init__(self, game, banks, wav):
        self.core = C.CDLL(os.path.join(game, "libfmod.so.14"), mode=C.RTLD_GLOBAL)
        self.st = C.CDLL(os.path.join(game, "libfmodstudio.so.14"))
        self.sys = C.c_void_p()
        self.check(self.st.FMOD_Studio_System_Create(C.byref(self.sys), 0x00020306), "create")
        core = C.c_void_p()
        self.st.FMOD_Studio_System_GetCoreSystem(self.sys, C.byref(core))
        self.core.FMOD_System_SetOutput(core, 5)  # WAVWRITER_NRT
        self.core.FMOD_System_SetSoftwareFormat(core, RATE, 3, 0)  # stereo
        self.core.FMOD_System_SetDSPBufferSize(core, BLOCK, 4)
        self.wav = wav
        self.check(self.st.FMOD_Studio_System_Initialize(self.sys, 512, 2, 0, C.c_char_p(wav.encode())), "init")  # SYNCHRONOUS_UPDATE
        for b in banks:
            h = C.c_void_p()
            self.check(self.st.FMOD_Studio_System_LoadBankFile(self.sys, b.encode(), 0, C.byref(h)), b)
        self.blocks = 0

    @staticmethod
    def check(r, what):
        if r:
            sys.exit(f"FMOD error {r}: {what}")

    def events(self):
        """Every event: path, GUID, oneshot, length (ms)."""
        out = []
        n = C.c_int()
        self.st.FMOD_Studio_System_GetBankCount(self.sys, C.byref(n))
        banks = (C.c_void_p * n.value)()
        self.st.FMOD_Studio_System_GetBankList(self.sys, banks, n, C.byref(n))
        for b in banks:
            self.st.FMOD_Studio_Bank_GetEventCount(C.c_void_p(b), C.byref(n))
            evs = (C.c_void_p * max(n.value, 1))()
            self.st.FMOD_Studio_Bank_GetEventList(C.c_void_p(b), evs, n, C.byref(n))
            for e in evs[:n.value]:
                e = C.c_void_p(e)
                buf, got = C.create_string_buffer(512), C.c_int()
                self.st.FMOD_Studio_EventDescription_GetPath(e, buf, 512, C.byref(got))
                guid = (C.c_ubyte * 16)()
                self.st.FMOD_Studio_EventDescription_GetID(e, guid)
                one, length = C.c_int(), C.c_int()
                self.st.FMOD_Studio_EventDescription_IsOneshot(e, C.byref(one))
                self.st.FMOD_Studio_EventDescription_GetLength(e, C.byref(length))
                out.append({"path": buf.value.decode(), "guid": bytes(guid), "oneshot": bool(one.value), "ms": length.value})
        return {e["path"]: e for e in out}

    def load(self, paths):
        """Loads the events' sample data up front: loading mixes blocks of its own."""
        descs = {}
        for p in paths:
            d = C.c_void_p()
            self.check(self.st.FMOD_Studio_System_GetEvent(self.sys, p.encode(), C.byref(d)), p)
            self.st.FMOD_Studio_EventDescription_LoadSampleData(d)
            descs[p] = d
        self.st.FMOD_Studio_System_FlushSampleLoading(self.sys)
        return descs

    def update(self, n=1):
        for _ in range(n):
            self.st.FMOD_Studio_System_Update(self.sys)
            self.blocks += 1
            time.sleep(PACE)

    def progress(self, value):
        self.st.FMOD_Studio_System_SetParameterByName(self.sys, b"Progress", C.c_float(value), 1)

    def param(self, inst, name, v, at_once=True):
        if isinstance(v, str):
            r = self.st.FMOD_Studio_EventInstance_SetParameterByNameWithLabel(inst, name.encode(), v.encode(), at_once)
        else:
            r = self.st.FMOD_Studio_EventInstance_SetParameterByName(inst, name.encode(), C.c_float(v), at_once)
        self.check(r, f"parameter {name}")

    def play(self, desc, seconds, switch=None, params=()):
        """Renders one instance until it stops or `seconds` pass; `switch` = (at_s, name, value)
        changes a parameter mid-way (`Progress`: the global one). Returns (first, last) block,
        stopped."""
        inst = C.c_void_p()
        self.st.FMOD_Studio_EventDescription_CreateInstance(desc, C.byref(inst))
        for name, v in params:
            self.param(inst, name, v)
        start = self.blocks
        self.st.FMOD_Studio_EventInstance_Start(inst)
        state = C.c_int()
        cap = int(seconds * RATE / BLOCK)
        while self.blocks - start < cap:
            if switch and self.blocks - start == int(switch[0] * RATE / BLOCK):
                if switch[1] == "Progress":
                    self.progress(switch[2])
                else:
                    self.param(inst, switch[1], switch[2], at_once=False)
            self.update()
            self.st.FMOD_Studio_EventInstance_GetPlaybackState(inst, C.byref(state))
            if state.value == 2:  # STOPPED
                break
        stopped = state.value == 2
        self.st.FMOD_Studio_EventInstance_Stop(inst, 1)  # immediate
        self.st.FMOD_Studio_EventInstance_Release(inst)
        return start, self.blocks, stopped

    def close(self):
        self.st.FMOD_Studio_System_Release(self.sys)


def open_wav(path):
    w = wave.open(path)
    assert w.getnchannels() == 2 and w.getsampwidth() == 2 and w.getframerate() == RATE, path
    return w


def read_frames(w, first, last):
    """Frames [first, last) as interleaved stereo (the render runs to hours: read per clip)."""
    w.setpos(max(first, 0))
    return array.array("h", w.readframes(max(last - max(first, 0), 0)))


def trim(pcm, quiet=8):
    """Drops digital silence before the sound and the quiet tail after it (interleaved stereo)."""
    n = len(pcm) // 2
    lo = 0
    while lo < n and abs(pcm[2 * lo]) <= 2 and abs(pcm[2 * lo + 1]) <= 2:
        lo += 1
    hi = n
    while hi > lo and abs(pcm[2 * hi - 2]) <= quiet and abs(pcm[2 * hi - 1]) <= quiet:
        hi -= 1
    return pcm[2 * lo:2 * hi]


def encode(pcm, out, kbps):
    """Interleaved 16-bit stereo -> ogg/opus via ffmpeg."""
    p = subprocess.run([FFMPEG, "-v", "error", "-y", "-f", "s16le", "-ar", str(RATE), "-ac", "2", "-i", "-",
                        "-c:a", "libopus", "-b:a", f"{kbps}k", "-vbr", "on", out], input=pcm.tobytes())
    if p.returncode:
        sys.exit(f"ffmpeg failed on {out}")
    to_m4a(out, kbps + 16)


def to_m4a(ogg, kbps):
    """The same clip as AAC in .m4a next to `ogg`: every iOS Safari decodes it, while older ones
    don't decode ogg/opus (the page picks per browser). Returns its size."""
    out = ogg[:-4] + ".m4a"
    p = subprocess.run([FFMPEG, "-v", "error", "-y", "-i", ogg, "-c:a", "aac", "-b:a", f"{kbps}k", "-movflags", "+faststart", out])
    if p.returncode:
        sys.exit(f"ffmpeg failed on {out}")
    return os.path.getsize(out)


def loop_seam(pcm, seconds):
    """`seconds` of interleaved stereo whose head has the following XFADE seconds faded in, so
    the end runs on into the start."""
    n = int(seconds * RATE)
    x = min(XFADE * RATE, len(pcm) // 2 - n)
    head = pcm[:2 * n]
    for i in range(x):
        g = i / x
        for c in (0, 1):
            v = int(head[2 * i + c] * g + pcm[2 * (n + i) + c] * (1 - g))
            head[2 * i + c] = max(-32768, min(32767, v))
    return head


def params_for(path):
    return [(n, v) for rx, n, v in PARAMS if re.search(rx, path)]


def key(path):
    return re.sub(r"[^a-z0-9_]+", "_", path.split(":/", 1)[1].lower()).strip("_")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--pck")
    ap.add_argument("--game", default=GAME, help="the install (its libfmod*.so)")
    ap.add_argument("--out", default=CACHE + "/audio")
    ap.add_argument("--only", help="regex over the sfx event paths to render")
    ap.add_argument("--add", action="store_true",
                    help="render only what a build made before GD29 lacks (parameter takes, creature loops, boss flags) into --out, keeping it")
    ap.add_argument("--sfx", help="render only the sfx events whose path matches this regex into --out, keeping it (GD41)")
    ap.add_argument("--ends", help="render only the outros whose key matches this regex into --out, keeping it")
    ap.add_argument("--list", action="store_true", help="print the event paths and stop")
    ap.add_argument("--m4a", action="store_true", help="only add an .m4a (AAC) next to every .ogg already in --out, and mark the manifest")
    ap.add_argument("--takes", type=int, default=2, help="most takes of an event with random picks")
    a = ap.parse_args()

    if a.m4a:
        total = 0
        for n in sorted(os.listdir(a.out)):
            if n.endswith(".ogg"):
                total += to_m4a(os.path.join(a.out, n), 80 if os.path.getsize(os.path.join(a.out, n)) > 100_000 else 72)
        m = json.load(open(os.path.join(a.out, "audio.json")))
        m["alt"] = ".m4a"
        json.dump(m, open(os.path.join(a.out, "audio.json"), "w"), indent=1, sort_keys=True)
        print(f"{a.out}: .m4a added, {total / 1e6:.1f} MB")
        return
    global LOOPS, ENDS, PARAM_SFX
    LOOPS = [(k, ev, pr, LOOP_SECONDS, ex) for k, ev, pr, secs, ex in LOOPS if WEB_LOOP.match(k)]
    ENDS, PARAM_SFX = [], []
    index, pck = pack_index(a.pck)
    work = os.path.join(CACHE, "audio-work")
    if not a.add and not a.ends and not a.sfx:
        shutil.rmtree(a.out, ignore_errors=True)
    os.makedirs(a.out, exist_ok=True)
    os.makedirs(work, exist_ok=True)
    banks = []
    with open(pck, "rb") as f:
        for p, (off, size) in sorted(index.items()):
            if p.startswith("banks/desktop/") and p.endswith(".bank"):
                dst = os.path.join(work, os.path.basename(p))
                if not (os.path.isfile(dst) and os.path.getsize(dst) == size):
                    f.seek(off)
                    with open(dst, "wb") as o:
                        o.write(f.read(size))
                banks.append(dst)
    # The strings bank first (event paths), then Master (the mixer), then the rest.
    banks.sort(key=lambda b: (not b.endswith("strings.bank"), not b.endswith("Master.bank"), b))

    samples = {}
    for b in banks:
        samples.update(event_samples(b))

    fm = Fmod(a.game, banks, os.path.join(work, "events.wav"))
    events = fm.events()
    fm.close()
    if a.list:
        print("\n".join(sorted(events)))
        return
    sfx = sorted(p for p, e in events.items()
                 if WEB_SFX.match(p))
    if a.only:
        sfx = [p for p in sfx if re.search(a.only, p)]

    # One FMOD session per batch: a session writes one wav, and a wav holds at most 4 GB (about
    # three hours), so the sfx go first and the music in batches of under 100 minutes.
    manifest = {"events": {}, "music": {}}
    old = os.path.join(a.out, "audio.json")
    if a.add or a.ends or a.sfx:
        manifest = json.load(open(old))
        if a.sfx:
            sfx = [p for p in sfx if re.search(a.sfx, p)]
            for p in sfx:
                manifest["events"].pop(p, None)
        else:
            sfx = []
    total = 0
    batches = [(sfx, [], [])]
    loops, ends, secs = [], [], 0
    new_loops = [] if a.ends or a.sfx else [l for l in LOOPS if not a.add or l[0] not in manifest["music"]]
    for l in new_loops:
        if secs + l[3] > 6000:
            batches.append(([], loops, []))
            loops, secs = [], 0
        loops.append(l)
        secs += l[3] + XFADE + 50 * BLOCK / RATE
    batches.append(([], loops, [e for e in ENDS if re.search(a.ends, e[0])] if a.ends else [] if a.add or a.sfx else list(ENDS)))
    for i, (b_sfx, b_loops, b_ends) in enumerate(batches):
        if a.ends and i == 0:
            continue
        if not (a.add and i == 0 and all(f"{p}#{n}={v}" in manifest["events"] for p, n, v in PARAM_SFX)):
            total += render(a, banks, os.path.join(work, f"render{i}.wav"), events, samples, b_sfx, b_loops, b_ends, manifest, first_batch=i == 0 and not a.sfx)
    # The sounds the game plays as files rather than FMOD events (TmpSfx: turn banners, card
    # draw and exhaust, victory), as they are.
    manifest["files"] = {}
    with open(pck, "rb") as f:
        for p, (off, size) in sorted(index.items()):
            if p.startswith("debug_audio/") and p.endswith((".mp3", ".wav")) and os.path.splitext(os.path.basename(p))[0] in WEB_FILES:
                f.seek(off)
                data = f.read(size)
                name = os.path.basename(p)
                with open(os.path.join(a.out, name), "wb") as o:
                    o.write(data)
                manifest["files"][os.path.splitext(name)[0]] = name
                total += size
    with open(os.path.join(a.out, "audio.json"), "w") as f:
        manifest["alt"] = ".m4a"  # every .ogg has an .m4a beside it
        json.dump(manifest, f, indent=1, sort_keys=True)
    shutil.rmtree(work)
    unmapped = [p for p in sfx if not samples.get(events[p]["guid"], ([], 1))[0]]
    print(f"{a.out}: {len(manifest['events'])}/{len(sfx)} sfx events, {len(manifest['music'])} music/ambience, {len(manifest['files'])} files, "
          f"{total / 1e6:.1f} MB; {len(unmapped)} events without a waveform: {unmapped}")


def render(a, banks, wav, events, samples, sfx, loops, ends, manifest, first_batch=False):
    """Renders `sfx` takes, `loops` and `ends` in one FMOD session into `wav`, cuts them out into
    `a.out` and adds them to `manifest`; returns the bytes written."""
    fm = Fmod(a.game, banks, wav)
    descs = fm.load((sorted({p for p, _, _ in PARAM_SFX}) if first_batch else []) + sfx + sorted({l[1] for l in loops + ends}))
    # A calibration take first: the wav holds blocks mixed before the first update (init and
    # loading), found from the file's length at the end.
    fm.update(4)
    jobs = []  # (kind, key, path, first block, last block)
    for p, name, v in PARAM_SFX if first_batch else []:
        e = events[p]
        _, widest = samples.get(e["guid"], ([], 1))
        for t in range(min(a.takes, widest)):
            s0, s1, _ = fm.play(descs[p], 12, params=params_for(p) + [(name, v)])
            fm.update(10)
            jobs.append(("sfx", f"{key(p)}_{name}_{v}_{t}", f"{p}#{name}={v}", s0, fm.blocks))
            fm.update(4)
    for p in sfx:
        e = events[p]
        _, widest = samples.get(e["guid"], ([], 1))
        takes = min(a.takes, widest) if e["oneshot"] else 1
        for t in range(takes):
            s0, s1, _ = fm.play(descs[p], 12 if e["oneshot"] else 8, params=params_for(p))
            fm.update(10)  # the bus effects' tails
            jobs.append(("sfx", key(p) + (f"_{t}" if takes > 1 else ""), p, s0, fm.blocks))
            fm.update(4)
    for k, p, prog, secs, extra in loops:
        if prog is not None:
            fm.progress(prog)
        s0, s1, stopped = fm.play(descs[p], secs + XFADE, params=params_for(p) + list(extra))
        # A state that runs out (an outro) plays once.
        jobs.append(("loop" if not stopped else "once", k, p, s0, s1))
        if stopped:
            print(f"{k}: {p} stops after {(s1 - s0) * BLOCK / RATE:.0f} s; plays once", file=sys.stderr)
        fm.progress(0)
        fm.update(50)
    for k, p, start, at, secs, name, value, _ in ends:
        fm.progress(ENEMY)
        s0, s1, _ = fm.play(descs[p], at + secs, switch=(at, name, value), params=list(start))
        jobs.append(("end", k, p, s0 + int(at * RATE / BLOCK), s1))
        fm.progress(0)
        fm.update(50)
    fm.close()

    w = open_wav(wav)
    frames = w.getnframes()
    # FMOD writes a few blocks on release as well (measured: 4).
    pre = frames - (fm.blocks + 4) * BLOCK
    total = 0
    for kind, k, p, s0, s1 in jobs:
        clip = read_frames(w, pre + s0 * BLOCK, pre + s1 * BLOCK)
        if kind == "sfx":
            clip = trim(clip)
        elif kind == "loop":
            clip = loop_seam(clip, next(l[3] for l in LOOPS if l[0] == k))
        else:
            clip = trim(clip)
        out = os.path.join(a.out, k + ".ogg")
        if len(clip) < 2 * 64:
            print(f"silent: {p} ({k})", file=sys.stderr)
            continue
        encode(clip, out, 56 if kind == "sfx" else 64)
        total += os.path.getsize(out)
        if kind == "sfx":
            base = events[p.split("#")[0]]
            e = manifest["events"].setdefault(p, {"takes": [], "loop": not base["oneshot"],
                                                  "samples": samples.get(base["guid"], ([], 1))[0]})
            e["takes"].append(k + ".ogg")
        else:
            m = manifest["music"][k] = {"file": k + ".ogg", "event": p, "seconds": round(len(clip) / 2 / RATE, 2), "loop": kind == "loop"}
            follow = next((e[7] for e in ENDS if e[0] == k), None)
            if follow:
                m["next"] = follow
    w.close()
    os.remove(wav)
    return total


if __name__ == "__main__":
    main()
