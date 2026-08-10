//! Command palette (`Cmd+Shift+P`, `LUX_PROMPT_MAESTRO.md` Phase 3
//! deliverable 7): a single command registry with categories, recency
//! ranking, and `when`-clause context keys for contextual enablement.
//!
//! **"Every menu command must be in the palette."** No menu UI exists yet
//! in `lux-ui` (Phase 3 has shipped no interactive widgets so far, only
//! passive ones) — but the invariant is structural, not aspirational: a
//! [`CommandRegistry`] is the *only* place a command can be defined. There
//! is no separate "menu item" type anywhere in this crate. When a real
//! menu is built, it has no choice but to source its items from
//! [`CommandRegistry::commands`] — the same registry the palette searches
//! — because no other list exists to source them from. This is what v0
//! got wrong: menus and the palette were two independently maintained
//! lists that drifted apart.
//!
//! Fuzzy ranking reuses `lux-search::fuzzy::rank_by`, the same `nucleo`
//! engine as Goto Anything (`docs/adr/0009-search-and-fuzzy-matching.md`).

use lux_search::fuzzy::rank_by;
use std::collections::{HashMap, HashSet};
use std::fmt;

/// Boolean context keys a `when` clause evaluates against (e.g.
/// `"editorFocus"`, `"hasSelection"`). A key absent from the context is
/// `false`, the same convention VS Code's `when` clauses use.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Context(HashMap<String, bool>);

impl Context {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, key: impl Into<String>, value: bool) -> &mut Self {
        self.0.insert(key.into(), value);
        self
    }

    fn get(&self, key: &str) -> bool {
        self.0.get(key).copied().unwrap_or(false)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Expr {
    Key(String),
    Not(Box<Expr>),
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
}

impl Expr {
    fn eval(&self, ctx: &Context) -> bool {
        match self {
            Expr::Key(key) => ctx.get(key),
            Expr::Not(e) => !e.eval(ctx),
            Expr::And(a, b) => a.eval(ctx) && b.eval(ctx),
            Expr::Or(a, b) => a.eval(ctx) || b.eval(ctx),
        }
    }
}

/// A parsed `when` clause: a boolean expression over context keys, with
/// `&&`/`||`/`!`/`(...)` and the usual precedence (`!` > `&&` > `||`). An
/// empty clause always evaluates to `true` (no restriction).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct When(Option<Expr>);

impl When {
    /// Always enabled, regardless of context — the default for a command
    /// with no `when` clause.
    pub fn always() -> Self {
        When(None)
    }

    pub fn parse(raw: &str) -> Result<Self, WhenParseError> {
        if raw.trim().is_empty() {
            return Ok(When(None));
        }
        let tokens = tokenize(raw)?;
        let mut parser = Parser {
            tokens: &tokens,
            pos: 0,
        };
        let expr = parser.parse_or()?;
        if parser.pos != tokens.len() {
            return Err(WhenParseError(format!(
                "unexpected trailing input in when clause {raw:?}"
            )));
        }
        Ok(When(Some(expr)))
    }

    pub fn is_satisfied(&self, ctx: &Context) -> bool {
        match &self.0 {
            None => true,
            Some(expr) => expr.eval(ctx),
        }
    }
}

/// A malformed `when` clause: unknown character, unbalanced parentheses,
/// a dangling operator, or similar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhenParseError(pub String);

impl fmt::Display for WhenParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid when clause: {}", self.0)
    }
}

impl std::error::Error for WhenParseError {}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    Ident(String),
    Not,
    And,
    Or,
    LParen,
    RParen,
}

