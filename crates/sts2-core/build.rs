//! Generates each content kind's registry from its directory, so adding an entity is adding one
//! file (`docs/specs/content-api.md`, "Registration").
//!
//! For every kind in [`KINDS`], each `src/content/<dir>/<stem>.rs` is an entity: the unit or
//! data struct `pub struct <Stem in CamelCase>`. The generated `$OUT_DIR/content/<dir>.rs`, which
//! the kind's `mod.rs` includes, holds the module declarations, the kind enum (one variant per
//! entity, named and documented like its struct), `ALL`, `id`, `from_id` and `model`.
//!
//! Exceptions, marked in the source:
//! - a module `mod.rs` declares by hand (`mod pools;`) is a shared module, not an entity;
//! - a file with the inner attribute `#![cfg(test)]` is a test-only entity: its variant exists
//!   only under `cfg(test)` and it is left out of `ALL` and `from_id`;
//! - a file with `const ID: &str = "...";` overrides the derived id (`ModelId.Entry`: the struct
//!   name in screaming snake case, plus the kind's suffix).

use std::path::{Path, PathBuf};
use std::{env, fs};

/// A content kind's registry.
struct Kind {
    /// The directory under `src/content`.
    dir: &'static str,
    /// The kind enum.
    name: &'static str,
    /// The enum's doc comment.
    doc: &'static str,
    /// The model trait `model` returns.
    model: &'static str,
    /// The visibility of the entity modules.
    vis: &'static str,
    /// Appended to the derived id (C# `StrengthPower` is `STRENGTH_POWER`).
    id_suffix: &'static str,
    /// Each variant holds its struct (the model's per-combat fields) instead of being a unit;
    /// the structs are re-exported and there is no `ALL`.
    data: bool,
    /// The models are combat listeners: generate `hooks`, a table of each model's `HOOKS`.
    hooks: bool,
    /// Generate `id_rank`, each id's rank in ordinal order, so that sorting by id compares
    /// integers (`CardModel.CompareTo`). Every id must be derived, not an `ID` const.
    id_rank: bool,
}

const KINDS: &[Kind] = &[
    Kind {
        dir: "afflictions",
        name: "AfflictionKind",
        doc: "An affliction model.",
        model: "AfflictionModel",
        vis: "pub(crate) ",
        id_suffix: "",
        data: false,
        hooks: true,
        id_rank: false,
    },
    Kind {
        dir: "ancients",
        name: "AncientKind",
        doc: "An ancient (`AncientEventModel`).",
        model: "AncientModel",
        vis: "pub(crate) ",
        id_suffix: "",
        data: false,
        hooks: false,
        id_rank: false,
    },
    Kind {
        dir: "cards",
        name: "CardKind",
        doc: "A card model.",
        model: "CardModel",
        vis: "pub(crate) ",
        id_suffix: "",
        data: false,
        hooks: true,
        id_rank: true,
    },
    Kind {
        dir: "enchantments",
        name: "EnchantmentKind",
        doc: "An enchantment model.",
        model: "EnchantmentModel",
        vis: "pub(crate) ",
        id_suffix: "",
        data: false,
        hooks: true,
        id_rank: false,
    },
    Kind {
        dir: "encounters",
        name: "Encounter",
        doc: "The encounters the engine can set up.",
        model: "EncounterModel",
        vis: "",
        id_suffix: "",
        data: false,
        hooks: false,
        id_rank: false,
    },
    Kind {
        dir: "events",
        name: "EventKind",
        doc: "An event model.",
        model: "EventModel",
        vis: "pub(crate) ",
        id_suffix: "",
        data: false,
        hooks: false,
        id_rank: false,
    },
    Kind {
        dir: "monsters",
        name: "MonsterKind",
        doc: "A monster model with its per-combat fields.",
        model: "MonsterModel",
        vis: "",
        id_suffix: "",
        data: true,
        hooks: true,
        id_rank: false,
    },
    Kind {
        dir: "potions",
        name: "PotionKind",
        doc: "A potion model.",
        model: "PotionModel",
        vis: "pub(crate) ",
        id_suffix: "",
        data: false,
        hooks: true,
        id_rank: false,
    },
    Kind {
        dir: "powers",
        name: "PowerKind",
        doc: "A power model.",
        model: "PowerModel",
        vis: "pub(crate) ",
        id_suffix: "_POWER",
        data: false,
        hooks: true,
        id_rank: false,
    },
    Kind {
        dir: "relics",
        name: "RelicKind",
        doc: "A relic model.",
        model: "RelicModel",
        vis: "pub(crate) ",
        id_suffix: "",
        data: false,
        hooks: true,
        id_rank: false,
    },
];

