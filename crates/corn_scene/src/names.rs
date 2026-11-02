//! CLAUDE (claude-opus-5-5): component names -> registered types.
//!
//! A name is a full type path (`bevy_pbr::light::ambient_light::AmbientLight`), an abridged one
//! (`crate::Type`, or `crate::module::Type` where the modules are a suffix of the real module path;
//! `bevy::` stands for any `bevy_*` crate), or a bare `Type`. Bare names are looked up in the file's
//! `use` lines, then in each crate of the precedence list in order. Two candidates in the same crate
//! warn and pick the shorter path, then the alphabetically first.
//!
//! The precedence list lives in a file (one crate per line, `#` comments). When it doesn't exist it is
//! written with the default order: bevy crates, then all others, each alphabetical. Registered crates
//! missing from the file are appended in that order.

use std::any::TypeId;
use std::collections::HashMap;

use bevy::reflect::TypeRegistry;

pub struct TypeEntry {
    pub type_id: TypeId,
    pub full: String,
    pub crate_name: String,
    /// module path between the crate and the type
    pub modules: Vec<String>,
}

/// registered types by their short name (`Type`, `Handle<Image>`)
#[derive(Default)]
pub struct TypeIndex {
    by_short: HashMap<String, Vec<TypeEntry>>,
    by_full: HashMap<String, TypeId>,
}

impl TypeIndex {
    pub fn new(registry: &TypeRegistry) -> Self {
        let mut index = TypeIndex::default();
        for registration in registry.iter() {
            let table = registration.type_info().type_path_table();
            let full = table.path().to_string();
            let segments = split_path(&full);
            let Some((_, modules)) = segments.split_last() else { continue };
            let crate_name = table.crate_name().map_or_else(|| segments[0].to_string(), str::to_string);
            let modules = modules.iter().skip(1).map(|s| s.to_string()).collect();
            index.by_full.insert(full.clone(), registration.type_id());
            index.by_short.entry(table.short_path().to_string()).or_default().push(TypeEntry {
                type_id: registration.type_id(),
                full,
                crate_name,
                modules,
            });
        }
        index
    }

    pub fn crates(&self) -> impl Iterator<Item = &str> {
        let mut crates: Vec<&str> =
            self.by_short.values().flatten().map(|e| e.crate_name.as_str()).collect();
        crates.sort();
        crates.dedup();
        crates.into_iter()
    }
}

/// splits at `::` outside of `<>`
pub fn split_path(path: &str) -> Vec<&str> {
    let mut out = vec![];
    let (mut depth, mut start) = (0, 0);
    let bytes = path.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'<' => depth += 1,
            b'>' => depth -= 1,
            b':' if depth == 0 && bytes.get(i + 1) == Some(&b':') => {
                out.push(&path[start..i]);
                i += 2;
                start = i;
                continue;
            }
            _ => {}
        }
        i += 1;
    }
    out.push(&path[start..]);
    out
}

fn is_bevy(crate_name: &str) -> bool {
    crate_name == "bevy" || crate_name.starts_with("bevy_")
}

fn crate_matches(written: &str, actual: &str) -> bool {
    written == actual || (written == "bevy" && is_bevy(actual))
}

/// the default precedence for these crates: bevy crates, then the rest, each alphabetical
pub fn default_precedence<'a>(crates: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut crates: Vec<&str> = crates.into_iter().collect();
    crates.sort_by_key(|c| (!is_bevy(c), *c));
    crates.dedup();
    crates.into_iter().map(str::to_string).collect()
}

/// reads a precedence file's text, appending crates it doesn't mention in default order
pub fn parse_precedence<'a>(text: &str, crates: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut list: Vec<String> = text
        .lines()
        .map(|l| l.split('#').next().unwrap().trim())
        .filter(|l| !l.is_empty())
        .map(str::to_string)
        .collect();
    let missing: Vec<&str> = crates.into_iter().filter(|c| !list.iter().any(|l| l == c)).collect();
    list.extend(default_precedence(missing));
    list
}

pub fn precedence_file_text(list: &[String]) -> String {
    let mut out = String::from(
        "# crate precedence for bare component names in scene files: the first crate with a match wins.\n\
         # registered crates missing here are appended in the default order (bevy crates, then the rest).\n",
    );
    for c in list {
        out.push_str(c);
        out.push('\n');
    }
    out
}

