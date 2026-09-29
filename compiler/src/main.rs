use std::{env, fs, process};

fn main() {
    let args: Vec<String> = env::args().collect();
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
        let source = match fs::read_to_string(path) {
            Ok(source) => source,
            Err(error) => {
                eprintln!("could not read {path}: {error}");
                process::exit(2);
            }
        };
        let program = match tokit_compiler::check(&source) {
            Ok(program) => program,
            Err(diagnostic) => {
                eprintln!("{}", diagnostic.display(&source));
                process::exit(1);
            }
        };
        if let Err(error) = tokit_compiler::native::build(&program, &source, output.as_ref()) {
            eprintln!("{error}");
            process::exit(1);
        }
        println!("{output}");
        return;
    }
    let (json, path) = match args.as_slice() {
        [_, command, path] if matches!(command.as_str(), "check" | "run" | "explain" | "stats") => {
            (false, path)
        }
        [_, command, flag, path]
            if matches!(command.as_str(), "check" | "run") && flag == "--json" =>
        {
            (true, path)
        }
        _ => {
            eprintln!(
                "usage: tok <check|run> [--json] <file.tok> | tok <explain|stats> <file.tok> | tok fmt [--check|--write] <file.tok> | tok build <file.tok> -o <output>"
            );
            process::exit(2);
        }
    };
    let source = match fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("could not read {path}: {error}");
            process::exit(2);
        }
    };
    let result = match args[1].as_str() {
        "check" => tokit_compiler::check(&source).map(|_| "ok".to_owned()),
        "explain" => {
            tokit_compiler::check(&source).map(|program| tokit_compiler::explain::explain(&program))
        }
        "stats" => tokit_compiler::check(&source)
            .map(|program| tokit_compiler::stats::measure(&source, &program).json()),
        _ => tokit_compiler::run(&source).map(|value| value.to_string()),
    };
    match result {
        Ok(output) if json => println!(
            "{{\"ok\":true,\"result\":\"{}\"}}",
            tokit_compiler::diagnostic::escape_json(&output)
        ),
        Ok(output) => println!("{output}"),
        Err(diagnostic) if json => {
            println!("{{\"ok\":false,\"error\":{}}}", diagnostic.json(&source));
            process::exit(1);
        }
        Err(diagnostic) => {
            eprintln!("{}", diagnostic.display(&source));
            process::exit(1);
        }
    }
}
