// BENCH_START
enum DivError { DivZero }
fn divide(a: i32, b: i32) -> Result<i32, DivError> {
    if b == 0 { Err(DivError::DivZero) } else { Ok(a / b) }
}
// BENCH_END

fn main() {
    let mut args = std::env::args().skip(1);
    let a: i32 = args.next().unwrap().parse().unwrap();
    let b: i32 = args.next().unwrap().parse().unwrap();
    match divide(a, b) {
        Ok(value) => println!("Ok({value})"),
        Err(DivError::DivZero) => println!("Err(DivError::DivZero)"),
    }
}
