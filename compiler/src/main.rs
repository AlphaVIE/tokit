use std::{
    collections::HashMap,
    env, fs,
    io::Write,
    path::{Component, Path, PathBuf},
    process,
    time::{SystemTime, UNIX_EPOCH},
};

use tokit_compiler::diagnostic::Diagnostic;
use tokit_compiler::sources::SourceMap;

fn display_diagnostic(diagnostic: &Diagnostic, sources: &SourceMap) -> String {
    if sources.len() == 1 {
        diagnostic.display(
            &sources
                .get(diagnostic.span.source_id)
                .expect("registered source")
                .text,
        )
    } else {
        diagnostic.display_with_sources(sources)
    }
}

fn json_diagnostic(diagnostic: &Diagnostic, sources: &SourceMap) -> String {
    if sources.len() == 1 {
        diagnostic.json(
            &sources
                .get(diagnostic.span.source_id)
                .expect("registered source")
                .text,
        )
    } else {
        diagnostic.json_with_sources(sources)
    }
}

fn package_entry_root(path: &str) -> Result<std::path::PathBuf, String> {
    match fs::canonicalize(path) {
        Ok(entry) if entry.is_file() && entry.extension().is_some_and(|ext| ext == "tok") => {
            Ok(entry.parent().expect("entry has parent").to_path_buf())
        }
        Ok(_) => Err("package command needs an entry .tok file".to_owned()),
        Err(error) => Err(format!("cannot open entry file: {error}")),
    }
}

fn run_command(args: &[String]) {
    let mut index = 0;
    let mut json = false;
    let mut read_root = None;
    let mut write_root = None;
    let mut net = None;
    while let Some(flag) = args.get(index) {
        match flag.as_str() {
            "--json" if !json => {
                json = true;
                index += 1;
            }
            "--allow-read" if read_root.is_none() && args.get(index + 1).is_some() => {
                read_root = args.get(index + 1).map(String::as_str);
                index += 2;
            }
            "--allow-write" if write_root.is_none() && args.get(index + 1).is_some() => {
                write_root = args.get(index + 1).map(String::as_str);
                index += 2;
            }
            "--allow-net" if net.is_none() && args.get(index + 1).is_some() => {
                net = args.get(index + 1).map(String::as_str);
                index += 2;
            }
            _ => break,
        }
    }
    let Some(path) = args.get(index) else {
        eprintln!(
            "usage: tok run [--json] [--allow-read <path>] [--allow-write <path>] [--allow-net <host:port|*>] <file.tok> [-- arguments...]"
        );
        process::exit(2);
    };
    let remaining = &args[index + 1..];
    let program_args = match remaining {
        [] => &[][..],
        [separator, rest @ ..] if separator == "--" => rest,
        _ => {
            eprintln!(
                "usage: tok run [--json] [--allow-read <path>] [--allow-write <path>] <file.tok> [-- arguments...]"
            );
            process::exit(2);
        }
    };
    let loaded = match tokit_compiler::modules::load(Path::new(path)) {
        Ok(loaded) => loaded,
        Err(error) if json => {
            println!(
                "{{\"ok\":false,\"error\":{}}}",
                json_diagnostic(&error.diagnostic, &error.sources)
            );
            process::exit(1);
        }
        Err(error) => {
            eprintln!("{}", display_diagnostic(&error.diagnostic, &error.sources));
            process::exit(1);
        }
    };
    let result = tokit_compiler::interpreter::run_with_grants(
        &loaded.program,
        tokit_compiler::interpreter::Grants {
            read: read_root.map(Path::new),
            write: write_root.map(Path::new),
            net,
        },
        program_args,
    );
    match result {
        Err(diagnostic) if tokit_compiler::interpreter::exit_status(&diagnostic).is_some() => {
            let status = tokit_compiler::interpreter::exit_status(&diagnostic).unwrap_or(1);
            if json {
                println!("{{\"ok\":true,\"exit\":{status}}}");
            }
            process::exit(status);
        }
        Ok(value) if json => println!(
            "{{\"ok\":true,\"result\":\"{}\"}}",
            tokit_compiler::diagnostic::escape_json(&value.to_string())
        ),
        Ok(tokit_compiler::interpreter::Value::Unit) => {}
        Ok(value) => println!("{value}"),
        Err(diagnostic) if json => {
            println!(
                "{{\"ok\":false,\"error\":{}}}",
                json_diagnostic(&diagnostic, &loaded.sources)
            );
            process::exit(1);
        }
        Err(diagnostic) => {
            eprintln!("{}", display_diagnostic(&diagnostic, &loaded.sources));
            process::exit(1);
        }
    }
}