fn tokenize(raw: &str) -> Result<Vec<Token>, WhenParseError> {
    let chars: Vec<char> = raw.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            c if c.is_whitespace() => i += 1,
            '(' => {
                tokens.push(Token::LParen);
                i += 1;
            }
            ')' => {
                tokens.push(Token::RParen);
                i += 1;
            }
            '!' => {
                tokens.push(Token::Not);
                i += 1;
            }
            '&' if chars.get(i + 1) == Some(&'&') => {
                tokens.push(Token::And);
                i += 2;
            }
            '|' if chars.get(i + 1) == Some(&'|') => {
                tokens.push(Token::Or);
                i += 2;
            }
            c if c.is_alphanumeric() || c == '_' => {
                let start = i;
                while i < chars.len()
                    && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '.')
                {
                    i += 1;
                }
                tokens.push(Token::Ident(chars[start..i].iter().collect()));
            }
            other => {
                return Err(WhenParseError(format!(
                    "unexpected character {other:?} in when clause {raw:?}"
                )));
            }
        }
    }
    Ok(tokens)
}

struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn parse_or(&mut self) -> Result<Expr, WhenParseError> {
        let mut expr = self.parse_and()?;
        while matches!(self.peek(), Some(Token::Or)) {
            self.pos += 1;
            expr = Expr::Or(Box::new(expr), Box::new(self.parse_and()?));
        }
        Ok(expr)
    }

    fn parse_and(&mut self) -> Result<Expr, WhenParseError> {
        let mut expr = self.parse_unary()?;
        while matches!(self.peek(), Some(Token::And)) {
            self.pos += 1;
            expr = Expr::And(Box::new(expr), Box::new(self.parse_unary()?));
        }
        Ok(expr)
    }

    fn parse_unary(&mut self) -> Result<Expr, WhenParseError> {
        if matches!(self.peek(), Some(Token::Not)) {
            self.pos += 1;
            return Ok(Expr::Not(Box::new(self.parse_unary()?)));
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<Expr, WhenParseError> {
        match self.tokens.get(self.pos) {
            Some(Token::Ident(name)) => {
                self.pos += 1;
                Ok(Expr::Key(name.clone()))
            }
            Some(Token::LParen) => {
                self.pos += 1;
                let expr = self.parse_or()?;
                match self.tokens.get(self.pos) {
                    Some(Token::RParen) => {
                        self.pos += 1;
                        Ok(expr)
                    }
                    _ => Err(WhenParseError("expected ')'".to_string())),
                }
            }
            Some(other) => Err(WhenParseError(format!("unexpected token {other:?}"))),
            None => Err(WhenParseError("expected an expression".to_string())),
        }
    }
}

/// One entry in the command registry: a stable `id`, a human-readable
/// `title` and `category`, and an optional `when` clause for contextual
/// enablement (always enabled by default).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub id: String,
    pub title: String,
    pub category: String,
    pub when: When,
}

impl Command {
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        category: impl Into<String>,
    ) -> Self {
        Command {
            id: id.into(),
            title: title.into(),
            category: category.into(),
            when: When::always(),
        }
    }

    pub fn with_when(mut self, when: When) -> Self {
        self.when = when;
        self
    }
}

/// Registering a command whose `id` is already taken — every command must
/// have a unique, stable id since it's what recency tracking and future
/// keybinding lookups (Phase 5) key on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DuplicateCommandId(pub String);

impl fmt::Display for DuplicateCommandId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "command id already registered: {}", self.0)
    }
}

impl std::error::Error for DuplicateCommandId {}

