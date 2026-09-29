use std::{env, fs, path::Path, process};

fn run_command(args: &[String]) {
    let mut index = 0;
    let mut json = false;
    let mut root = None;
    while let Some(flag) = args.get(index) {
        match flag.as_str() {
            "--json" if !json => {
                json = true;
                index += 1;
            }
            "--allow-read" if root.is_none() => {
                root = args.get(index + 1).map(String::as_str);
                if root.is_none() {
                    eprintln!(
                        "usage: tok run [--json] [--allow-read <path>] <file.tok> [-- arguments...]"
                    );
                    process::exit(2);
                }
                index += 2;
            }
            _ => break,
        }
    }
    let Some(path) = args.get(index) else {
        eprintln!("usage: tok run [--json] [--allow-read <path>] <file.tok> [-- arguments...]");
        process::exit(2);
    };
    let remaining = &args[index + 1..];
    let program_args = match remaining {
        [] => &[][..],
        [separator, rest @ ..] if separator == "--" => rest,
        _ => {
            eprintln!("usage: tok run [--json] [--allow-read <path>] <file.tok> [-- arguments...]");
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
    let result = tokit_compiler::run_with_runtime_args(&source, root.map(Path::new), program_args);
    match result {
        Ok(value) if json => println!(
            "{{\"ok\":true,\"result\":\"{}\"}}",
            tokit_compiler::diagnostic::escape_json(&value.to_string())
        ),
        Ok(value) => println!("{value}"),
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

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.get(1).is_some_and(|command| command == "run") {
        run_command(&args[2..]);
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
        [_, command, path] if matches!(command.as_str(), "check" | "explain" | "stats") => {
            (false, path)
        }
        [_, command, flag, path] if command == "check" && flag == "--json" => (true, path),
        _ => {
            eprintln!(
                "usage: tok check [--json] <file.tok> | tok run [--json] [--allow-read <path>] <file.tok> [-- arguments...] | tok <explain|stats> <file.tok> | tok fmt [--check|--write] <file.tok> | tok build <file.tok> -o <output>"
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
        _ => unreachable!("run is handled before this command match"),
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
