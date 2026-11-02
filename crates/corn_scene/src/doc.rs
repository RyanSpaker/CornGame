//! CLAUDE (claude-opus-5-5): the parsed form of a scene file and its parser. Component names are
//! resolved while parsing (`use` lines come first), values stay untyped until applied.
//!
//! In a body, an item is:
//! - `use crate;` / `use path::Type;` / `use path::{A, B};` / `nodefaultuse;` (top of the file only)
//! - `spawn name { .. }`: a child entity named `name`. a top-level one is also its own scene, `file#name`
//! - `include "path";` or `include "path" { .. }`: the scene's root becomes this entity, then the body applies.
//!   `include "path" as child { .. }` keeps each root a child of this entity, the body applying to it.
//!   a `.corn` file (`file.corn`, or `file.corn#name` for `spawn name`) pastes its items here, other scenes
//!   (gltf) are copied in
//! - `class: a b;` gives this entity classes. `.a { .. }` is an ordinary rule: any number, in any file
//! - `Comp: value;` / `Comp.field.sub: value;`: a component patch. a space after the `:` is required,
//!   which tells it from a pseudo-class (`Button:hover`)
//! - `selector, selector { .. }`: a rule over this entity and its descendants
//! - `script { .. }` (top level only): the file's script module, raw JS. its top-level code runs once,
//!   when the file loads, before the file's items apply
//! - `on Event: handler;` or `on Event: ${ .. }`: a script listener on this entity, once spawned
//! - `asset name: Type value` (top level only): a new asset, `Type::default()` (or its recipe) patched
//!   with the value. handle fields take it as `name`, other files as `"file.corn#name"`
//!
//! `${ .. }` is inline JS, any number of statements. As a value it runs once per entity it applies to,
//! as `(e, root)`, and its last statement is the value. As a handler it is the body of
//! `function* (e, args, event)`.

use std::any::TypeId;
use std::collections::HashSet;
use std::fmt::Write;
use std::sync::Arc;

use crate::js_scan;
use crate::lexer::{Lexer, Loc, Tok, Token};
use crate::names::Resolver;
use crate::value::Value;

// CLAUDE
/// an `include` path's file and label
pub fn split_include(path: &str) -> (&str, Option<&str>) {
    match path.split_once('#') {
        Some((file, label)) => (file, Some(label)),
        None => (path, None),
    }
}

pub type Body = Arc<Vec<Item>>;