fn test_command(args: &[String]) {
    let mut index = 0;
    let mut read_root = None;
    let mut write_root = None;
    while let Some(flag) = args.get(index) {
        match flag.as_str() {
            "--allow-read" if read_root.is_none() && args.get(index + 1).is_some() => {
                read_root = args.get(index + 1).map(String::as_str);
                index += 2;
            }
            "--allow-write" if write_root.is_none() && args.get(index + 1).is_some() => {
                write_root = args.get(index + 1).map(String::as_str);
                index += 2;
            }
            _ => break,
        }
    }
    let [path] = &args[index..] else {
        eprintln!("usage: tok test [--allow-read <path>] [--allow-write <path>] <file.tok>");
        process::exit(2);
    };
    let loaded = match tokit_compiler::modules::load(Path::new(path)) {
        Ok(loaded) => loaded,
        Err(error) => {
            eprintln!("{}", display_diagnostic(&error.diagnostic, &error.sources));
            process::exit(1);
        }
    };
    match tokit_compiler::test_runner::run_loaded(
        &loaded.program,
        &loaded.sources,
        read_root.map(Path::new),
        write_root.map(Path::new),
    ) {
        Ok(report) => {
            println!("{}", report.display());
            if report.failed() > 0 {
                process::exit(1);
            }
        }
        Err(diagnostic) => {
            eprintln!("{}", display_diagnostic(&diagnostic, &loaded.sources));
            process::exit(1);
        }
    }
}

