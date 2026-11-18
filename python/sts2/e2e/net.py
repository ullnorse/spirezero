"""The end-to-end net (`docs/plans/necro-e2e.md` §1, WP4).

- **Embedding:** a token is the sum of its row of the shared ID table (`encode.ID_ROWS` x d),
  its kind, its `cat` and `owner` embeddings and a lean numeric stem (Linear -> SiLU -> Linear
  over the symlog columns; zero columns stay inert). The four summary tokens add the mean ID
  embedding of their pile's cards.
- **Trunk:** `layers` pre-norm blocks, d = 256, 4 heads, RMSNorm, SwiGLU 704, no positions;
  padded tokens are masked out of attention.
- **Policy head (entity):** the logit of index `a` is MLP(W_e h[ent(a)] + W_t h[tgt(a)] +
  W_s h[STATE] + e_kind(a) + e_sub(a)) -> 1, computed only for legal indices; the rest are
  -inf. The first layer is split per input so each token is projected once.
- **Value module:** one block over [trunk outputs; privileged tokens], read at STATE, two fp32
  outputs (combat rows, run rows). The privileged tokens have their own embeddings and stem;
  their ID-table lookups are detached. Nothing privileged reaches the policy: `policy()` takes
  no privileged input, and its parameters (`policy_parameters()`) exclude the value module's.
- **v2 inputs** (`obs_v2=True`, `encode.encode(..., v2=True)`): every `x_` parameter. They enter
  through weights that start at zero, so a v1 checkpoint loaded into a v2 net (`load_v1`) gives
  bit-identical logits and values until they train: the intent's enemy link (`link_gain` x the
  `owner` embedding of the linked creature), the forecast kinds (`fc`), the v2 numeric columns
  (`stem`, its output layer zero) and the MAPNODE tokens, a second stream `y` that each block
  runs with the same weights (y attends over x and y) and that x reads through one extra
  attention per block, scaled by `gate` (zero per block). The value module reads x only.
- **Side inputs** (RLN21, `x_side`, with `obs_v2` unless `side=False`): the crab facing and the
  powers' shown numbers (`encode`'s `znum`) through their own linear map (no bias), zero, so an
  ABI-8 v2 checkpoint loaded into this net (`load_v1`, `x_side` keys missing) gives the same bits.
- **Plan column** (RLN22, `x_plan`, with `obs_v2` unless `plan=False`): each legal action's plan
  column (`encode`'s `anum`) through its own linear map (no bias) into the head's hidden input
  `z`, zero, so an ABI-9 checkpoint (no `x_plan` keys, `load_net`) gives the same bits.
- **Content stats** (RLN29, `card_stats=True`, `cstats`): a fixed numeric vector per card (base
  and upgraded), relic and potion (`encode.stats_table`, from `sts2_rl::content_stats`), a
  buffer saved with the weights, through one linear map (no bias) added to every token's ID
  embedding (and to the pile summaries' pooled cards); a card token reads its upgraded row when
  its `up` column is set (`encode.STATS_UP_COL`). Other ID rows are zero, so they get nothing.
  Off (the default), the net has no such module and the same parameters, init and outputs.
- **`emb_std`** (RLN29): the init std of the ID table and the kind, `cat` and `owner`
  embeddings (default 0.02, the earlier nets').
- **Full view** (RLN30, `obs_full=True`, with `obs_v2`; `encode(..., full=True)`): `full` holds
  the full view's token kinds (`encode.FULL_KINDS`, after `kind`'s rows) and its per-token
  extras, added to the token: the sum of the `hcat` slots' embeddings (a past move's intent
  kind; a map row's point types and edges), of the `hmove` slots' (a past move's key) and a stem
  over `hnum` (past moves' numbers; a pile token's counts). A fresh-run option: off, the net has no such module
  and the same parameters, init and outputs; its parameters train in the policy group.
- **Event options** (RLN33, `obs_events=True`, with `obs_v2`; `encode(..., events=True)`):
  `events` maps each ancient and event option's vector into its tokens (`ev_slot`): a linear
  map of `evec`, plus the ID table's rows of the cards, relics and potions it names (`eids`, and
  their stats rows with `card_stats`) and an embedding of the enchantment (`ench`). Zero vectors
  add nothing. Off, the net has no such module and the same parameters, init and outputs.
"""

