// BENCH_START
fn square_async(x: i32) -> Result<i32, &'static str> {
    std::thread::spawn(move || x * x).join().map_err(|_| "TaskError::Failed")
}
// BENCH_END

fn main() {
    let x: i32 = std::env::args().nth(1).unwrap().parse().unwrap();
    match square_async(x) {
        Ok(value) => println!("Ok({value})"),
        Err(error) => println!("Err({error})"),
    }
}
