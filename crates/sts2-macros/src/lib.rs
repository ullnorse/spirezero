//! Proc macros for `sts2-core`'s hook dispatch.
//!
//! - `#[hook_ids]` on the `Hooks` trait adds `enum Hook`, one variant per trait method (named
//!   exactly like the method), so a hook can be named as data.
//! - `#[hooks]` on an `impl Hooks for X` block adds `X::HOOKS`, the `HookSet` of the methods
//!   the block overrides, so dispatch can skip models whose method is the default.
//!
//! Both only look at the `fn <name>` items at the top level of the block, so they need no
//! parser crate.

use std::fmt::Write as _;

use proc_macro::{Delimiter, Ident, TokenStream, TokenTree};

/// The names of the `fn` items at the top level of `item`'s last brace-delimited block.
fn fn_names(item: &TokenStream) -> Vec<Ident> {
    let body = item
        .clone()
        .into_iter()
        .filter_map(|t| match t {
            TokenTree::Group(g) if g.delimiter() == Delimiter::Brace => Some(g),
            _ => None,
        })
        .last()
        .expect("an item with a `{ ... }` body");
    let tokens: Vec<TokenTree> = body.stream().into_iter().collect();
    tokens
        .windows(2)
        .filter_map(|w| match w {
            [TokenTree::Ident(kw), TokenTree::Ident(name)] if kw.to_string() == "fn" => {
                Some(name.clone())
            }
            _ => None,
        })
        .collect()
}

fn parse(code: &str) -> TokenStream {
    code.parse().expect("generated code parses")
}

/// On `trait Hooks`: also emits `pub enum Hook` with one variant per method and `Hook::COUNT`.
///
/// # Panics
/// If the item has no `{ ... }` body.
#[proc_macro_attribute]
pub fn hook_ids(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let names = fn_names(&item);
    let variants: Vec<String> = names.iter().map(ToString::to_string).collect();
    let code = format!(
        "/// A [`Hooks`] method, by name.\n\
         #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]\n\
         #[allow(non_camel_case_types, reason = \"named exactly like the `Hooks` methods\")]\n\
         #[allow(dead_code, reason = \"not every hook has a dispatcher yet\")]\n\
         pub enum Hook {{ {} }}\n\
         impl Hook {{\n\
             /// The number of hooks.\n\
             pub const COUNT: usize = {};\n\
             /// Every hook, in declaration order ([`Hook::index`] order).\n\
             #[allow(dead_code, reason = \"for tests and checks\")]\n\
             pub const ALL: [Self; {1}] = [{}];\n\
         }}",
        variants.join(", "),
        variants.len(),
        variants
            .iter()
            .map(|v| format!("Self::{v}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let mut out = item;
    out.extend(parse(&code));
    out
}

/// On `impl Hooks for X`: adds `X::HOOKS`, the set of the overridden methods.
///
/// # Panics
/// If the item is not `impl Hooks for X { ... }`.
#[proc_macro_attribute]
pub fn hooks(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let names = fn_names(&item);
    let mut set = String::new();
    for name in &names {
        write!(set, ".with(crate::combat::hooks::Hook::{name})").expect("writing to a String");
    }
    // The type: the tokens between `for` and the body.
    let tokens: Vec<TokenTree> = item.clone().into_iter().collect();
    let start = tokens
        .iter()
        .position(|t| matches!(t, TokenTree::Ident(i) if i.to_string() == "for"))
        .expect("`impl Hooks for X`")
        + 1;
    let ty: TokenStream = tokens[start..tokens.len() - 1].iter().cloned().collect();
    let mut out = item;
    out.extend(parse(&format!(
        "impl {ty} {{ \
             #[doc = \"The hooks the model overrides.\"] \
             pub const HOOKS: crate::combat::hooks::HookSet = \
                 crate::combat::hooks::HookSet::EMPTY{set}; \
         }}"
    )));
    out
}
