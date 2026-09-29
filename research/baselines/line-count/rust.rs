use std::io;

// BENCH_START
fn line_count(path: &str) -> io::Result<i32> {
    let text = std::fs::read_to_string(path)?;
    Ok(i32::try_from(text.lines().count()).unwrap())
}
// BENCH_END

fn main() {
    let path = std::env::args().nth(1).unwrap();
    match line_count(&path) {
        Ok(count) => println!("Ok({count})"),
        Err(error) if error.kind() == io::ErrorKind::NotFound => println!("Err(IoError::NotFound)"),
        Err(error) => panic!("{error}"),
    }
}