/// the name lookup of one file: the global index and precedence plus the file's `use` lines
pub struct Resolver<'a> {
    pub index: &'a TypeIndex,
    pub precedence: &'a [String],
    /// `use crate;` lines, first in the file first
    pub use_crates: Vec<String>,
    /// `use path::Type;` lines
    pub use_names: HashMap<String, TypeId>,
    pub no_default_use: bool,
}

#[derive(Debug)]
pub struct Resolved {
    pub type_id: TypeId,
    pub warning: Option<String>,
}

impl<'a> Resolver<'a> {
    pub fn new(index: &'a TypeIndex, precedence: &'a [String]) -> Self {
        Resolver { index, precedence, use_crates: vec![], use_names: HashMap::new(), no_default_use: false }
    }

    /// `use` line contents: a crate (`avian3d`) or a type path (`avian3d::RigidBody`)
    pub fn add_use(&mut self, path: &str) -> Result<Option<String>, String> {
        let segments = split_path(path);
        if segments.len() == 1 {
            if !self.index.crates().any(|c| crate_matches(path, c)) {
                return Err(format!("`use {path}`: no registered types from crate `{path}`"));
            }
            self.use_crates.push(path.to_string());
            return Ok(None);
        }
        let resolved = self.resolve_path(&segments)?;
        self.use_names.insert(segments.last().unwrap().to_string(), resolved.type_id);
        Ok(resolved.warning)
    }

    pub fn resolve(&self, name: &str) -> Result<Resolved, String> {
        let segments = split_path(name);
        if segments.len() > 1 {
            return self.resolve_path(&segments);
        }
        if let Some(&type_id) = self.use_names.get(name) {
            return Ok(Resolved { type_id, warning: None });
        }
        let candidates = self.index.by_short.get(name).map(Vec::as_slice).unwrap_or_default();
        let mut crates: Vec<&str> = self.use_crates.iter().map(String::as_str).collect();
        if !self.no_default_use {
            crates.extend(self.precedence.iter().map(String::as_str));
        }
        for c in crates {
            let in_crate: Vec<&TypeEntry> = candidates.iter().filter(|e| crate_matches(c, &e.crate_name)).collect();
            if !in_crate.is_empty() {
                return Ok(pick(name, in_crate));
            }
        }
        Err(match (candidates.first(), self.no_default_use) {
            (Some(e), true) => format!("`{name}` is not imported (nodefaultuse): `use {};`", e.full),
            (Some(e), false) => format!("`{name}` is only in `{}`, which isn't in the precedence list", e.crate_name),
            (None, _) => format!("unknown type `{name}`{}", self.did_you_mean(name)),
        })
    }

    fn resolve_path(&self, segments: &[&str]) -> Result<Resolved, String> {
        let full = segments.join("::");
        if let Some(&type_id) = self.index.by_full.get(&full) {
            return Ok(Resolved { type_id, warning: None });
        }
        let (short, rest) = segments.split_last().unwrap();
        let (crate_name, modules) = rest.split_first().unwrap();
        let candidates: Vec<&TypeEntry> = self
            .index
            .by_short
            .get(*short)
            .into_iter()
            .flatten()
            .filter(|e| crate_matches(crate_name, &e.crate_name) && e.modules.ends_with(&modules.iter().map(|m| m.to_string()).collect::<Vec<_>>()))
            .collect();
        if candidates.is_empty() {
            return Err(format!("unknown type `{full}`{}", self.did_you_mean(short)));
        }
        // `bevy::X` may match several bevy crates: the precedence list decides between them
        let best_crate = self
            .precedence
            .iter()
            .find(|c| candidates.iter().any(|e| &e.crate_name == *c))
            .map_or(candidates[0].crate_name.as_str(), String::as_str);
        Ok(pick(&full, candidates.into_iter().filter(|e| e.crate_name == best_crate).collect()))
    }

    fn did_you_mean(&self, short: &str) -> String {
        let lower = short.to_lowercase();
        let mut close: Vec<&str> = self
            .index
            .by_short
            .iter()
            .filter(|(k, _)| k.to_lowercase() == lower)
            .flat_map(|(_, v)| v.iter().map(|e| e.full.as_str()))
            .collect();
        close.sort();
        match close.first() {
            Some(c) => format!(" (did you mean `{c}`?)"),
            None => String::new(),
        }
    }
}

