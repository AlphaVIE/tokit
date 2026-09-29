// BENCH_START
fn sum_positive(xs: &[i32]) -> i32 {
    let mut total = 0;
    for &x in xs {
        if x > 0 { total += x; }
    }
    total
}
// BENCH_END

fn main() {
    let xs: Vec<i32> = std::env::args().skip(1).map(|x| x.parse().unwrap()).collect();
    println!("{}", sum_positive(&xs));
}
