use std::{env, fs, path::Path, process};

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

fn run_command(args: &[String]) {
    let mut index = 0;
    let mut json = false;
    let mut read_root = None;
    let mut write_root = None;
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
            _ => break,
        }
    }
    let Some(path) = args.get(index) else {
        eprintln!(
            "usage: tok run [--json] [--allow-read <path>] [--allow-write <path>] <file.tok> [-- arguments...]"
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
    let result = tokit_compiler::interpreter::run_with_capabilities(
        &loaded.program,
        read_root.map(Path::new),
        write_root.map(Path::new),
        program_args,
    );
    match result {
        Ok(value) if json => println!(
            "{{\"ok\":true,\"result\":\"{}\"}}",
            tokit_compiler::diagnostic::escape_json(&value.to_string())
        ),
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

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.get(1).is_some_and(|command| command == "run") {
        run_command(&args[2..]);
        return;
    }
    if args.get(1).is_some_and(|command| command == "test") {
        test_command(&args[2..]);
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
            if matches!(command.as_str(), "check" | "explain" | "stats" | "ai-index") =>
        {
            (false, path)
        }
        [_, command, flag, path] if command == "check" && flag == "--json" => (true, path),
        _ => {
            eprintln!(
                "usage: tok check [--json] <file.tok> | tok run [--json] [--allow-read <path>] [--allow-write <path>] <file.tok> [-- arguments...] | tok test [--allow-read <path>] [--allow-write <path>] <file.tok> | tok <explain|stats|ai-index> <file.tok> | tok fmt [--check|--write] <file.tok> | tok build <file.tok> -o <output>"
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
        "explain" => tokit_compiler::explain::explain(&loaded.program),
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
