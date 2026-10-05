fn sum(n: i32, values: Vec<i64>) -> i64 {
    let mut index = 0;
    let mut total = 0_i64;
    for _ in 0..n {
        total = total.checked_add(values[index]).expect("i64 overflow");
        index += 1;
        if index == values.len() {
            index = 0;
        }
    }
    total
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    assert!(args.len() >= 3);
    let n: i32 = args[0].parse().expect("invalid iteration count");
    let values: Vec<i64> = args[1..]
        .iter()
        .map(|value| value.parse().expect("invalid i64"))
        .collect();
    println!("Ok({})", sum(n, values));
}
