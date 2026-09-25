//! Build-time half of the UI translations (the runtime half is `src/i18n.rs`).
//!
//! `i18n/en.ftl` is the source of truth. For every message it holds, this
//! generates a typed function in `i18n::t` (`menu-file-new` becomes
//! `t::menu_file_new(shortcut)`), so a misspelled message or a missing,
//! extra or misnamed argument is a compile error instead of a blank label
//! at runtime.
//!
//! The translations are checked too. A syntax error, a message English does
//! not have, a reference to a missing message, or a `$variable` the code
//! never passes fails the build; untranslated messages only warn, because
//! they fall back to English at runtime.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use fluent_syntax::ast;
use fluent_syntax::parser;

/// Where the `.ftl` files live, relative to the package root.
const DIRECTORY: &str = "i18n";
const SOURCE_LANGUAGE: &str = "en";
const TRANSLATIONS: &[&str] = &["es", "pt-BR", "ru"];

struct Message {
    /// Variables the message reads, including through message references,
    /// in order of first appearance: the generated function takes its
    /// arguments in this order, as a reader of the English text expects.
    variables: Vec<String>,
    /// Messages this one references (`{ other-message }`).
    references: BTreeSet<String>,
    /// Readable rendering of the value, for the generated doc comment.
    text: String,
    line: usize,
}

impl Message {
    /// Record a variable once, keeping first-appearance order.
    fn add_variable(&mut self, name: &str) -> bool {
        let new = !self.variables.iter().any(|known| known == name);
        if new {
            self.variables.push(name.to_string());
        }
        new
    }
}

struct Catalog {
    path: PathBuf,
    messages: BTreeMap<String, Message>,
}

pub fn generate() {
    println!("cargo:rerun-if-changed=build/i18n.rs");
    let english = load(SOURCE_LANGUAGE);
    for id in english.messages.keys() {
        if !is_kebab_case(id) {
            fail(
                &english,
                id,
                "message ids must be lowercase kebab-case (they become Rust function names)",
            );
        }
    }
    for language in TRANSLATIONS {
        check_translation(&english, &load(language));
    }

    let out = PathBuf::from(std::env::var("OUT_DIR").expect("cargo sets OUT_DIR"));
    std::fs::write(out.join("i18n_messages.rs"), render(&english))
        .expect("failed to write the generated translation API");
}

fn load(language: &str) -> Catalog {
    let path = Path::new(DIRECTORY).join(format!("{language}.ftl"));
    println!("cargo:rerun-if-changed={}", path.display());
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    let resource = match parser::parse(source.as_str()) {
        Ok(resource) => resource,
        Err((_, errors)) => {
            let report: Vec<String> = errors
                .iter()
                .map(|error| {
                    format!(
                        "{}:{}: {error}",
                        path.display(),
                        line_of(&source, error.pos.start)
                    )
                })
                .collect();
            panic!("Fluent syntax errors:\n{}", report.join("\n"));
        }
    };

    let mut catalog = Catalog {
        path,
        messages: BTreeMap::new(),
    };
    for entry in &resource.body {
        let ast::Entry::Message(message) = entry else {
            continue;
        };
        let id = message.id.name.to_string();
        let line = line_of(&source, offset_in(&source, message.id.name));
        let Some(value) = &message.value else {
            panic!(
                "{}:{line}: `{id}` has no value; attribute-only messages are not supported",
                catalog.path.display()
            );
        };
        let mut info = Message {
            variables: Vec::new(),
            references: BTreeSet::new(),
            text: String::new(),
            line,
        };
        walk_pattern(value, &mut info);
        info.text = render_pattern(value);
        if catalog.messages.insert(id.clone(), info).is_some() {
            panic!("{}:{line}: `{id}` is defined twice", catalog.path.display());
        }
    }
    resolve_references(&mut catalog);
    catalog
}

/// Fold each referenced message's variables into the referencing message:
/// formatting `a = { b }` needs every argument `b` reads.
fn resolve_references(catalog: &mut Catalog) {
    for (id, message) in &catalog.messages {
        for reference in &message.references {
            if !catalog.messages.contains_key(reference) {
                fail(
                    catalog,
                    id,
                    &format!("references `{reference}`, which this file does not define"),
                );
            }
        }
    }
    loop {
        let mut changed = false;
        let ids: Vec<String> = catalog.messages.keys().cloned().collect();
        for id in ids {
            let inherited: Vec<String> = catalog.messages[&id]
                .references
                .iter()
                .flat_map(|reference| catalog.messages[reference].variables.clone())
                .collect();
            let message = catalog.messages.get_mut(&id).expect("id came from the map");
            for variable in inherited {
                changed |= message.add_variable(&variable);
            }
        }
        if !changed {
            break;
        }
    }
}

fn check_translation(english: &Catalog, translation: &Catalog) {
    for (id, message) in &translation.messages {
        let Some(source) = english.messages.get(id) else {
            fail(
                translation,
                id,
                "is not in en.ftl (renamed or removed?); delete it or add it to English first",
            );
        };
        let unknown: Vec<&String> = message
            .variables
            .iter()
            .filter(|variable| !source.variables.contains(variable))
            .collect();
        if !unknown.is_empty() {
            fail(
                translation,
                id,
                &format!(
                    "uses {unknown:?}, but the code only passes {:?}",
                    source.variables
                ),
            );
        }
    }
    let missing = english
        .messages
        .keys()
        .filter(|id| !translation.messages.contains_key(*id))
        .count();
    if missing > 0 {
        println!(
            "cargo:warning={}: {missing} of {} messages untranslated (English is shown for them)",
            translation.path.display(),
            english.messages.len()
        );
    }
}