fn write_patch(target: &Path, original: &str, updated: &str) -> Result<(), String> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let temporary = target.with_extension(format!("tok.patch-{}-{nonce}", process::id()));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|error| format!("cannot stage patch: {error}"))?;
    let result = (|| {
        let permissions = fs::metadata(target)
            .map_err(|error| format!("cannot inspect patch target: {error}"))?
            .permissions();
        file.write_all(updated.as_bytes())
            .and_then(|()| file.sync_all())
            .map_err(|error| format!("cannot stage patch: {error}"))?;
        drop(file);
        fs::set_permissions(&temporary, permissions)
            .map_err(|error| format!("cannot stage patch: {error}"))?;
        if fs::read_to_string(target).map_err(|error| error.to_string())? != original {
            return Err("patch target changed during validation".to_owned());
        }
        fs::rename(&temporary, target).map_err(|error| format!("cannot commit patch: {error}"))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn patch_command(entry: &Path, request_path: &Path, write: bool) -> Result<String, String> {
    let request_text = fs::read_to_string(request_path)
        .map_err(|error| format!("cannot read patch request: {error}"))?;
    let request: tokit_compiler::ai_patch::Request = serde_json::from_str(&request_text)
        .map_err(|error| format!("invalid patch request: {error}"))?;
    if request.version != 1 {
        return Err("unsupported patch version".to_owned());
    }
    let entry = fs::canonicalize(entry).map_err(|error| format!("cannot open entry: {error}"))?;
    let root = entry.parent().expect("entry has parent");
    let target_relative = Path::new(&request.target);
    if target_relative.as_os_str().is_empty()
        || !target_relative
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
    {
        return Err("patch target must be a plain relative path".to_owned());
    }
    let target = fs::canonicalize(root.join(target_relative))
        .map_err(|error| format!("cannot open patch target: {error}"))?;
    if !target.starts_with(root)
        || target
            .extension()
            .is_none_or(|extension| extension != "tok")
    {
        return Err("patch target must be a .tok source inside the entry directory".to_owned());
    }
    let original = fs::read_to_string(&target)
        .map_err(|error| format!("cannot read patch target: {error}"))?;
    let updated =
        tokit_compiler::ai_patch::apply(&original, &request.edits).map_err(|error| error.json())?;
    let mut overrides = HashMap::<PathBuf, String>::new();
    overrides.insert(target.clone(), updated.clone());
    let loaded = tokit_compiler::modules::load_with_overrides(&entry, &overrides)
        .map_err(|error| error.json())?;
    if !(0..loaded.sources.len()).any(|id| {
        loaded
            .sources
            .get(tokit_compiler::ast::SourceId(id))
            .is_some_and(|source| source.path == target && source.display_override.is_none())
    }) {
        return Err("patch target must be a local, unpinned module in the loaded graph".to_owned());
    }
    if write {
        write_patch(&target, &original, &updated)?;
        Ok(
            serde_json::json!({"ok":true,"target":request.target,"edits":request.edits.len()})
                .to_string(),
        )
    } else {
        Ok(updated)
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() == 2 && args[1] == "lsp" {
        let stdin = std::io::stdin();
        let stdout = std::io::stdout();
        match tokit_compiler::lsp::serve(&mut stdin.lock(), &mut stdout.lock()) {
            Ok(true) => return,
            Ok(false) => process::exit(1),
            Err(error) => {
                eprintln!("language server I/O error: {error}");
                process::exit(1);
            }
        }
    }
    if args.get(1).is_some_and(|command| command == "run") {
        run_command(&args[2..]);
        return;
    }
    if args.get(1).is_some_and(|command| command == "test") {
        test_command(&args[2..]);
        return;
    }
    let patch_index = match args.as_slice() {
        [_, command, path] if command == "ai-patch-index" => Some((path, None)),
        [_, command, path, name] if command == "ai-patch-index" => {
            Some((path, Some(name.as_str())))
        }
        _ => None,
    };
    if let Some((path, name)) = patch_index {
        let result = fs::read_to_string(path)
            .map_err(|error| error.to_string())
            .and_then(|source| {
                tokit_compiler::ai_patch::index_function(&source, name)
                    .map_err(|error| error.json())
            });
        match result {
            Ok(index) => println!("{index}"),
            Err(error) => {
                eprintln!("{error}");
                process::exit(1);
            }
        }
        return;
    }
    let patch = match args.as_slice() {
        [_, command, entry, request] if command == "ai-patch" => Some((entry, request, false)),
        [_, command, entry, request, flag] if command == "ai-patch" && flag == "--write" => {
            Some((entry, request, true))
        }
        _ => None,
    };
    if let Some((entry, request, write)) = patch {
        match patch_command(Path::new(entry), Path::new(request), write) {
            Ok(output) => println!("{output}"),
            Err(error) => {
                if serde_json::from_str::<serde_json::Value>(&error).is_ok() {
                    eprintln!("{error}");
                } else {
                    eprintln!("{}", serde_json::json!({"ok":false,"message":error}));
                }
                process::exit(1);
            }
        }
        return;
    }
    if let [_, command, path] = args.as_slice()
        && command == "pkg-hash"
    {
        match tokit_compiler::packages::hash_path(Path::new(path)) {
            Ok(digest) => println!("{digest}"),
            Err(error) => {
                eprintln!("{error}");
                process::exit(1);
            }
        }
        return;
    }
    if let [_, command, path] = args.as_slice()
        && command == "lock"
    {
        let root = match package_entry_root(path) {
            Ok(root) => root,
            Err(error) => {
                eprintln!("{error}");
                process::exit(2);
            }
        };
        match tokit_compiler::packages::write_lock(&root) {
            Ok(lock) => println!("{}", lock.display()),
            Err(error) => {
                eprintln!("{error}");
                process::exit(1);
            }
        }
        return;
    }
    if args.get(1).is_some_and(|command| command == "repl") {
        let stdin = std::io::stdin();
        let mut input = stdin.lock();
        let mut output = std::io::stdout();
        if let Err(error) = tokit_compiler::tools::repl(&mut input, &mut output) {
            eprintln!("{error}");
            process::exit(1);
        }
        return;
    }
    if args
        .get(1)
        .is_some_and(|command| matches!(command.as_str(), "doc" | "lint" | "bench"))
    {
        tools_command(&args[1..]);
        return;
    }
    // Project commands that work in the current directory.
    match args.as_slice() {
        [_, command, name] if command == "new" => {
            exit_with(tokit_compiler::packages::new_project(Path::new(name)));
        }
        [_, command, spec] if command == "add" => {
            exit_with(tokit_compiler::packages::add_registry(Path::new("."), spec));
        }
        [_, command, name] if command == "rm" => {
            exit_with(tokit_compiler::packages::remove_local(Path::new("."), name));
        }
        [_, command, rest @ ..] if command == "search" && rest.len() <= 1 => {
            let term = rest.first().map(String::as_str).unwrap_or("");
            for package in tokit_compiler::registry::packages() {
                if package.name.contains(term) || package.description.contains(term) {
                    println!(
                        "{}@{}  {}",
                        package.name, package.version, package.description
                    );
                }
            }
            return;
        }
        _ => {}
    }
    if args
        .get(1)
        .is_some_and(|command| command == "add" || command == "rm")
    {
        let (path, name, dependency_path, dependency_entry) = match args.as_slice() {
            [_, command, path, name, dependency_path] if command == "add" => {
                (path, name, Some(dependency_path.as_str()), None)
            }
            [
                _,
                command,
                path,
                name,
                dependency_path,
                flag,
                dependency_entry,
            ] if command == "add" && flag == "--entry" => (
                path,
                name,
                Some(dependency_path.as_str()),
                Some(dependency_entry.as_str()),
            ),
            [_, command, path, name] if command == "rm" => (path, name, None, None),
            _ => {
                eprintln!(
                    "usage: tok add <entry.tok> <name> <relative-path> [--entry <relative.tok>] | tok rm <entry.tok> <name>"
                );
                process::exit(2);
            }
        };
        let root = match package_entry_root(path) {
            Ok(root) => root,
            Err(error) => {
                eprintln!("{error}");
                process::exit(2);
            }
        };
        let result = match dependency_path {
            Some(dependency_path) => {
                tokit_compiler::packages::add_local(&root, name, dependency_path, dependency_entry)
            }
            None => tokit_compiler::packages::remove_local(&root, name),
        };
        match result {
            Ok(lock) => println!("{}", lock.display()),
            Err(error) => {
                eprintln!("{error}");
                process::exit(1);
            }
        }
        return;
    }
    if let [_, command, path] = args.as_slice()
        && command == "tokens"
    {
        let source = fs::read_to_string(path).unwrap_or_else(|error| {
            eprintln!("could not read {path}: {error}");
            process::exit(2);
        });
        let listing = tokit_compiler::lexer::listing(&source);
        println!("{listing}");
        if listing.starts_with("error ") {
            process::exit(1);
        }
        return;
    }
    let compact = match args.as_slice() {
        [_, command, path] if command == "compact" => Some((path, false, "all")),
        [_, command, flag, path] if command == "compact" && flag == "--write" => {
            Some((path, true, "all"))
        }
        [_, command, mode, path]
            if command == "compact"
                && matches!(
                    mode.as_str(),
                    "--functions-only" | "--types-only" | "--blocks-only"
                ) =>
        {
            Some((path, false, mode.as_str()))
        }
        [_, command, mode, flag, path]
            if command == "compact"
                && matches!(
                    mode.as_str(),
                    "--functions-only" | "--types-only" | "--blocks-only"
                )
                && flag == "--write" =>
        {
            Some((path, true, mode.as_str()))
        }
        _ => None,
    };
    if let Some((path, write, mode)) = compact {
        let source = fs::read_to_string(path).unwrap_or_else(|error| {
            eprintln!("could not read {path}: {error}");
            process::exit(2);
        });
        let result = match mode {
            "--functions-only" => tokit_compiler::format::compact_functions(&source),
            "--types-only" => tokit_compiler::format::compact_integer_types(&source),
            "--blocks-only" => tokit_compiler::format::compact_blocks(&source),
            _ => tokit_compiler::format::compact_functions(&source)
                .and_then(|text| tokit_compiler::format::compact_integer_types(&text))
                .and_then(|text| tokit_compiler::format::compact_blocks(&text)),
        }
        .unwrap_or_else(|error| {
            eprintln!("{}", error.display(&source));
            process::exit(1);
        });
        if write {
            fs::write(path, result).unwrap_or_else(|error| {
                eprintln!("could not write {path}: {error}");
                process::exit(2);
            });
        } else {
            print!("{result}");
        }
        return;
    }
    let fmt = match args.as_slice() {
        [_, command, path] if command == "fmt" => Some(("print", path)),
        [_, command, mode, path]
            if command == "fmt" && matches!(mode.as_str(), "--check" | "--write") =>
        {
            Some((mode.as_str(), path))
        }
        _ => None,
    };
    if let Some((mode, path)) = fmt {
        let source = match fs::read_to_string(path) {
            Ok(source) => source,
            Err(error) => {
                eprintln!("could not read {path}: {error}");
                process::exit(2);
            }
        };
        let formatted = match tokit_compiler::format::format(&source) {
            Ok(formatted) => formatted,
            Err(diagnostic) => {
                eprintln!("{}", diagnostic.display(&source));
                process::exit(1);
            }
        };
        match mode {
            "--check" if source != formatted => {
                eprintln!("{path} is not canonically formatted");
                process::exit(1);
            }
            "--check" => {}
            "--write" => {
                if source != formatted
                    && let Err(error) = fs::write(path, formatted)
                {
                    eprintln!("could not write {path}: {error}");
                    process::exit(2);
                }
            }
            _ => print!("{formatted}"),
        }
        return;
    }
    if let [_, command, path, flag, output] = args.as_slice()
        && command == "build"
        && flag == "-o"
    {
        let loaded = match tokit_compiler::modules::load(Path::new(path)) {
            Ok(loaded) => loaded,
            Err(error) => {
                eprintln!("{}", display_diagnostic(&error.diagnostic, &error.sources));
                process::exit(1);
            }
        };
        if let Err(error) = tokit_compiler::native::build_with_sources(
            &loaded.program,
            &loaded.sources,
            output.as_ref(),
        ) {
            eprintln!("{error}");
            process::exit(1);
        }
        println!("{output}");
        return;
    }
    let (json, path) = match args.as_slice() {
        [_, command, path]
            if matches!(
                command.as_str(),
                "check" | "explain" | "stats" | "ai-index" | "expand"
            ) =>
        {
            (false, path)
        }
        [_, command, flag, path] if command == "check" && flag == "--json" => (true, path),
        [_, command, flag, path] if command == "explain" && flag == "--pseudo" => (false, path),
        _ => {
            eprintln!(
                "usage: tok check [--json] <file.tok> | tok run [--json] [--allow-read <path>] [--allow-write <path>] <file.tok> [-- arguments...] | tok test [--allow-read <path>] [--allow-write <path>] <file.tok> | tok <explain [--pseudo]|expand|stats|ai-index|tokens> <file.tok> | tok doc [--private] <file.tok> | tok lint [--json] <file.tok> | tok bench [--iterations N] <file.tok> | tok repl | tok fmt [--check|--write] <file.tok> | tok build <file.tok> -o <output> | tok lsp | tok pkg-hash <file.tok|directory> | tok lock <entry.tok> | tok add <entry.tok> <name> <relative-path> [--entry <relative.tok>] | tok rm <entry.tok> <name>"
            );
            process::exit(2);
        }
    };
    let loaded = match tokit_compiler::modules::load(Path::new(path)) {
        Ok(loaded) => loaded,
        Err(error) if json => {
            println!(
                "{{\"ok\":false,\"error\":{}}}",
                json_diagnostic(&error.diagnostic, &error.sources)
            );
            process::exit(1);
        }
        Err(error) => {
            eprintln!("{}", display_diagnostic(&error.diagnostic, &error.sources));
            process::exit(1);
        }
    };
    let output = match args[1].as_str() {
        "check" => "ok".to_owned(),
        "explain" if args.iter().any(|arg| arg == "--pseudo") => {
            tokit_compiler::expand::pseudocode(&loaded.program)
        }
        "explain" => tokit_compiler::explain::explain(&loaded.program),
        "expand" => tokit_compiler::expand::expand(&loaded.program),
        "stats" => tokit_compiler::stats::measure_sources(&loaded.sources, &loaded.program).json(),
        "ai-index" => tokit_compiler::ai_index::index_loaded(&loaded),
        _ => unreachable!("run is handled before this command match"),
    };
    if json {
        println!(
            "{{\"ok\":true,\"result\":\"{}\"}}",
            tokit_compiler::diagnostic::escape_json(&output)
        );
    } else {
        println!("{output}");
    }
}

/// Print a written path, or report the error and exit unsuccessfully.
fn exit_with(result: Result<std::path::PathBuf, String>) -> ! {
    match result {
        Ok(path) => {
            println!("{}", path.display());
            process::exit(0);
        }
        Err(error) => {
            eprintln!("{error}");
            process::exit(1);
        }
    }
}

/// `tok doc [--private] file.tok`, `tok lint [--json] file.tok`,
/// `tok bench [--iterations N] file.tok`.
fn tools_command(args: &[String]) {
    let command = args[0].as_str();
    let mut flags = Vec::new();
    let mut iterations = 1000;
    let mut path = None;
    let mut index = 1;
    while let Some(arg) = args.get(index) {
        match arg.as_str() {
            "--iterations" => {
                iterations = args
                    .get(index + 1)
                    .and_then(|value| value.parse::<u32>().ok())
                    .unwrap_or_else(|| {
                        eprintln!("--iterations needs a positive number");
                        process::exit(2);
                    });
                index += 2;
                continue;
            }
            flag if flag.starts_with("--") => flags.push(flag.to_owned()),
            other => path = Some(other.to_owned()),
        }
        index += 1;
    }
    let Some(path) = path else {
        eprintln!(
            "usage: tok doc [--private] <file.tok> | tok lint [--json] <file.tok> | tok bench [--iterations N] <file.tok>"
        );
        process::exit(2);
    };
    let loaded = match tokit_compiler::modules::load(Path::new(&path)) {
        Ok(loaded) => loaded,
        Err(error) => {
            eprintln!("{}", display_diagnostic(&error.diagnostic, &error.sources));
            process::exit(1);
        }
    };
    match command {
        "doc" => {
            let source = fs::read_to_string(&path).unwrap_or_default();
            let entry = tokit_compiler::parse(&source).unwrap_or_else(|_| loaded.program.clone());
            let title = Path::new(&path)
                .file_stem()
                .map_or("module".into(), |stem| stem.to_string_lossy());
            print!(
                "{}",
                tokit_compiler::doc::document(
                    &entry,
                    &source,
                    &title,
                    flags.iter().any(|f| f == "--private")
                )
            );
        }
        "lint" => {
            let warnings = tokit_compiler::lint::lint(&loaded.program);
            if flags.iter().any(|flag| flag == "--json") {
                let items = warnings
                    .iter()
                    .map(|warning| json_diagnostic(warning, &loaded.sources))
                    .collect::<Vec<_>>()
                    .join(",");
                println!("{{\"ok\":true,\"warnings\":[{items}]}}");
            } else {
                for warning in &warnings {
                    println!("{}", display_diagnostic(warning, &loaded.sources));
                }
            }
        }
        _ => {
            let (program, names) =
                match tokit_compiler::tools::bench_program(loaded.program.clone(), iterations) {
                    Ok(result) => result,
                    Err(error) => {
                        eprintln!("{error}");
                        process::exit(1);
                    }
                };
            let binary = std::env::temp_dir().join(format!(
                "tok-bench-{}{}",
                process::id(),
                std::env::consts::EXE_SUFFIX
            ));
            match tokit_compiler::native::build_with_sources(&program, &loaded.sources, &binary) {
                Ok(()) => {
                    let status = process::Command::new(&binary).status();
                    let _ = fs::remove_file(&binary);
                    if !status.is_ok_and(|status| status.success()) {
                        process::exit(1);
                    }
                }
                Err(error) => {
                    eprintln!(
                        "native build unavailable ({}); timing in the interpreter",
                        error.lines().next().unwrap_or("")
                    );
                    if let Err(error) = tokit_compiler::interpreter::run(&program) {
                        eprintln!("{}", display_diagnostic(&error, &loaded.sources));
                        process::exit(1);
                    }
                }
            }
            let _ = names;
        }
    }
}