#[derive(Debug)]
pub enum Item {
    Patch { component: TypeId, name: String, field: Vec<String>, value: Value, loc: Loc },
    Classes { names: Vec<String>, loc: Loc },
    /// `key` is unique in the file, to recognize what this item spawned
    Spawn { name: String, body: Body, key: u32, loc: Loc },
    Include { path: String, child: bool, body: Option<Body>, loc: Loc },
    Rule { selectors: Vec<Selector>, body: Body, loc: Loc },
    On { event: TypeId, event_name: String, handler: Handler, loc: Loc },
    /// `asset name: Type value`: made when the file loads, a labeled asset `file#name`
    Asset { name: String, asset: TypeId, type_name: String, value: Value, loc: Loc },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Handler {
    /// a function the module exports
    Export(String),
    /// `${ .. }`: index into [`Doc::inline`]
    Inline(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Combinator {
    Descendant,
    Child,
}

/// compounds joined by combinators; the first one's combinator relates it to the rule's scope
#[derive(Debug, Default)]
pub struct Selector {
    pub parts: Vec<(Combinator, Compound)>,
}

#[derive(Debug, Default)]
pub struct Compound {
    /// type selectors: the entity has these components
    pub components: Vec<TypeId>,
    /// `#name`
    pub name: Option<String>,
    pub classes: Vec<String>,
    pub attrs: Vec<Attr>,
    pub pseudo: Vec<String>,
}

/// `[Comp]`, `[Comp.field="v"]`
#[derive(Debug)]
pub struct Attr {
    pub component: TypeId,
    pub field: Vec<String>,
    pub test: Option<(AttrOp, String)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttrOp {
    /// `=`
    Equals,
    /// `~=`: one of the whitespace separated words
    Word,
    /// `*=`
    Contains,
    /// `^=`
    Prefix,
    /// `$=`
    Suffix,
}

#[derive(Debug, Default)]
pub struct Doc {
    pub path: String,
    pub body: Body,
    /// `script { .. }` blocks, in order, with where they start
    pub scripts: Vec<(String, Loc)>,
    /// `${ .. }` code, by index
    pub inline: Vec<Inline>,
}

#[derive(Debug)]
pub struct Inline {
    pub source: String,
    /// an `on` handler, not a value
    pub handler: bool,
    pub loc: Loc,
}

// CLAUDE
/// turns `${ .. }` code into a value: an expression if it parses as one (so `{ a: 1 }` is an object),
/// else statements whose last one is the value. direct `eval` sees `e`, `root` and the module's variables
const VALUE_PRELUDE: &str = r#"
function __scene_value(body) {
  let source;
  try {
    new Function("return (" + body + "\n)");
    source = "(" + body + "\n)";
  } catch (_) {
    source = "{" + body + "\n}";
  }
  return function (e, root) {
    return eval(source);
  };
}
"#;

impl Doc {
    /// the top-level `spawn` items: each is also a scene of its own, `file#name`
    pub fn fragments(&self) -> impl Iterator<Item = (&str, u32)> {
        self.body.iter().filter_map(|item| match item {
            Item::Spawn { name, key, .. } => Some((name.as_str(), *key)),
            _ => None,
        })
    }

    /// every path an `include` names, anywhere in the file
    pub fn include_paths(&self) -> Vec<String> {
        fn walk(body: &Body, out: &mut Vec<String>) {
            for item in body.iter() {
                match item {
                    Item::Include { path, body, .. } => {
                        if !out.contains(path) {
                            out.push(path.clone());
                        }
                        if let Some(body) = body {
                            walk(body, out);
                        }
                    }
                    Item::Spawn { body, .. } | Item::Rule { body, .. } => walk(body, out),
                    _ => {}
                }
            }
        }
        let mut out = vec![];
        walk(&self.body, &mut out);
        out
    }

    /// the name of `${ .. }` number `index` in the module
    pub fn inline_function(&self, index: usize) -> String {
        if self.inline[index].handler { format!("__scene_h{index}") } else { format!("__scene_v{index}") }
    }

    // CLAUDE
    /// the file's script module: its `script` blocks (at their own line numbers, so errors point into the
    /// file) and a function for each `${ .. }`. None without either
    pub fn module_source(&self) -> Option<String> {
        if self.scripts.is_empty() && self.inline.is_empty() {
            return None;
        }
        let mut out = String::new();
        let mut line = 1;
        for (source, loc) in &self.scripts {
            while line < loc.line {
                out.push('\n');
                line += 1;
            }
            out.push_str(source);
            line += source.matches('\n').count() as u32;
        }
        out.push_str("\n;\n");
        out.push_str(VALUE_PRELUDE);
        for (index, inline) in self.inline.iter().enumerate() {
            let name = self.inline_function(index);
            if inline.handler {
                let _ = writeln!(out, "exports.{name} = function* (e, args, event) {{\n{}\n}};", inline.source);
            } else {
                // a JSON string is a JS string literal
                let body = serde_json::to_string(&inline.source).unwrap();
                let _ = writeln!(out, "exports.{name} = __scene_value({body});");
            }
        }
        Some(out)
    }
}

/// parses a file. Err holds all errors, Ok the doc and warnings
pub fn parse(src: &str, path: &str, resolver: Resolver) -> Result<(Doc, Vec<String>), Vec<String>> {
    let mut p = Parser {
        lex: Lexer::new(src),
        ahead: Vec::new(),
        resolver,
        path,
        errors: vec![],
        warnings: vec![],
        next_key: 0,
        asset_names: HashSet::new(),
        scripts: vec![],
        inline: vec![],
    };
    let body = match p.body(true) {
        Ok(body) => body,
        Err(e) => return Err(vec![e]),
    };
    if !p.errors.is_empty() {
        return Err(p.errors);
    }
    let doc = Doc { path: path.to_string(), body, scripts: p.scripts, inline: p.inline };
    Ok((doc, p.warnings))
}

struct Parser<'a> {
    lex: Lexer<'a>,
    ahead: Vec<Token>,
    resolver: Resolver<'a>,
    path: &'a str,
    /// errors that don't stop parsing (unknown names)
    errors: Vec<String>,
    warnings: Vec<String>,
    next_key: u32,
    asset_names: HashSet<String>,
    scripts: Vec<(String, Loc)>,
    inline: Vec<Inline>,
}

type PResult<T> = Result<T, String>;

impl<'a> Parser<'a> {
    fn loc(&self, at: usize) -> Loc {
        Loc::of(self.lex.src, at)
    }

    fn msg(&self, at: usize, msg: impl std::fmt::Display) -> String {
        format!("{}:{}: {msg}", self.path, self.loc(at))
    }

    fn error_at(&mut self, at: usize, msg: impl std::fmt::Display) {
        let m = self.msg(at, msg);
        self.errors.push(m);
    }

    fn peek(&mut self, n: usize) -> PResult<&Token> {
        while self.ahead.len() <= n {
            let t = self.lex.next().map_err(|(at, m)| self.msg(at, m))?;
            self.ahead.push(t);
        }
        Ok(&self.ahead[n])
    }

    fn next(&mut self) -> PResult<Token> {
        self.peek(0)?;
        Ok(self.ahead.remove(0))
    }

    fn is(&mut self, n: usize, c: char) -> PResult<bool> {
        Ok(self.peek(n)?.tok == Tok::Punct(c))
    }

    fn expect(&mut self, c: char) -> PResult<Token> {
        let t = self.next()?;
        if t.tok != Tok::Punct(c) {
            return Err(self.msg(t.at, format!("expected `{c}`, found {}", describe(&t.tok))));
        }
        Ok(t)
    }

    fn ident(&mut self) -> PResult<(String, usize)> {
        let t = self.next()?;
        match t.tok {
            Tok::Ident(s) => Ok((s, t.at)),
            other => Err(self.msg(t.at, format!("expected a name, found {}", describe(&other)))),
        }
    }

    fn key(&mut self) -> u32 {
        self.next_key += 1;
        self.next_key
    }

    fn body(&mut self, top: bool) -> PResult<Body> {
        let mut items = vec![];
        let mut uses_allowed = top;
        loop {
            let t = self.peek(0)?.clone();
            let word = match &t.tok {
                Tok::Ident(s) => s.as_str(),
                _ => "",
            };
            match &t.tok {
                Tok::Eof if top => break,
                Tok::Eof => return Err(self.msg(t.at, "missing `}`")),
                Tok::Punct('}') if !top => break,
                Tok::Punct('}') => return Err(self.msg(t.at, "unexpected `}`")),
                Tok::Punct(';') => {
                    self.next()?;
                    continue;
                }
                _ => {}
            }
            let is_use = word == "use" || word == "nodefaultuse";
            if is_use && !uses_allowed {
                return Err(self.msg(t.at, format!("`{word}` must come before everything else in the file")));
            }
            uses_allowed &= is_use;

            if word == "use" {
                self.next()?;
                self.use_line()?;
            } else if word == "nodefaultuse" {
                self.next()?;
                self.expect(';')?;
                self.resolver.no_default_use = true;
            } else if word == "spawn" && !self.is(1, ':')? {
                self.next()?;
                let name_tok = self.next()?;
                let name = match name_tok.tok {
                    Tok::Ident(s) | Tok::Str(s) => s,
                    other => return Err(self.msg(name_tok.at, format!("expected a name to spawn, found {}", describe(&other)))),
                };
                self.expect('{')?;
                let body = self.body(false)?;
                self.expect('}')?;
                let key = self.key();
                items.push(Item::Spawn { name, body, key, loc: self.loc(t.at) });
            } else if word == "include" && !self.is(1, ':')? {
                self.next()?;
                let path_tok = self.next()?;
                let Tok::Str(path) = path_tok.tok else {
                    return Err(self.msg(path_tok.at, "expected a \"path\" to include"));
                };
                let child = matches!(&self.peek(0)?.tok, Tok::Ident(w) if w == "as");
                if child {
                    self.next()?;
                    let (word, at) = self.ident()?;
                    if word != "child" {
                        return Err(self.msg(at, "expected `as child`"));
                    }
                }
                let body = if self.is(0, '{')? {
                    self.next()?;
                    let body = self.body(false)?;
                    self.expect('}')?;
                    Some(body)
                } else {
                    self.expect(';')?;
                    None
                };
                items.push(Item::Include { path, child, body, loc: self.loc(t.at) });
            } else if word == "script" && self.is(1, '{')? {
                if !top {
                    return Err(self.msg(t.at, "`script` blocks are the file's module: top level only"));
                }
                self.next()?;
                let (source, at) = self.js_block()?;
                self.scripts.push((source, self.loc(at)));
            } else if word == "on" && matches!(self.peek(1)?.tok, Tok::Ident(_)) {
                self.next()?;
                let (event_name, at) = self.type_name()?;
                self.expect(':')?;
                let handler = if self.is_inline()? {
                    Handler::Inline(self.inline(true)?)
                } else {
                    match self.next()?.tok {
                        Tok::Ident(s) | Tok::Str(s) => {
                            self.end_item()?;
                            Handler::Export(s)
                        }
                        other => return Err(self.msg(t.at, format!("expected a handler, found {}", describe(&other)))),
                    }
                };
                if let Some(event) = self.resolve(&event_name, at) {
                    items.push(Item::On { event, event_name, handler, loc: self.loc(t.at) });
                }
            } else if word == "asset" && matches!(self.peek(1)?.tok, Tok::Ident(_)) && self.is(2, ':')? {
                if !top {
                    return Err(self.msg(t.at, "`asset` is only allowed at the top level"));
                }
                self.next()?;
                let (name, at) = self.ident()?;
                self.next()?;
                let (type_name, type_at) = self.type_name()?;
                let value = self.value_seq()?;
                if !ends_itself(&value) {
                    self.end_item()?;
                }
                if !self.asset_names.insert(name.clone()) {
                    self.error_at(at, format!("`asset {name}` is defined twice"));
                }
                if let Some(asset) = self.resolve(&type_name, type_at) {
                    items.push(Item::Asset { name, asset, type_name, value, loc: self.loc(t.at) });
                }
            } else if word == "class" && self.is(1, ':')? {
                self.next()?;
                self.next()?;
                let mut names = vec![];
                while let Tok::Ident(name) = &self.peek(0)?.tok {
                    let name = name.clone();
                    self.next()?;
                    names.push(name);
                }
                self.end_item()?;
                items.push(Item::Classes { names, loc: self.loc(t.at) });
            } else if self.is_patch()? {
                items.extend(self.patch()?);
            } else {
                items.push(self.rule()?);
            }
        }
        Ok(Arc::new(items))
    }

    /// `;`, or nothing before a `}` or the end
    fn end_item(&mut self) -> PResult<()> {
        if self.is(0, '}')? || self.peek(0)?.tok == Tok::Eof {
            return Ok(());
        }
        self.expect(';').map(|_| ())
    }

    fn use_line(&mut self) -> PResult<()> {
        let mut prefix = vec![];
        loop {
            let (name, at) = self.ident()?;
            prefix.push(name);
            if self.peek(0)?.tok != Tok::PathSep {
                self.expect(';')?;
                self.add_use(&prefix.join("::"), at);
                return Ok(());
            }
            self.next()?;
            if self.is(0, '{')? {
                self.next()?;
                loop {
                    let (name, at) = self.type_name()?;
                    self.add_use(&format!("{}::{name}", prefix.join("::")), at);
                    if self.is(0, ',')? {
                        self.next()?;
                    }
                    if self.is(0, '}')? {
                        self.next()?;
                        break;
                    }
                }
                self.expect(';')?;
                return Ok(());
            }
        }
    }

    fn add_use(&mut self, path: &str, at: usize) {
        match self.resolver.add_use(path) {
            Ok(warning) => {
                if let Some(w) = warning {
                    let m = self.msg(at, w);
                    self.warnings.push(m);
                }
            }
            Err(e) => self.error_at(at, e),
        }
    }

    fn resolve(&mut self, name: &str, at: usize) -> Option<TypeId> {
        match self.resolver.resolve(name) {
            Ok(r) => {
                if let Some(w) = r.warning {
                    let m = self.msg(at, w);
                    self.warnings.push(m);
                }
                Some(r.type_id)
            }
            Err(e) => {
                self.error_at(at, e);
                None
            }
        }
    }

    /// `a::b::C<D, e::F>`: the text and where it starts
    fn type_name(&mut self) -> PResult<(String, usize)> {
        let (mut name, at) = self.ident()?;
        loop {
            if self.peek(0)?.tok == Tok::PathSep {
                self.next()?;
                name.push_str("::");
                name.push_str(&self.ident()?.0);
            } else if self.is(0, '<')? && !self.peek(0)?.space_before {
                self.next()?;
                name.push('<');
                loop {
                    let (arg, _) = self.type_name()?;
                    name.push_str(&arg);
                    if self.is(0, ',')? {
                        self.next()?;
                        name.push_str(", ");
                    } else {
                        self.expect('>')?;
                        name.push('>');
                        break;
                    }
                }
            } else {
                return Ok((name, at));
            }
        }
    }

    /// `Type(::seg)*(<..>)?(.field)*: ` followed by whitespace (or `{`): a patch, not a selector
    fn is_patch(&mut self) -> PResult<bool> {
        let mut i = 0;
        if !matches!(self.peek(i)?.tok, Tok::Ident(_)) {
            return Ok(false);
        }
        i += 1;
        let mut generic_depth = 0;
        loop {
            let t = self.peek(i)?.clone();
            match t.tok {
                Tok::PathSep | Tok::Punct('.') | Tok::Punct(',') if matches!(self.peek(i + 1)?.tok, Tok::Ident(_)) => {
                    if t.tok == Tok::Punct(',') && generic_depth == 0 {
                        return Ok(false);
                    }
                    i += 2;
                }
                Tok::Punct('<') if !t.space_before => {
                    generic_depth += 1;
                    i += 1;
                }
                Tok::Punct('>') if generic_depth > 0 => {
                    generic_depth -= 1;
                    i += 1;
                }
                Tok::Ident(_) if generic_depth > 0 => i += 1,
                Tok::Punct(':') if generic_depth == 0 => {
                    let after = self.peek(i + 1)?;
                    return Ok(after.space_before || after.tok == Tok::Punct('{'));
                }
                _ => return Ok(false),
            }
        }
    }

    fn patch(&mut self) -> PResult<Option<Item>> {
        let (name, at) = self.type_name()?;
        let mut field = vec![];
        while self.is(0, '.')? {
            self.next()?;
            field.push(self.ident()?.0);
        }
        self.expect(':')?;
        let value = self.value_seq()?;
        if !ends_itself(&value) {
            self.end_item()?;
        }
        Ok(self.resolve(&name, at).map(|component| Item::Patch { component, name, field, value, loc: self.loc(at) }))
    }

    /// `${` next, touching
    fn is_inline(&mut self) -> PResult<bool> {
        Ok(self.is(0, '$')? && self.is(1, '{')? && !self.peek(1)?.space_before)
    }

    // CLAUDE
    /// `${ .. }` (at `$`): records the code, giving its index
    fn inline(&mut self, handler: bool) -> PResult<usize> {
        let at = self.next()?.at;
        let (source, _) = self.js_block()?;
        self.inline.push(Inline { source, handler, loc: self.loc(at) });
        Ok(self.inline.len() - 1)
    }

    /// raw JS up to the `}` matching the `{` next (both consumed): its text and where it starts
    fn js_block(&mut self) -> PResult<(String, usize)> {
        self.expect('{')?;
        debug_assert!(self.ahead.is_empty());
        let start = self.lex.pos;
        let end = js_scan::script_end(&self.lex.src[start..])
            .map_err(|e| self.msg(start + e.start, format!("unclosed {:?} in script", e.kind)))?;
        self.lex.pos = start + end + 1;
        Ok((self.lex.src[start..start + end].to_string(), start))
    }

    fn value_end(&mut self) -> PResult<bool> {
        Ok(matches!(self.peek(0)?.tok, Tok::Eof | Tok::Punct(';' | '}' | ',' | ']' | ')')))
    }

    fn value_seq(&mut self) -> PResult<Value> {
        let mut items = vec![];
        while !self.value_end()? {
            items.push(self.value()?);
            // `Comp: { .. }` needs no `;` after the `}`
            if items.last().is_some_and(ends_itself) {
                break;
            }
        }
        match items.len() {
            0 => {
                let at = self.peek(0)?.at;
                Err(self.msg(at, "missing value"))
            }
            1 => Ok(items.pop().unwrap()),
            _ => Ok(Value::Seq(items)),
        }
    }

    fn value(&mut self) -> PResult<Value> {
        if self.is_inline()? {
            return Ok(Value::Script(self.inline(false)?));
        }
        let t = self.next()?;
        Ok(match t.tok {
            Tok::Number { value, unit } => Value::Num { value, unit },
            Tok::Str(s) => Value::Str(s),
            Tok::Hash(h) => Value::Hex(h),
            Tok::Punct('[') => Value::List(self.list(']')?),
            Tok::Punct('(') => Value::Call(String::new(), self.list(')')?),
            Tok::Punct('{') => Value::Struct(String::new(), self.entries()?),
            Tok::Ident(_) => {
                self.ahead.insert(0, t);
                let (name, _) = self.type_name()?;
                if self.is(0, '(')? && !self.peek(0)?.space_before {
                    self.next()?;
                    Value::Call(name, self.list(')')?)
                } else if self.is(0, '{')? {
                    self.next()?;
                    Value::Struct(name, self.entries()?)
                } else {
                    Value::Path(name)
                }
            }
            other => return Err(self.msg(t.at, format!("expected a value, found {}", describe(&other)))),
        })
    }

    /// comma separated values up to `close` (consumed)
    fn list(&mut self, close: char) -> PResult<Vec<Value>> {
        let mut items = vec![];
        loop {
            if self.is(0, close)? {
                self.next()?;
                return Ok(items);
            }
            items.push(self.value_seq()?);
            if !self.is(0, close)? {
                self.expect(',')?;
            }
        }
    }

    /// `a: 1; b.c: 2` up to `}` (consumed), `,` also separates
    fn entries(&mut self) -> PResult<Vec<(Vec<String>, Value)>> {
        let mut entries = vec![];
        loop {
            if self.is(0, '}')? {
                self.next()?;
                return Ok(entries);
            }
            let mut path = vec![self.ident()?.0];
            while self.is(0, '.')? {
                self.next()?;
                path.push(self.ident()?.0);
            }
            self.expect(':')?;
            entries.push((path, self.value_seq()?));
            if !self.is(0, '}')? {
                let t = self.next()?;
                if !matches!(t.tok, Tok::Punct(';' | ',')) {
                    return Err(self.msg(t.at, format!("expected `;`, found {}", describe(&t.tok))));
                }
            }
        }
    }

    fn rule(&mut self) -> PResult<Item> {
        let at = self.peek(0)?.at;
        let mut selectors = vec![self.selector()?];
        while self.is(0, ',')? {
            self.next()?;
            selectors.push(self.selector()?);
        }
        let t = self.next()?;
        if t.tok != Tok::Punct('{') {
            let hint = if t.tok == Tok::Punct(';') { " (a component patch needs a space after `:`)" } else { "" };
            return Err(self.msg(t.at, format!("expected `{{` after a selector, found {}{hint}", describe(&t.tok))));
        }
        let body = self.body(false)?;
        self.expect('}')?;
        Ok(Item::Rule { selectors, body, loc: self.loc(at) })
    }

    fn selector(&mut self) -> PResult<Selector> {
        let mut selector = Selector::default();
        let mut combinator = Combinator::Descendant;
        if self.is(0, '>')? {
            self.next()?;
            combinator = Combinator::Child;
        }
        loop {
            selector.parts.push((combinator, self.compound()?));
            if self.is(0, '>')? {
                self.next()?;
                combinator = Combinator::Child;
            } else if self.is(0, '{')? || self.is(0, ',')? || self.is(0, ';')? || self.peek(0)?.tok == Tok::Eof {
                return Ok(selector);
            } else {
                combinator = Combinator::Descendant;
            }
        }
    }

    fn compound(&mut self) -> PResult<Compound> {
        let mut c = Compound::default();
        let mut first = true;
        loop {
            let t = self.peek(0)?.clone();
            if !first && t.space_before {
                break;
            }
            match &t.tok {
                Tok::Ident(_) if first => {
                    let (name, at) = self.type_name()?;
                    c.components.extend(self.resolve(&name, at));
                }
                Tok::Punct('*') if first => {
                    self.next()?;
                }
                Tok::Hash(name) => {
                    c.name = Some(name.clone());
                    self.next()?;
                }
                Tok::Punct('.') => {
                    self.next()?;
                    c.classes.push(self.ident()?.0);
                }
                Tok::Punct('[') => {
                    self.next()?;
                    c.attrs.extend(self.attr()?);
                }
                Tok::Punct(':') => {
                    self.next()?;
                    let (mut name, _) = self.ident()?;
                    if self.is(0, '(')? {
                        // keep arguments as text for now
                        let start = self.next()?.at;
                        let mut depth = 1;
                        while depth > 0 {
                            match self.next()?.tok {
                                Tok::Punct('(') => depth += 1,
                                Tok::Punct(')') => depth -= 1,
                                Tok::Eof => return Err(self.msg(start, "unclosed `(`")),
                                _ => {}
                            }
                        }
                        name.push_str("(..)");
                    }
                    c.pseudo.push(name);
                }
                Tok::Punct('&') => return Err(self.msg(t.at, "`&` is not supported yet")),
                _ if first => return Err(self.msg(t.at, format!("expected a selector, found {}", describe(&t.tok)))),
                _ => break,
            }
            first = false;
        }
        Ok(c)
    }

    fn attr(&mut self) -> PResult<Option<Attr>> {
        let (name, at) = self.type_name()?;
        let mut field = vec![];
        while self.is(0, '.')? {
            self.next()?;
            field.push(self.ident()?.0);
        }
        let t = self.next()?;
        let op = match t.tok {
            Tok::Punct(']') => None,
            Tok::Punct('=') => Some(AttrOp::Equals),
            Tok::Punct(c @ ('~' | '*' | '^' | '$')) => {
                self.expect('=')?;
                Some(match c {
                    '~' => AttrOp::Word,
                    '*' => AttrOp::Contains,
                    '^' => AttrOp::Prefix,
                    _ => AttrOp::Suffix,
                })
            }
            other => return Err(self.msg(t.at, format!("expected `]` or `=`, found {}", describe(&other)))),
        };
        let test = match op {
            None => None,
            Some(op) => {
                let v = self.next()?;
                let text = match v.tok {
                    Tok::Str(s) | Tok::Ident(s) => s,
                    Tok::Number { value, unit } => format!("{value}{}", unit.unwrap_or_default()),
                    other => return Err(self.msg(v.at, format!("expected a value, found {}", describe(&other)))),
                };
                self.expect(']')?;
                Some((op, text))
            }
        };
        Ok(self.resolve(&name, at).map(|component| Attr { component, field, test }))
    }
}

fn describe(tok: &Tok) -> String {
    match tok {
        Tok::Ident(s) => format!("`{s}`"),
        Tok::Number { value, unit } => format!("`{value}{}`", unit.as_deref().unwrap_or("")),
        Tok::Str(s) => format!("{s:?}"),
        Tok::Hash(h) => format!("`#{h}`"),
        Tok::PathSep => "`::`".into(),
        Tok::Punct(c) => format!("`{c}`"),
        Tok::Eof => "the end of the file".into(),
    }
}

/// a value ending in `}` ends its item: no `;` needed
fn ends_itself(value: &Value) -> bool {
    matches!(value, Value::Struct(..) | Value::Script(_))
}