/// one crate's candidates: the shortest path, then alphabetical; more than one warns
fn pick(name: &str, mut in_crate: Vec<&TypeEntry>) -> Resolved {
    in_crate.sort_by(|a, b| a.modules.len().cmp(&b.modules.len()).then_with(|| a.full.cmp(&b.full)));
    let chosen = in_crate[0];
    let warning = (in_crate.len() > 1).then(|| {
        let others: Vec<&str> = in_crate[1..].iter().map(|e| e.full.as_str()).collect();
        format!("`{name}` is ambiguous in `{}`: using `{}`, not {}", chosen.crate_name, chosen.full, others.join(", "))
    });
    Resolved { type_id: chosen.type_id, warning }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// a distinct type id per test entry
    fn fake_id(i: usize) -> TypeId {
        [TypeId::of::<[u8; 0]>(), TypeId::of::<[u8; 1]>(), TypeId::of::<[u8; 2]>(), TypeId::of::<[u8; 3]>(),
         TypeId::of::<[u8; 4]>(), TypeId::of::<[u8; 5]>(), TypeId::of::<[u8; 6]>(), TypeId::of::<[u8; 7]>()][i]
    }

    fn index(paths: &[&str]) -> TypeIndex {
        let mut index = TypeIndex::default();
        for (i, full) in paths.iter().enumerate() {
            let segments = split_path(full);
            let entry = TypeEntry {
                type_id: fake_id(i),
                full: full.to_string(),
                crate_name: segments[0].into(),
                modules: segments[1..segments.len() - 1].iter().map(|s| s.to_string()).collect(),
            };
            index.by_full.insert(full.to_string(), entry.type_id);
            index.by_short.entry(segments.last().unwrap().to_string()).or_default().push(entry);
        }
        index
    }

    /// the full path a name resolves to, and whether it warned
    fn chosen<'a>(index: &'a TypeIndex, r: &Resolver, name: &str) -> (&'a str, bool) {
        let res = r.resolve(name).unwrap();
        let full = index.by_full.iter().find(|(_, id)| **id == res.type_id).unwrap().0;
        (full, res.warning.is_some())
    }
    #[test]
    fn precedence_and_uses() {
        let index = index(&[
            "bevy_pbr::light::ambient_light::AmbientLight",
            "corn_game::ecs::ambient_light::AmbientLight",
            "corn_game::ecs::death::Dead",
            "corn_game::ecs::npc::Dead",
            "corn_game::Dead2",
            "avian3d::dynamics::RigidBody",
        ]);
        let precedence = default_precedence(index.crates());
        assert_eq!(precedence, ["bevy_pbr", "avian3d", "corn_game"]);

        let mut r = Resolver::new(&index, &precedence);
        assert_eq!(chosen(&index, &r, "AmbientLight"), ("bevy_pbr::light::ambient_light::AmbientLight", false));
        assert_eq!(chosen(&index, &r, "corn_game::AmbientLight"), ("corn_game::ecs::ambient_light::AmbientLight", false));
        assert_eq!(chosen(&index, &r, "bevy::AmbientLight"), ("bevy_pbr::light::ambient_light::AmbientLight", false));
        assert_eq!(chosen(&index, &r, "Dead"), ("corn_game::ecs::death::Dead", true));
        assert!(r.resolve("corn_game::npc::Dead").unwrap().warning.is_none());
        assert!(r.resolve("corn_game::other::Dead").is_err());

        r.add_use("corn_game").unwrap();
        assert_eq!(chosen(&index, &r, "AmbientLight"), ("corn_game::ecs::ambient_light::AmbientLight", false));

        let mut strict = Resolver::new(&index, &precedence);
        strict.no_default_use = true;
        assert!(strict.resolve("RigidBody").unwrap_err().contains("use avian3d::dynamics::RigidBody"));
        strict.add_use("avian3d::RigidBody").unwrap();
        assert!(strict.resolve("RigidBody").is_ok());
        assert!(strict.resolve("bevy_pbr::AmbientLight").is_ok());
    }

    #[test]
    fn precedence_file() {
        let list = parse_precedence("# mine\ncorn_game\n\nbevy_pbr # light\n", ["avian3d", "bevy_ui", "corn_game", "bevy_pbr"]);
        assert_eq!(list, ["corn_game", "bevy_pbr", "bevy_ui", "avian3d"]);
        assert_eq!(parse_precedence(&precedence_file_text(&list), ["avian3d"]), list);
    }

    #[test]
    fn generic_paths() {
        assert_eq!(split_path("a::B<c::D, e::F>"), ["a", "B<c::D, e::F>"]);
    }
}