/// The single command registry. Every command in the application — menu
/// item or not — is registered here exactly once; the palette searches
/// this and nothing else.
#[derive(Debug, Default)]
pub struct CommandRegistry {
    commands: Vec<Command>,
    ids: HashSet<String>,
    last_used: HashMap<String, u64>,
    clock: u64,
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, command: Command) -> Result<(), DuplicateCommandId> {
        if !self.ids.insert(command.id.clone()) {
            return Err(DuplicateCommandId(command.id));
        }
        self.commands.push(command);
        Ok(())
    }

    /// Every registered command, in registration order — the list a menu
    /// would filter by category to build itself from (see module docs).
    pub fn commands(&self) -> impl Iterator<Item = &Command> {
        self.commands.iter()
    }

    /// Marks `id` as used just now, for recency ranking. A logical clock
    /// (not a wall-clock timestamp) — only relative order matters, which
    /// also keeps ranking deterministic and testable.
    pub fn record_use(&mut self, id: &str) {
        self.clock += 1;
        self.last_used.insert(id.to_string(), self.clock);
    }

    /// Searches enabled commands (`when` satisfied by `ctx`) by `query`,
    /// highest score first. An empty query lists every enabled command,
    /// most-recently-used first, then never-used commands in registration
    /// order — the palette's "recent commands" view. A non-empty query
    /// fuzzy-ranks by title, breaking ties between equally good matches by
    /// recency.
    pub fn search(&self, query: &str, ctx: &Context) -> Vec<PaletteEntry<'_>> {
        let enabled: Vec<&Command> = self
            .commands
            .iter()
            .filter(|c| c.when.is_satisfied(ctx))
            .collect();
        let recency_of = |id: &str| self.last_used.get(id).copied().unwrap_or(0);

        if query.trim().is_empty() {
            let mut entries = enabled;
            entries.sort_by_key(|c| std::cmp::Reverse(recency_of(&c.id)));
            return entries
                .into_iter()
                .map(|command| PaletteEntry { command, score: 0 })
                .collect();
        }

        let mut ranked = rank_by(query, enabled, |c| c.title.as_str());
        ranked.sort_by(|(a, score_a), (b, score_b)| {
            score_b
                .cmp(score_a)
                .then_with(|| recency_of(&b.id).cmp(&recency_of(&a.id)))
        });
        ranked
            .into_iter()
            .map(|(command, score)| PaletteEntry { command, score })
            .collect()
    }
}