/// One entity file.
struct Entity {
    /// The module (the file stem).
    module: String,
    /// The struct and variant name.
    name: String,
    /// The struct's `///` lines, else the file's first `//!` paragraph as `///` lines.
    doc: Vec<String>,
    /// The file has `#![cfg(test)]`.
    test_only: bool,
    /// The id expression: a string literal, or the file's `ID` const.
    id: String,
    path: PathBuf,
}

fn main() {
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("set by cargo"));
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("set by cargo")).join("content");
    fs::create_dir_all(&out).expect("create OUT_DIR/content");
    let content = manifest.join("src/content");
    println!("cargo::rerun-if-changed={}", content.display());
    for kind in KINDS {
        let dir = content.join(kind.dir);
        let code = generate(kind, &entities(kind, &dir));
        let path = out.join(format!("{}.rs", kind.dir));
        if fs::read_to_string(&path).ok().as_deref() != Some(code.as_str()) {
            fs::write(&path, code).expect("write the generated registry");
        }
    }
}

/// The entities in `dir`, sorted by module name.
fn entities(kind: &Kind, dir: &Path) -> Vec<Entity> {
    let mod_rs = read(&dir.join("mod.rs"));
    let shared: Vec<&str> = mod_rs.lines().filter_map(declared_module).collect();
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .expect("read a content directory")
        .map(|entry| entry.expect("read a directory entry").path())
        .filter(|path| path.extension().is_some_and(|e| e == "rs"))
        .collect();
    files.sort();
    files
        .into_iter()
        .filter_map(|path| {
            let module = path.file_stem()?.to_str()?.to_owned();
            (module != "mod" && !shared.contains(&module.as_str()))
                .then(|| entity(kind, module, path))
        })
        .collect()
}

/// The module a `mod.rs` line declares by hand (`pub(crate) mod pools;`), if any.
fn declared_module(line: &str) -> Option<&str> {
    let line = line.trim();
    let rest = ["pub(crate) ", "pub(super) ", "pub ", ""]
        .into_iter()
        .find_map(|vis| line.strip_prefix(vis)?.strip_prefix("mod "))?;
    rest.strip_suffix(';')
}

fn entity(kind: &Kind, module: String, path: PathBuf) -> Entity {
    let source = read(&path);
    let name = camel_case(&module);
    let lines: Vec<&str> = source.lines().collect();
    let Some(at) = lines.iter().position(|l| declares_struct(l, &name)) else {
        panic!(
            "{}: an entity file must declare `pub struct {name}`; declare a shared module by \
             hand in {}/mod.rs instead",
            path.display(),
            kind.dir,
        );
    };
    let mut doc: Vec<String> = lines[..at]
        .iter()
        .rev()
        .map(|l| l.trim())
        .skip_while(|l| l.starts_with("#["))
        .take_while(|l| l.starts_with("///"))
        .map(str::to_owned)
        .collect();
    doc.reverse();
    if doc.is_empty() {
        // The variant's doc: the struct's, else the file's first `//!` paragraph, its links as
        // plain code (they resolve in the entity's module, not next to the enum).
        doc = lines
            .iter()
            .map(|l| l.trim())
            .take_while(|l| l.starts_with("//!") && *l != "//!")
            .map(|l| format!("///{}", unlink(&l[3..])))
            .collect();
    }
    let id = if lines.iter().any(|l| declares_id(l)) {
        format!("{module}::ID")
    } else {
        format!("\"{}{}\"", screaming_snake_case(&name), kind.id_suffix)
    };
    Entity {
        test_only: lines.iter().any(|l| l.trim() == "#![cfg(test)]"),
        module,
        name,
        doc,
        id,
        path,
    }
}

/// `text` with each intra-doc link ``[`x`]`` or ``[`x`](path)`` reduced to `` `x` ``.
fn unlink(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("[`") {
        let Some(len) = rest[start + 2..].find("`]") else {
            break;
        };
        let end = start + 2 + len;
        out.push_str(&rest[..start]);
        out.push_str(&rest[start + 1..=end]);
        rest = &rest[end + 2..];
        if rest.starts_with('(')
            && let Some(close) = rest.find(')')
        {
            rest = &rest[close + 1..];
        }
    }
    out.push_str(rest);
    out
}