fn walk_pattern(pattern: &ast::Pattern<&str>, info: &mut Message) {
    for element in &pattern.elements {
        if let ast::PatternElement::Placeable { expression } = element {
            walk_expression(expression, info);
        }
    }
}

fn walk_expression(expression: &ast::Expression<&str>, info: &mut Message) {
    match expression {
        ast::Expression::Select { selector, variants } => {
            walk_inline(selector, info);
            for variant in variants {
                walk_pattern(&variant.value, info);
            }
        }
        ast::Expression::Inline(inline) => walk_inline(inline, info),
    }
}

fn walk_inline(inline: &ast::InlineExpression<&str>, info: &mut Message) {
    match inline {
        ast::InlineExpression::VariableReference { id } => {
            info.add_variable(id.name);
        }
        ast::InlineExpression::MessageReference { id, .. } => {
            info.references.insert(id.name.to_string());
        }
        ast::InlineExpression::FunctionReference { arguments, .. } => {
            walk_arguments(arguments, info)
        }
        ast::InlineExpression::TermReference { arguments, .. } => {
            if let Some(arguments) = arguments {
                walk_arguments(arguments, info);
            }
        }
        ast::InlineExpression::Placeable { expression } => walk_expression(expression, info),
        ast::InlineExpression::StringLiteral { .. }
        | ast::InlineExpression::NumberLiteral { .. } => {}
    }
}

fn walk_arguments(arguments: &ast::CallArguments<&str>, info: &mut Message) {
    for argument in &arguments.positional {
        walk_inline(argument, info);
    }
    for argument in &arguments.named {
        walk_inline(&argument.value, info);
    }
}

fn render_pattern(pattern: &ast::Pattern<&str>) -> String {
    let mut text = String::new();
    for element in &pattern.elements {
        match element {
            ast::PatternElement::TextElement { value } => text.push_str(value),
            ast::PatternElement::Placeable {
                expression: ast::Expression::Inline(ast::InlineExpression::VariableReference { id }),
            } => {
                let _ = write!(text, "{{ ${} }}", id.name);
            }
            ast::PatternElement::Placeable { .. } => text.push_str("{ … }"),
        }
    }
    text
}

fn render(english: &Catalog) -> String {
    let mut out = String::from("// @generated by build/i18n.rs from i18n/en.ftl. Do not edit.\n\n");
    for (id, message) in &english.messages {
        for line in message.text.lines() {
            let _ = writeln!(out, "/// {}", line.trim_end());
        }
        let _ = writeln!(out, "///\n/// Fluent message `{id}`.");
        let function = rust_identifier(id);
        if message.variables.is_empty() {
            let _ = writeln!(
                out,
                "pub fn {function}() -> String {{\n    super::format({id:?}, None)\n}}\n"
            );
            continue;
        }
        let parameters: Vec<String> = message
            .variables
            .iter()
            .map(|variable| format!("{}: impl Into<FluentValue<'a>>", rust_identifier(variable)))
            .collect();
        let _ = writeln!(
            out,
            "pub fn {function}<'a>({}) -> String {{\n    let mut args = FluentArgs::with_capacity({});",
            parameters.join(", "),
            message.variables.len()
        );
        for variable in &message.variables {
            let _ = writeln!(
                out,
                "    args.set({variable:?}, {});",
                rust_identifier(variable)
            );
        }
        let _ = writeln!(out, "    super::format({id:?}, Some(&args))\n}}\n");
    }
    let ids: Vec<String> = english
        .messages
        .keys()
        .map(|id| format!("{id:?}"))
        .collect();
    let _ = writeln!(
        out,
        "/// Every message id in en.ftl, for tests.\n#[cfg(test)]\npub(crate) const MESSAGE_IDS: &[&str] = &[{}];",
        ids.join(", ")
    );
    out
}

fn rust_identifier(fluent_name: &str) -> String {
    const KEYWORDS: &[&str] = &[
        "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum",
        "extern", "false", "fn", "for", "gen", "if", "impl", "in", "let", "loop", "match", "mod",
        "move", "mut", "pub", "ref", "return", "static", "struct", "trait", "true", "type",
        "unsafe", "use", "where", "while",
    ];
    let name = fluent_name.replace('-', "_");
    if KEYWORDS.contains(&name.as_str()) {
        format!("r#{name}")
    } else {
        name
    }
}

fn is_kebab_case(id: &str) -> bool {
    id.starts_with(|c: char| c.is_ascii_lowercase())
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn fail(catalog: &Catalog, id: &str, problem: &str) -> ! {
    let line = catalog.messages.get(id).map_or(0, |message| message.line);
    panic!("{}:{line}: `{id}` {problem}", catalog.path.display());
}

fn line_of(source: &str, offset: usize) -> usize {
    source[..offset.min(source.len())].matches('\n').count() + 1
}

/// Byte offset of a slice the parser borrowed from `source`.
fn offset_in(source: &str, slice: &str) -> usize {
    (slice.as_ptr() as usize).saturating_sub(source.as_ptr() as usize)
}