/// One ranked palette result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaletteEntry<'a> {
    pub command: &'a Command,
    pub score: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmd(id: &str, title: &str, category: &str) -> Command {
        Command::new(id, title, category)
    }

    #[test]
    fn register_rejects_duplicate_ids() {
        let mut registry = CommandRegistry::new();
        registry.register(cmd("save", "Save File", "File")).unwrap();
        let err = registry
            .register(cmd("save", "Save File As...", "File"))
            .unwrap_err();
        assert_eq!(err, DuplicateCommandId("save".to_string()));
    }

    #[test]
    fn when_bare_key_is_true_only_if_present_and_true() {
        let when = When::parse("editorFocus").unwrap();
        let mut ctx = Context::new();
        assert!(!when.is_satisfied(&ctx));
        ctx.set("editorFocus", true);
        assert!(when.is_satisfied(&ctx));
        ctx.set("editorFocus", false);
        assert!(!when.is_satisfied(&ctx));
    }

    #[test]
    fn when_supports_negation_and_and_or() {
        let mut ctx = Context::new();
        ctx.set("editorFocus", true);
        ctx.set("hasSelection", false);

        assert!(
            When::parse("editorFocus && !hasSelection")
                .unwrap()
                .is_satisfied(&ctx)
        );
        assert!(
            !When::parse("editorFocus && hasSelection")
                .unwrap()
                .is_satisfied(&ctx)
        );
        assert!(
            When::parse("hasSelection || editorFocus")
                .unwrap()
                .is_satisfied(&ctx)
        );
    }

    #[test]
    fn when_and_binds_tighter_than_or() {
        // "a || (b && c)", not "(a || b) && c".
        let mut ctx = Context::new();
        ctx.set("a", true);
        ctx.set("b", false);
        ctx.set("c", false);
        assert!(When::parse("a || b && c").unwrap().is_satisfied(&ctx));
    }

    #[test]
    fn when_parentheses_override_precedence() {
        let mut ctx = Context::new();
        ctx.set("a", true);
        ctx.set("b", false);
        ctx.set("c", false);
        // "(a || b) && c" is false here, unlike the unparenthesized form.
        assert!(!When::parse("(a || b) && c").unwrap().is_satisfied(&ctx));
    }

    #[test]
    fn when_empty_clause_is_always_satisfied() {
        assert!(When::parse("").unwrap().is_satisfied(&Context::new()));
        assert!(When::parse("   ").unwrap().is_satisfied(&Context::new()));
        assert!(When::always().is_satisfied(&Context::new()));
    }

    #[test]
    fn when_rejects_malformed_clauses() {
        assert!(When::parse("(a").is_err());
        assert!(When::parse("a &&").is_err());
        assert!(When::parse("a $ b").is_err());
        assert!(When::parse("&& a").is_err());
        assert!(When::parse("a b").is_err());
    }

    #[test]
    fn search_filters_out_commands_whose_when_clause_fails() {
        let mut registry = CommandRegistry::new();
        registry
            .register(
                cmd("format", "Format Document", "Editor")
                    .with_when(When::parse("editorFocus").unwrap()),
            )
            .unwrap();
        registry
            .register(cmd("about", "About Lux", "Help"))
            .unwrap();

        let empty_ctx = Context::new();
        let results: Vec<&str> = registry
            .search("", &empty_ctx)
            .into_iter()
            .map(|e| e.command.id.as_str())
            .collect();
        assert_eq!(results, vec!["about"]);

        let mut focused_ctx = Context::new();
        focused_ctx.set("editorFocus", true);
        let results: Vec<&str> = registry
            .search("", &focused_ctx)
            .into_iter()
            .map(|e| e.command.id.as_str())
            .collect();
        assert_eq!(results.len(), 2);
        assert!(results.contains(&"format"));
    }

    #[test]
    fn search_with_query_fuzzy_ranks_by_title() {
        let mut registry = CommandRegistry::new();
        registry.register(cmd("save", "Save File", "File")).unwrap();
        registry
            .register(cmd("close", "Close Window", "Window"))
            .unwrap();
        let results = registry.search("save", &Context::new());
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].command.id, "save");
    }

    #[test]
    fn search_empty_query_orders_by_recency_most_recent_first() {
        let mut registry = CommandRegistry::new();
        registry.register(cmd("a", "Command A", "Test")).unwrap();
        registry.register(cmd("b", "Command B", "Test")).unwrap();
        registry.register(cmd("c", "Command C", "Test")).unwrap();

        registry.record_use("b");
        registry.record_use("a");

        let results: Vec<&str> = registry
            .search("", &Context::new())
            .into_iter()
            .map(|e| e.command.id.as_str())
            .collect();
        // "a" used most recently, then "b", then never-used "c" last (in
        // registration order among the never-used tail).
        assert_eq!(results, vec!["a", "b", "c"]);
    }

    #[test]
    fn search_with_query_breaks_score_ties_by_recency() {
        let mut registry = CommandRegistry::new();
        registry.register(cmd("a", "Format", "Editor")).unwrap();
        registry.register(cmd("b", "Format", "Editor")).unwrap();

        registry.record_use("b");

        let results: Vec<&str> = registry
            .search("format", &Context::new())
            .into_iter()
            .map(|e| e.command.id.as_str())
            .collect();
        assert_eq!(results, vec!["b", "a"]);
    }

    #[test]
    fn no_separate_menu_list_every_registered_command_is_in_the_palette() {
        // The structural invariant this deliverable exists to enforce:
        // there is nothing to register a command *as a menu item* other
        // than this same registry, so a full, unrestricted palette search
        // always contains exactly what was registered — no drift between
        // "menu" and "palette" is possible because there is only one list.
        let mut registry = CommandRegistry::new();
        let ids = ["new_file", "open_file", "save_file", "close_window"];
        for id in ids {
            registry.register(cmd(id, id, "File")).unwrap();
        }

        let mut palette_ids: Vec<&str> = registry
            .search("", &Context::new())
            .into_iter()
            .map(|e| e.command.id.as_str())
            .collect();
        palette_ids.sort_unstable();
        let mut expected: Vec<&str> = ids.to_vec();
        expected.sort_unstable();
        assert_eq!(palette_ids, expected);
    }
}