import numpy as np
import torch
import torch.nn as nn
import torch.nn.functional as F

from . import encode as E


class Block(nn.Module):
    def __init__(self, d=256, heads=4, ffn=704):
        super().__init__()
        self.heads = heads
        self.n1 = nn.RMSNorm(d)
        self.qkv = nn.Linear(d, 3 * d, bias=False)
        self.out = nn.Linear(d, d, bias=False)
        self.n2 = nn.RMSNorm(d)
        self.w1 = nn.Linear(d, ffn, bias=False)
        self.w3 = nn.Linear(d, ffn, bias=False)
        self.w2 = nn.Linear(ffn, d, bias=False)

    def forward(self, x, valid):
        b, t, d = x.shape
        q, k, v = self.qkv(self.n1(x)).view(b, t, 3, self.heads, d // self.heads).unbind(2)
        q, k, v = (z.transpose(1, 2) for z in (q, k, v))
        a = F.scaled_dot_product_attention(q, k, v, attn_mask=valid[:, None, None, :])
        x = x + self.out(a.transpose(1, 2).reshape(b, t, d))
        h = self.n2(x)
        return x + self.w2(F.silu(self.w1(h)) * self.w3(h))

    def forward2(self, x, valid, y, y_valid, rows, gate):
        """`forward` of x plus `gate` x x's attention over the second stream `y` (R x F x d,
        `y_valid`) of the rows `rows` (R indices), and y's own step (attending over its row's x
        and y). With `gate` 0, x's output is `forward(x, valid)` bit for bit: its own maths runs
        unchanged on every row and `rows` add an exact zero."""
        b, t, d = x.shape
        r, f = y.shape[:2]
        hd = d // self.heads
        q, k, v = self.qkv(self.n1(x)).view(b, t, 3, self.heads, hd).unbind(2)
        q, k, v = (z.transpose(1, 2) for z in (q, k, v))
        a = F.scaled_dot_product_attention(q, k, v, attn_mask=valid[:, None, None, :])
        qy, ky, vy = self.qkv(self.n1(y)).view(r, f, 3, self.heads, hd).unbind(2)
        qy, ky, vy = (z.transpose(1, 2) for z in (qy, ky, vy))
        has = y_valid.any(1)
        # A row without y tokens attends to its first (masked to zero below), never to nothing.
        safe = y_valid | (~has).unsqueeze(1) & (torch.arange(f, device=x.device) == 0)
        c = F.scaled_dot_product_attention(q[rows], ky, vy, attn_mask=safe[:, None, None, :])
        a = a.index_add(0, rows, gate * (c * has.view(r, 1, 1, 1).to(c.dtype)))
        x = x + self.out(a.transpose(1, 2).reshape(b, t, d))
        h = self.n2(x)
        x = x + self.w2(F.silu(self.w1(h)) * self.w3(h))
        ay = F.scaled_dot_product_attention(
            qy, torch.cat([k[rows], ky], 2), torch.cat([v[rows], vy], 2),
            attn_mask=torch.cat([valid[rows], y_valid], 1)[:, None, None, :])
        y = y + self.out(ay.transpose(1, 2).reshape(r, f, d))
        hy = self.n2(y)
        return x, y + self.w2(F.silu(self.w1(hy)) * self.w3(hy))

    def enable_compile(self):
        """Compile `forward`, `forward2` and `forward_first` (inductor, dynamic batch and width).
        They shadow the methods as instance attributes, so state_dict keys and calls are
        unchanged."""
        self.forward = torch.compile(self.forward, dynamic=True)
        self.forward2 = torch.compile(self.forward2, dynamic=True)
        self.forward_first = torch.compile(self.forward_first, dynamic=True)

    def forward_first(self, x, valid):
        """`forward(x, valid)[:, :1]`, computing only position 0's query, attention output and
        feed-forward (the same maths; the other positions are keys and values only)."""
        b, t, d = x.shape
        hd = d // self.heads
        n = self.n1(x)
        w = self.qkv.weight
        q = F.linear(n[:, :1], w[:d]).view(b, 1, self.heads, hd).transpose(1, 2)
        k, v = F.linear(n, w[d:]).view(b, t, 2, self.heads, hd).unbind(2)
        k, v = k.transpose(1, 2), v.transpose(1, 2)
        a = F.scaled_dot_product_attention(q, k, v, attn_mask=valid[:, None, None, :])
        x = x[:, :1] + self.out(a.transpose(1, 2).reshape(b, 1, d))
        h = self.n2(x)
        return x + self.w2(F.silu(self.w1(h)) * self.w3(h))


class Stem(nn.Module):
    def __init__(self, d, inputs=E.NUM, zero_out=False):
        super().__init__()
        self.a = nn.Linear(inputs, d)
        self.b = nn.Linear(d, d)
        if zero_out:
            nn.init.zeros_(self.b.weight)
            nn.init.zeros_(self.b.bias)

    def forward(self, num):
        return self.b(F.silu(self.a(num)))


class V2Inputs(nn.Module):
    """The v2 inputs' weights (`E2ENet` module doc); zero at the start where they reach x."""

    def __init__(self, d, layers):
        super().__init__()
        self.link_gain = nn.Parameter(torch.zeros(d))
        self.fc = nn.Embedding(E.STEPS * E.FC_KINDS, d)
        nn.init.zeros_(self.fc.weight)
        self.stem = Stem(d, E.XNUM, zero_out=True)
        self.gate = nn.Parameter(torch.zeros(layers))
        # The MAPNODE stream's own embedding (it reaches x only through the gates).
        self.node = nn.Parameter(torch.zeros(d))
        nn.init.normal_(self.node, std=0.02)
        self.ystem = Stem(d, E.XNUM)


class SideInputs(nn.Module):
    """The RLN21 inputs' weights (`E2ENet` module doc); zero at the start where they reach x."""

    def __init__(self, d):
        super().__init__()
        # Flags and one number per token: a linear map is enough, and nearly free per token.
        self.proj = nn.Linear(E.ZNUM, d, bias=False)
        nn.init.zeros_(self.proj.weight)


class PlanInputs(nn.Module):
    """The RLN22 input's weights (`E2ENet` module doc); zero at the start."""

    def __init__(self, head_hidden):
        super().__init__()
        self.proj = nn.Linear(E.ANUM, head_hidden, bias=False)
        nn.init.zeros_(self.proj.weight)


class FullInputs(nn.Module):
    """The RLN30 full view's weights (`E2ENet` module doc)."""

    def __init__(self, d, std):
        super().__init__()
        self.kind = nn.Embedding(len(E.FULL_KINDS), d)
        self.hcat = nn.Embedding(E.HCAT_ROWS, d)
        self.hmove = nn.Embedding(1 + E.HIST * E.MOVE_BUCKETS, d)
        self.stem = Stem(d, E.HNUM)
        for emb in (self.kind, self.hcat, self.hmove):
            nn.init.normal_(emb.weight, std=std)
        with torch.no_grad():  # "no move" adds nothing
            self.hcat.weight[0].zero_()
            self.hmove.weight[0].zero_()

    def tokens(self, hcat, hmove, hnum):
        """The extras' sum per token (B, T, d). RLN31: `embedding_bag` sums the slots without a
        (B, T, slots, d) tensor (the update ran 30% slower with one); index 0 adds nothing."""
        b, t = hcat.shape[:2]

        def bag(idx, emb):
            return F.embedding_bag(idx.reshape(b * t, -1), emb.weight, mode="sum", padding_idx=0)

        slots = (bag(hcat, self.hcat) + bag(hmove, self.hmove)).view(b, t, -1)
        return slots + self.stem(hnum)


class EventInputs(nn.Module):
    """The RLN33 event options' weights (`E2ENet` module doc)."""

    def __init__(self, d, std):
        super().__init__()
        self.proj = nn.Linear(E.EV_COLS, d, bias=False)
        self.ench = nn.Embedding(64, d)
        nn.init.normal_(self.ench.weight, std=std)
        with torch.no_grad():  # "no enchantment" adds nothing
            self.ench.weight[0].zero_()


class ContentStats(nn.Module):
    """The RLN29 content prior: the stats table (a buffer, 2 x `ID_ROWS` x columns: the base and
    upgraded rows by ID row) and its linear map into the token width."""

    def __init__(self, d, table):
        super().__init__()
        self.register_buffer("table", table.float().clone())
        self.proj = nn.Linear(table.shape[-1], d, bias=False)

    def rows(self):
        """The projected table, (2 x `ID_ROWS`) x d: index `up * ID_ROWS + ident`."""
        return self.proj(self.table.flatten(0, 1))


class RunHead(nn.Module):
    """RLN42: the run decisions' own policy head (rows outside combat), the shape of the shared
    one (`E2ENet.head`) without the plan column (zero on run actions): a warm start can
    re-initialise it and keep the combat head."""

    def __init__(self, d, head_hidden):
        super().__init__()
        self.pe = nn.Linear(d, head_hidden)
        self.pt = nn.Linear(d, head_hidden, bias=False)
        self.ps = nn.Linear(d, head_hidden, bias=False)
        self.akind = nn.Embedding(32, head_hidden)
        self.asub = nn.Embedding(16, head_hidden)
        self.pout = nn.Linear(head_hidden, 1)
        for emb in (self.akind, self.asub):
            nn.init.normal_(emb.weight, std=0.02)
        nn.init.zeros_(self.pout.weight)
        nn.init.zeros_(self.pout.bias)

    def logits(self, h, tok, rows, acts, ent, tgt):
        """The logits of the (row, action) pairs given (rows of `h`, entity and target tokens)."""
        t = h.shape[1]
        z = self.pe(h[rows, ent]) + self.ps(h[rows, 0])
        z = z + torch.where((tgt >= 0).unsqueeze(-1), self.pt(h[rows, tgt.clamp(0, t - 1)]),
                            torch.zeros_like(z))
        z = z + self.akind(tok["akind"][acts]) + self.asub(tok["asub"][acts].clamp(0, 15))
        return self.pout(F.silu(z)).squeeze(-1).float()


class E2ENet(nn.Module):
    def __init__(self, d=256, layers=4, heads=4, ffn=704, head_hidden=256, obs_v2=False,
                 side=None, plan=None, card_stats=False, emb_std=None, stats_table=None,
                 obs_full=False, obs_events=False, discard_potion=False, run_head=False):
        """`discard_potion` (RLN32): the logits cover the discard-potion indices (361 wide, else
        355: the same shapes as before, so the same bits). It adds no parameter.
        `card_stats`: the content prior (`stats_table`, default `encode.stats_table()`
        from the library); `emb_std`: the input embeddings' init std (None: 0.02)."""
        super().__init__()
        self.d = d
        self.obs_v2 = obs_v2
        # RLN21: the side inputs come with v2; `side=False` is the ABI-8 v2 net (references).
        self.side = obs_v2 if side is None else (side and obs_v2)
        # RLN22: the plan column comes with the side inputs; `plan=False` is the ABI-9 net.
        self.plan = self.side if plan is None else (plan and self.side)
        # Policy side (trunk and head).
        self.ids = nn.Embedding(E.ID_ROWS, d)
        self.kind = nn.Embedding(len(E.KINDS), d)
        self.cat = nn.Embedding(64, d)
        self.owner = nn.Embedding(32, d)
        self.stem = Stem(d)
        self.up = nn.Parameter(torch.zeros(d))
        self.blocks = nn.ModuleList(Block(d, heads, ffn) for _ in range(layers))
        self.norm = nn.RMSNorm(d)
        self.pe = nn.Linear(d, head_hidden)
        self.pt = nn.Linear(d, head_hidden, bias=False)
        self.ps = nn.Linear(d, head_hidden, bias=False)
        self.akind = nn.Embedding(32, head_hidden)
        self.asub = nn.Embedding(16, head_hidden)
        self.pout = nn.Linear(head_hidden, 1)
        # Value module (sees the privileged tokens).
        self.v_kind = nn.Embedding(len(E.PRIV_KINDS), d)
        self.v_move = nn.Embedding(E.MOVE_BUCKETS, d)
        self.v_move_next = nn.Embedding(E.MOVE_BUCKETS, d)
        self.v_grid = nn.Embedding(2 * 96 * E.ROOM_TYPES, d)
        self.v_stem = Stem(d)
        self.v_block = Block(d, heads, ffn)
        self.v_norm = nn.RMSNorm(d)
        self.v_hidden = nn.Linear(d, d)
        self.v_out = nn.Linear(d, 2)
        std = 0.02 if emb_std is None else emb_std
        nn.init.normal_(self.ids.weight, std=std)
        for emb in (self.kind, self.cat, self.owner, self.akind, self.asub, self.v_kind,
                    self.v_move, self.v_move_next, self.v_grid):
            nn.init.normal_(emb.weight, std=std if emb in (self.kind, self.cat, self.owner)
                            else 0.02)
        nn.init.zeros_(self.pout.weight)
        nn.init.zeros_(self.pout.bias)
        nn.init.zeros_(self.v_out.weight)
        nn.init.zeros_(self.v_out.bias)
        if obs_v2:
            # Registered last, so `parameters()` is the v1 net's list then these (the EMAs and
            # the optimiser's groups extend a v1 checkpoint's in that order).
            self.x_v2 = V2Inputs(d, layers)
        if self.side:
            self.x_side = SideInputs(d)
        if self.plan:
            self.x_plan = PlanInputs(head_hidden)
        self.card_stats = card_stats
        if card_stats:  # registered last: off, the parameter list and init are the same
            self.cstats = ContentStats(d, E.stats_table() if stats_table is None
                                       else stats_table)
        if obs_full and not obs_v2:
            raise ValueError("obs_full needs obs_v2")
        self.obs_full = obs_full
        self.n_actions = E.ACTIONS_ALL if discard_potion else E.ACTIONS
        if obs_full:  # registered last: off, the parameter list and init are the same
            self.full = FullInputs(d, std)
        if obs_events and not obs_v2:
            raise ValueError("obs_events needs obs_v2")
        self.obs_events = obs_events
        if obs_events:  # registered last: off, the parameter list and init are the same
            self.events = EventInputs(d, std)
        # RLN42: rows outside combat use their own head (registered last: off, the same net).
        self.run_head = RunHead(d, head_hidden) if run_head else None

    V2_PREFIX = "x_"
    SIDE_PREFIX = "x_side."
    PLAN_PREFIX = "x_plan."
    LATER_PREFIXES = (SIDE_PREFIX, PLAN_PREFIX)
    """The inputs added after RLN16's, in registration order (each its own optimiser group)."""

    VALUE_PREFIX = ("v_",)
    value_first = False
    """Run the value block for its read position only (`Block.forward_first`): the same
    function, cheaper; a learner switch, so a checkpoint's numbers do not depend on it."""

    fast_gather = False
    """RLN44: the content-stats rows by `F.embedding` (`trunk`): the same forward, a faster
    backward (same maths to float noise); a learner switch like `value_first`."""

    def enable_compile(self):
        for blk in (*self.blocks, self.v_block):
            blk.enable_compile()
        if self.obs_v2:
            self._v2_trunk = torch.compile(self._v2_trunk, dynamic=True)

    def value_parameters(self):
        return [p for n, p in self.named_parameters() if n.startswith(self.VALUE_PREFIX)]

    def policy_parameters(self):
        return [p for n, p in self.named_parameters() if not n.startswith(self.VALUE_PREFIX)]

    def trunk(self, tok):
        """Token dict -> (h (B, T, d), valid (B, T)) with T the batch's longest row (or
        `tok["t"]`, a host int at least that, when the caller knows it)."""
        t = tok["t"] if "t" in tok else int(tok["width"].max())
        sl = lambda k: tok[k][:, :t]  # noqa: E731
        valid = sl("valid")
        if self.obs_full:
            kind = F.embedding(sl("kind"), torch.cat([self.kind.weight, self.full.kind.weight]))
        else:
            kind = self.kind(sl("kind"))
        x = (self.ids(sl("ident")) + kind + self.cat(sl("cat").clamp(0, 63))
             + self.owner(sl("owner").clamp(0, 31)) + self.stem(sl("num")))
        if self.obs_full:
            x = x + self.full.tokens(sl("hcat"), sl("hmove"), sl("hnum"))
        if self.obs_events:
            x = x + self._event_tokens(sl("ev_slot"), tok["evec"], tok["eids"],
                                       tok["ench"]).to(x.dtype)
        # Pile summaries: the mean ID embedding of each group's cards, at positions 1-4.
        bag = self.ids(tok["bag_id"]) + tok["bag_up"].unsqueeze(-1) * self.up
        if self.card_stats:
            rows = self.cstats.rows()
            up = E.token_up(sl("kind"), sl("num"))
            # RLN44 `fast_gather`: the same rows by `F.embedding`, whose backward sums the many
            # repeated indices (padding, common cards) by sorted segments; the advanced-index
            # backward serialised them (46% of an update minibatch at NECRO4's shape).
            take = F.embedding if self.fast_gather else lambda i, w: w[i]
            x = x + take(up * E.ID_ROWS + sl("ident"), rows).to(x.dtype)
            bag = bag + take((tok["bag_up"] > 0).long() * E.ID_ROWS + tok["bag_id"],
                             rows).to(bag.dtype)
        groups = F.one_hot(tok["bag_group"], 5)[:, :, 1:].to(bag.dtype)  # B, BAG, 4
        summ = torch.einsum("bng,bnd->bgd", groups, bag)
        summ = summ / groups.sum(1).clamp(min=1).unsqueeze(-1)
        x = torch.cat([x[:, :1], x[:, 1:E.FIXED] + summ.to(x.dtype), x[:, E.FIXED:]], 1)
        if not self.obs_v2:
            for blk in self.blocks:
                x = blk(x, valid)
            return self.norm(x), valid
        # The MAPNODE stream runs only on the rows that may have MAPNODE tokens (map decisions,
        # a few percent): `tok["y_rows"]` (indices), else the rows with one. A caller that knows
        # them on the host (`y_rows_of`) saves a device sync per forward.
        rows = tok.get("y_rows")
        if rows is None:
            rows = tok["y_valid"].any(1).nonzero().squeeze(1)
        znum = sl("znum") if self.side else None
        x = self._v2_trunk(x, valid, sl("link"), sl("xcat"), sl("xnum"), tok["y_valid"],
                           tok["y_cat"], tok["y_num"], rows, znum)
        return self.norm(x), valid

    def _event_tokens(self, slot, evec, eids, ench):
        """The event options' additions per token (B, T, d): each of the row's 8 options' sum
        (ID rows by `embedding_bag`, id 0 weight 0), taken by the tokens that read it."""
        b, t, k = eids.shape
        flat = eids.reshape(b * t, k)
        on = flat > 0

        def bag(table):
            return F.embedding_bag(flat, table, per_sample_weights=on.to(table.dtype),
                                   mode="sum")

        named = bag(self.ids.weight)
        if self.card_stats:
            named = named + bag(self.cstats.rows()).to(named.dtype)
        per = (self.events.proj(evec.to(named.dtype)) + named.view(b, t, -1)
               + self.events.ench(ench))  # B, 8, d
        per = torch.cat([torch.zeros_like(per[:, :1]), per], 1).flatten(0, 1)  # B*9, d
        rows = torch.arange(b, device=slot.device).unsqueeze(1) * (t + 1)
        return F.embedding(rows + slot, per) if self.fast_gather else per[rows + slot]

    def _v2_trunk(self, x, valid, link, xcat, xnum, y_valid, y_cat, y_num, rows, znum=None):
        """The v2 inputs and the blocks (`trunk`); one graph under `enable_compile`."""
        xv = self.x_v2
        x = x + (xv.link_gain * self.owner(link.clamp(0, 31))
                 * (link > 0).unsqueeze(-1).to(x.dtype))
        x = x + xv.fc(xcat).sum(2) + xv.stem(xnum)
        if znum is not None:
            x = x + self.x_side.proj(znum)
        if rows.shape[0] == 0:
            for blk in self.blocks:
                x = blk(x, valid)
            return x
        y_valid = y_valid[rows]
        y = xv.node + self.cat(y_cat[rows].clamp(0, 63)) + xv.ystem(y_num[rows])
        y = y.to(x.dtype)
        for i, blk in enumerate(self.blocks):
            x, y = blk.forward2(x, valid, y, y_valid, rows, xv.gate[i].to(x.dtype))
        return x

    @staticmethod
    def y_rows_of(may_have_y, device):
        """`tok["y_rows"]` from a host bool array of the rows that may have MAPNODE tokens (a
        superset is fine: the map-phase rows), copied without a device sync."""
        t = torch.from_numpy(np.flatnonzero(may_have_y))
        if device.type != "cuda":
            return t.to(device)
        return t.pin_memory().to(device, non_blocking=True)

    def load_v1(self, state):
        """Loads a v1 net's `state_dict` into this v2 net, or an ABI-8 v2 net's (no `x_side`)
        into this one: every missing key must be an `x_` one (they keep their init, zero where it
        matters). Returns the missing keys."""
        missing, unexpected = self.load_state_dict(state, strict=False)
        bad = [k for k in missing if not k.startswith(self.V2_PREFIX)]
        if bad or unexpected:
            raise ValueError(f"not a v1 state: missing {bad}, unexpected {unexpected}")
        return missing

    def load_net(self, state):
        """Loads a checkpoint's net `state_dict`: strict, except that an older v2 net loads
        with the inputs it lacks at their init (the same outputs): an ABI-8 one (no `x_side`
        or `x_plan` keys) or an ABI-9 one (no `x_plan` keys)."""
        absent = tuple(p for p in self.LATER_PREFIXES if not any(k.startswith(p) for k in state))
        if self.obs_v2 and absent:
            missing, unexpected = self.load_state_dict(state, strict=False)
            if unexpected or any(not k.startswith(absent) for k in missing):
                raise ValueError(f"not an older v2 state: missing {missing}, unexpected "
                                 f"{unexpected}")
            return
        self.load_state_dict(state)

    def v2_parameters(self):
        """The RLN16 inputs' parameters (`x_v2`)."""
        return [p for n, p in self.named_parameters()
                if n.startswith(self.V2_PREFIX) and not n.startswith(self.LATER_PREFIXES)]

    def side_parameters(self):
        """The RLN21 inputs' parameters (`x_side`), registered after `x_v2`."""
        return [p for n, p in self.named_parameters() if n.startswith(self.SIDE_PREFIX)]

    def plan_parameters(self):
        """The RLN22 input's parameters (`x_plan`), registered last."""
        return [p for n, p in self.named_parameters() if n.startswith(self.PLAN_PREFIX)]

    def head(self, h, tok, run=True):
        """Logits (B, `n_actions`) float32, -inf off the legal mask; `run=False` scores the run
        rows with the shared head too (RLN42's distillation teacher)."""
        b = h.shape[0]
        legal = tok["legal"]
        # RLN44: with the pair count known on the host (`n_legal`, `encode.legal_counts`), the
        # same pairs in the same order without the device sync of `nonzero`.
        static = "n_legal" in tok
        if static:
            rows, acts = torch.nonzero_static(legal, size=tok["n_legal"]).unbind(1)
        else:
            rows, acts = legal.nonzero(as_tuple=True)
        t = h.shape[1]
        ent = tok["ent"][rows, acts].clamp(max=t - 1)
        tgt = tok["tgt"][rows, acts]
        pe = self.pe(h)
        pt = self.pt(h)
        z = pe[rows, ent] + self.ps(h[:, 0])[rows]
        z = z + torch.where((tgt >= 0).unsqueeze(-1), pt[rows, tgt.clamp(0, t - 1)],
                            torch.zeros_like(z))
        z = z + self.akind(tok["akind"][acts]) + self.asub(tok["asub"][acts].clamp(0, 15))
        if self.plan:
            z = z + self.x_plan.proj(tok["anum"][rows, acts].to(z.dtype))
        logit = self.pout(F.silu(z)).squeeze(-1).float()
        if run and self.run_head is not None and static:
            # RLN44: every pair through both heads, the run rows' from the run head (no sync).
            logit = torch.where(tok["combat"][rows], logit, self.run_head.logits(
                h, tok, rows, acts, ent, tgt))
        elif run and self.run_head is not None:  # RLN42: the run rows' pairs from their own head
            run = (~tok["combat"][rows]).nonzero().squeeze(1)
            if run.numel():
                logit = logit.index_put((run,), self.run_head.logits(
                    h, tok, rows[run], acts[run], ent[run], tgt[run]))
        out = torch.full((b, self.n_actions), float("-inf"), device=h.device)
        out[rows, acts] = logit
        return out

    RUN_HEAD_PREFIX = "run_head."

    def policy(self, tok):
        """The policy's logits and the trunk output; reads no privileged input."""
        h, valid = self.trunk(tok)
        return self.head(h, tok), h, valid

    def value(self, h, valid, ptok, combat):
        """V over [trunk; priv], fp32 (B,)."""
        b = h.shape[0]
        p_ids = self.ids(ptok["ident"]).detach()
        is_move = ptok["kind"] == E.PK["move"]
        p_ids = torch.where(is_move.unsqueeze(-1), self.v_move(ptok["ident"].clamp(0, E.MOVE_BUCKETS - 1))
                            + self.v_move_next(ptok["move_next"]),
                            p_ids)
        p = p_ids + self.v_kind(ptok["kind"]) + self.v_stem(ptok["num"])
        cells = torch.arange(96, device=h.device) * E.ROOM_TYPES
        rooms = self.v_grid(cells + ptok["rooms"]).mean(1)
        nxt = self.v_grid(96 * E.ROOM_TYPES + cells + ptok["next_map"]).mean(1)
        grid = torch.zeros_like(p)
        k = ptok["kind"]
        grid = torch.where((k == E.PK["rooms"]).unsqueeze(-1), rooms.unsqueeze(1), grid)
        grid = torch.where((k == E.PK["next_map"]).unsqueeze(-1), nxt.unsqueeze(1), grid)
        p = p + grid
        x = torch.cat([h, p.to(h.dtype)], 1)
        mask = torch.cat([valid, ptok["valid"]], 1)
        x = self.v_block.forward_first(x, mask) if self.value_first else self.v_block(x, mask)
        s = self.v_norm(x[:, 0])
        v = self.v_out(F.silu(self.v_hidden(s)).float())
        return torch.where(combat, v[:, 0], v[:, 1])

    def forward(self, tok, ptok):
        logits, h, valid = self.policy(tok)
        return logits, self.value(h, valid, ptok, tok["combat"])


def masked_stats(logits):
    """Log-probs, and per-row entropy normalised by log(legal classes) (in [0, 1])."""
    legal = torch.isfinite(logits)
    logp = torch.log_softmax(logits, -1)
    safe = torch.where(legal, logp, torch.zeros_like(logp))  # no -inf in the product's grad
    ent = -(safe.exp() * safe * legal).sum(-1)
    n = legal.sum(-1).clamp(min=2).float()
    return logp, ent / torch.log(n)