fn declares_struct(line: &str, name: &str) -> bool {
    line.strip_prefix("pub struct ")
        .and_then(|rest| rest.strip_prefix(name))
        .is_some_and(|rest| rest.starts_with([';', ' ', '{', '(', '<']))
}

fn declares_id(line: &str) -> bool {
    let line = line.trim_start();
    ["pub(crate) ", "pub(super) ", "pub ", ""]
        .into_iter()
        .any(|vis| line.starts_with(&format!("{vis}const ID: &str")))
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// `right_hand_hand` -> `RightHandHand`.
fn camel_case(snake: &str) -> String {
    snake
        .split('_')
        .flat_map(|word| {
            let mut chars = word.chars();
            chars
                .next()
                .map(|c| c.to_ascii_uppercase())
                .into_iter()
                .chain(chars)
        })
        .collect()
}

/// `ModelId.Entry` of a class name: `LeafSlimeM` -> `LEAF_SLIME_M`, `TheKinBoss` ->
/// `THE_KIN_BOSS`. A word starts at an uppercase letter after a lowercase letter or digit, or
/// before a lowercase letter after an uppercase one.
fn screaming_snake_case(camel: &str) -> String {
    let chars: Vec<char> = camel.chars().collect();
    let mut out = String::new();
    for (i, &c) in chars.iter().enumerate() {
        if i > 0 && c.is_ascii_uppercase() {
            let prev = chars[i - 1];
            let next_lower = chars.get(i + 1).is_some_and(char::is_ascii_lowercase);
            if !prev.is_ascii_uppercase() || next_lower {
                out.push('_');
            }
        }
        out.push(c.to_ascii_uppercase());
    }
    out
}

/// Generated source, one line at a time.
#[derive(Default)]
struct Code(String);

impl Code {
    /// `ALL` (`test_only` false) or `TEST_ONLY`: the models in file-name order.
    fn model_list(&mut self, entities: &[Entity], test_only: bool) {
        let list: Vec<&Entity> = entities
            .iter()
            .filter(|e| e.test_only == test_only)
            .collect();
        if test_only {
            self.line("    /// The test-only models, by file name.");
            self.line("    #[cfg(test)]");
            self.line(
                "    #[allow(clippy::allow_attributes, dead_code, reason = \"generated for every kind; not every kind's tests use it\")]",
            );
        } else {
            self.line("    /// Every model, by file name (test-only ones excluded).");
        }
        let name = if test_only { "TEST_ONLY" } else { "ALL" };
        self.line(format!("    pub const {name}: [Self; {}] = [", list.len()));
        for e in list {
            self.line(format!("        Self::{},", e.name));
        }
        self.line("    ];");
        self.line("");
    }

    /// The model lists: `ALL`, `TEST_ONLY` and the id lookup, or for a kind whose variants hold
    /// data, `IDS` and `ordinal`.
    fn lists(&mut self, kind: &Kind, entities: &[Entity]) {
        if kind.data {
            self.ordinals(entities);
        } else {
            self.model_list(entities, false);
            self.model_list(entities, true);
            self.id_lookup(kind.name);
            self.line("");
        }
    }

    /// For a kind whose variants hold data (no `ALL`): `IDS`, every model's id in file-name
    /// order (test-only ones excluded), and `ordinal`, a model's index there (test-only ones
    /// after the others).
    fn ordinals(&mut self, entities: &[Entity]) {
        let listed = entities.iter().filter(|e| !e.test_only).count();
        self.line(
            "    /// Every model's id, by file name (test-only ones excluded); [`Self::ordinal`] indexes it.",
        );
        self.line(format!("    pub const IDS: [&'static str; {listed}] = ["));
        for e in entities.iter().filter(|e| !e.test_only) {
            self.line(format!("        {},", e.id));
        }
        self.line("    ];");
        self.line("");
        self.line(
            "    /// The model's index in [`Self::IDS`] (test-only models after the others).",
        );
        self.line("    #[must_use]");
        self.line("    pub const fn ordinal(&self) -> usize {");
        self.line("        match self {");
        let mut next = 0;
        let mut next_test = listed;
        for e in entities {
            let ordinal = if e.test_only {
                next_test += 1;
                next_test - 1
            } else {
                next += 1;
                next - 1
            };
            if e.test_only {
                self.line("            #[cfg(test)]");
            }
            self.line(format!("            Self::{}(_) => {ordinal},", e.name));
        }
        self.line("        }");
        self.line("    }");
        self.line("");
    }

    /// `hooks`: each model's `HOOKS` (`#[sts2_macros::hooks]`), from a table indexed by the
    /// variant, or a match for a kind whose variants hold data.
    fn hooks(&mut self, kind: &Kind, entities: &[Entity]) {
        const SET: &str = "crate::combat::hooks::HookSet";
        self.line("");
        self.line("    /// The hooks the model overrides: its `HOOKS` (`#[sts2_macros::hooks]`).");
        self.line("    #[must_use]");
        if kind.data {
            // Inline, so that a walk reads the table without a call.
            self.line("    #[inline]");
            self.line(format!("    pub(crate) const fn hooks(&self) -> {SET} {{"));
            self.line("        match self {");
            self.per_entity(entities, "            ", |e| {
                format!("Self::{0}(_) => {0}::HOOKS,", e.name)
            });
            self.line("        }");
        } else {
            self.line(format!("    pub(crate) const fn hooks(self) -> {SET} {{"));
            self.line("        *self.hooks_ref()");
            self.line("    }");
            self.line("");
            self.line("    /// [`Self::hooks`] in place, for a walk that reads one word of it.");
            self.line("    #[must_use]");
            self.line("    #[inline]");
            self.line(format!(
                "    pub(crate) const fn hooks_ref(self) -> &'static {SET} {{"
            ));
            self.line(format!("        const HOOKS: &[{SET}] = &["));
            self.per_entity(entities, "            ", |e| {
                format!("{}::{}::HOOKS,", e.module, e.name)
            });
            self.line("        ];");
            self.line("        &HOOKS[self.index()]");
        }
        self.line("    }");
    }

    /// `id_rank`: the rank of each model's id in ordinal (byte) order among all models, test-only
    /// ones included, from a table indexed by the variant.
    fn id_rank(&mut self, entities: &[Entity]) {
        let mut ids: Vec<&str> = entities
            .iter()
            .map(|e| {
                e.id.strip_prefix('"')
                    .and_then(|id| id.strip_suffix('"'))
                    .unwrap_or_else(|| panic!("{}: `id_rank` needs a derived id", e.module))
            })
            .collect();
        ids.sort_unstable();
        self.line("");
        self.line(
            "    /// The rank of [`Self::id`] in ordinal order: comparing ranks is comparing ids.",
        );
        self.line("    #[must_use]");
        self.line("    pub(crate) const fn id_rank(self) -> u16 {");
        self.line("        const RANKS: &[u16] = &[");
        self.per_entity(entities, "            ", |e| {
            let rank = ids
                .binary_search(&&e.id[1..e.id.len() - 1])
                .expect("every id is in the list");
            format!("{rank},")
        });
        self.line("        ];");
        self.line("        RANKS[self.index()]");
        self.line("    }");
    }

    /// `with_model!(kind, m => body)`: `body` with `m` bound to the model of `kind` as its own
    /// type (for a kind whose variants hold data, a borrow of the variant's struct), so calls on it are static (and inlined: a method every model leaves at its default
    /// folds to a constant or a table) instead of a `dyn` call through [`Self::model`].
    fn static_model(&mut self, kind: &Kind, entities: &[Entity]) {
        self.line("");
        self.line(format!(
            "/// `$body` with `$m` bound to the model of `$kind` (a `{}`), statically dispatched.",
            kind.name
        ));
        self.line("#[allow(clippy::allow_attributes, unused_macros, reason = \"generated for every kind; not every kind uses it\")]");
        self.line("macro_rules! with_model {");
        self.line("    ($kind:expr, $m:ident => $body:expr) => {");
        if kind.data {
            // `$m` borrows the variant's struct.
            self.line("        match &$kind {");
            self.per_entity(entities, "            ", |e| {
                format!(
                    "$crate::content::{dir}::{kind}::{name}($m) => $body,",
                    dir = kind.dir,
                    kind = kind.name,
                    name = e.name,
                )
            });
        } else {
            self.line("        match $kind {");
            self.per_entity(entities, "            ", |e| {
                format!(
                    "$crate::content::{dir}::{kind}::{name} => {{ let $m = &$crate::content::{dir}::{module}::{name}; $body }}",
                    dir = kind.dir,
                    kind = kind.name,
                    name = e.name,
                    module = e.module,
                )
            });
        }
        self.line("        }");
        self.line("    };");
        self.line("}");
    }

    /// `from_id`: a hash table of the ids (`content::IdTable`), built on first use.
    fn id_lookup(&mut self, name: &str) {
        self.line(format!(
            r"    /// The model with `ModelId.Entry` `id`, if ported (test-only ones excluded), from a
    /// hash table built on first use.
    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {{
        static BY_ID: std::sync::OnceLock<crate::content::IdTable<{name}>> =
            std::sync::OnceLock::new();
        BY_ID
            .get_or_init(|| crate::content::IdTable::new(Self::ALL.into_iter().map(|k| (k.id(), k))))
            .get(id)
    }}
"
        ));
    }

    fn line(&mut self, line: impl AsRef<str>) {
        self.0.push_str(line.as_ref());
        self.0.push('\n');
    }

    /// One line per entity, each under `#[cfg(test)]` if the entity is test-only.
    fn per_entity(&mut self, entities: &[Entity], indent: &str, line: impl Fn(&Entity) -> String) {
        for e in entities {
            if e.test_only {
                self.line(format!("{indent}#[cfg(test)]"));
            }
            self.line(format!("{indent}{}", line(e)));
        }
    }
}

/// The registry source of `kind`.
fn generate(kind: &Kind, entities: &[Entity]) -> String {
    let Kind { name, model, .. } = kind;
    let mut c = Code::default();
    c.line(format!(
        "// Generated by build.rs from src/content/{}/.",
        kind.dir
    ));
    c.line("");
    for e in entities {
        c.line(format!("#[path = {:?}]", e.path.display().to_string()));
        c.line(format!("{}mod {};", kind.vis, e.module));
    }
    if kind.data {
        c.line("");
        for e in entities {
            c.line(format!("pub use {}::{};", e.module, e.name));
        }
    }

    c.line("");
    c.line(format!("/// {}", kind.doc));
    c.line("#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]");
    c.line(format!("pub enum {name} {{"));
    for e in entities {
        for doc in &e.doc {
            c.line(format!("    {doc}"));
        }
        if e.test_only {
            c.line("    #[cfg(test)]");
        }
        if kind.data {
            c.line(format!("    {0}({0}),", e.name));
        } else {
            c.line(format!("    {},", e.name));
        }
    }
    c.line("}");
    c.line("");
    if !kind.data {
        c.line(format!("impl_index!({name});"));
        c.line("");
    }

    c.line("#[allow(");
    c.line("    clippy::allow_attributes,");
    c.line("    clippy::too_many_lines,");
    c.line(
        "    reason = \"generated: one arm per entity file (too long only for the larger kinds)\"",
    );
    c.line(")]");
    c.line(format!("impl {name} {{"));
    c.lists(kind, entities);

    c.line("    /// `ModelId.Entry`, which is also the referee's id.");
    c.line("    #[must_use]");
    if kind.data {
        c.line("    pub const fn id(&self) -> &'static str {");
        c.line("        match self {");
        c.per_entity(entities, "            ", |e| {
            format!("Self::{}(_) => {},", e.name, e.id)
        });
    } else {
        c.line("    pub const fn id(self) -> &'static str {");
        c.line("        match self {");
        c.per_entity(entities, "            ", |e| {
            format!("Self::{} => {},", e.name, e.id)
        });
    }
    c.line("        }");
    c.line("    }");
    c.line("");

    c.line("    /// The model: its static data and hook table.");
    if kind.data {
        c.line(format!("    pub(crate) fn model(&self) -> &dyn {model} {{"));
        c.line("        match self {");
        c.per_entity(entities, "            ", |e| {
            format!("Self::{}(m) => m,", e.name)
        });
    } else {
        c.line(format!(
            "    pub(crate) fn model(self) -> &'static dyn {model} {{"
        ));
        c.line("        match self {");
        c.per_entity(entities, "            ", |e| {
            format!("Self::{0} => &{1}::{0},", e.name, e.module)
        });
    }
    c.line("        }");
    c.line("    }");
    if kind.hooks {
        c.hooks(kind, entities);
    }
    if kind.id_rank {
        c.id_rank(entities);
    }
    c.line("}");
    c.static_model(kind, entities);
    c.0
}
