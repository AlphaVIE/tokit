use std::{env, fs, process};

fn main() {
    let args: Vec<String> = env::args().collect();
    let (json, path) = match args.as_slice() {
        [_, command, path] if matches!(command.as_str(), "check" | "run") => (false, path),
        [_, command, flag, path]
            if matches!(command.as_str(), "check" | "run") && flag == "--json" =>
        {
            (true, path)
        }
        _ => {
            eprintln!("usage: tok <check|run> [--json] <file.tok>");
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
    let result = if args[1] == "check" {
        tokit_compiler::check(&source).map(|_| "ok".to_owned())
    } else {
        tokit_compiler::run(&source).map(|value| value.to_string())
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
