//! Compact, deterministic facts from a checked Tokit program.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

use crate::ast::{Program, Span, Type};
use crate::diagnostic::escape_json;
use crate::explain::{Facts, visit};
use crate::modules::{LoadedProgram, ModuleInfo};
use crate::sources::SourceMap;

fn string(value: &str) -> String {
    format!("\"{}\"", escape_json(value))
}

fn strings<'a>(values: impl IntoIterator<Item = &'a str>) -> String {
    format!(
        "[{}]",
        values.into_iter().map(string).collect::<Vec<_>>().join(",")
    )
}

fn span(span: Span, multi_source: bool) -> String {
    if multi_source {
        format!("[{},{},{}]", span.source_id.0, span.start, span.end)
    } else {
        format!("[{},{}]", span.start, span.end)
    }
}

fn fields(fields: &[(String, Type)]) -> String {
    format!(
        "[{}]",
        fields
            .iter()
            .map(|(name, ty)| format!("[{},{}]", string(name), string(&ty.to_string())))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn add_visibility(out: &mut String, enabled: bool, public: bool) {
    if enabled {
        assert_eq!(out.pop(), Some('}'));
        write!(out, ",\"public\":{public}}}").expect("writing to String cannot fail");
    }
}

/// Return a source-order JSON index. Call only after static checking succeeds.
pub fn index(program: &Program) -> String {
    index_impl(program, None, None)
}

pub fn index_with_sources(program: &Program, sources: &SourceMap) -> String {
    if sources.len() == 1 {
        index(program)
    } else {
        index_impl(program, Some(sources), None)
    }
}

pub fn index_loaded(loaded: &LoadedProgram) -> String {
    if loaded.sources.len() == 1 {
        index(&loaded.program)
    } else {
        index_impl(
            &loaded.program,
            Some(&loaded.sources),
            Some(&loaded.modules),
        )
    }
}

fn index_impl(
    program: &Program,
    sources: Option<&SourceMap>,
    modules: Option<&[ModuleInfo]>,
) -> String {
    let multi_source = sources.is_some();
    let record_names = program
        .records
        .iter()
        .map(|record| record.name.clone())
        .collect::<BTreeSet<_>>();
    let mut entries = Vec::new();
    for function in &program.functions {
        let mut facts = Facts::with_records(record_names.clone());
        visit(&function.body, &mut facts);
        entries.push((function, facts));
    }

    // A fixed point handles recursion and mutually recursive call cycles.
    let mut effects = entries
        .iter()
        .map(|(function, facts)| (function.name.clone(), facts.effects.clone()))
        .collect::<BTreeMap<_, _>>();
    loop {
        let previous = effects.clone();
        for (function, facts) in &entries {
            let mut reachable = facts.effects.clone();
            for callee in &facts.calls {
                if let Some(callee_effects) = previous.get(callee) {
                    reachable.extend(callee_effects);
                }
            }
            effects.insert(function.name.clone(), reachable);
        }
        if effects == previous {
            break;
        }
    }

    let mut out = if let Some(sources) = sources {
        let paths = (0..sources.len())
            .map(|id| {
                let path = sources
                    .display_path(crate::ast::SourceId(id))
                    .expect("registered source path");
                string(&path.to_string_lossy())
            })
            .collect::<Vec<_>>()
            .join(",");
        let mut header = format!(
            "{{\"version\":{},\"sources\":[{paths}]",
            if modules.is_some() { 3 } else { 2 }
        );
        if let Some(modules) = modules {
            header.push_str(",\"modules\":[");
            for (index, module) in modules.iter().enumerate() {
                if index > 0 {
                    header.push(',');
                }
                let imports = module
                    .imports
                    .iter()
                    .map(|(alias, target)| format!("[{},{}]", string(alias), target.0))
                    .collect::<Vec<_>>()
                    .join(",");
                write!(
                    header,
                    "{{\"source\":{},\"imports\":[{}],\"exports\":{}}}",
                    module.source_id.0,
                    imports,
                    strings(module.exports.iter().map(String::as_str))
                )
                .expect("writing to String cannot fail");
            }
            header.push(']');
        }
        header.push_str(",\"records\":[");
        header
    } else {
        String::from("{\"version\":1,\"records\":[")
    };
    for (index, record) in program.records.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        write!(
            out,
            "{{\"name\":{},\"span\":{},\"params\":{},\"fields\":{}}}",
            string(&record.name),
            span(record.span, multi_source),
            strings(record.type_params.iter().map(String::as_str)),
            fields(&record.fields)
        )
        .expect("writing to String cannot fail");
        add_visibility(&mut out, modules.is_some(), record.public);
    }
    out.push_str("],\"enums\":[");
    for (index, decl) in program.enums.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        let variants = decl
            .variants
            .iter()
            .map(|variant| {
                format!(
                    "[{},{}]",
                    string(&variant.name),
                    variant
                        .payload
                        .as_ref()
                        .map(|ty| string(&ty.to_string()))
                        .unwrap_or_else(|| "null".to_owned())
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        write!(
            out,
            "{{\"name\":{},\"span\":{},\"variants\":[{}]}}",
            string(&decl.name),
            span(decl.span, multi_source),
            variants
        )
        .expect("writing to String cannot fail");
        add_visibility(&mut out, modules.is_some(), decl.public);
    }
    out.push_str("],\"functions\":[");
    for (index, (function, facts)) in entries.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        let calls = facts
            .calls
            .iter()
            .filter(|name| effects.contains_key(*name))
            .map(String::as_str);
        let builtins = facts
            .calls
            .iter()
            .filter(|name| !effects.contains_key(*name))
            .map(String::as_str);
        write!(
            out,
            "{{\"name\":{},\"span\":{},\"type_params\":{},\"params\":{},\"ret\":{},\"calls\":{},\"builtins\":{},\"constructs\":{},\"direct_effects\":{},\"effects\":{}}}",
            string(&function.name),
            span(function.span, multi_source),
            strings(function.type_params.iter().map(String::as_str)),
            fields(&function.params),
            string(&function.ret.to_string()),
            strings(calls),
            strings(builtins),
            strings(facts.constructors.iter().map(String::as_str)),
            strings(facts.effects.iter().copied()),
            strings(effects[&function.name].iter().copied())
        )
        .expect("writing to String cannot fail");
        add_visibility(&mut out, modules.is_some(), function.public);
    }
    out.push_str("]}");
    out
}
