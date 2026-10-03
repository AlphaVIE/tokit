fn cycle_sum(n: i32) -> i32 {
    let mut value = 1_i32;
    let mut total = 0_i32;
    for _ in 0..n {
        total = total.checked_add(value).expect("i32 overflow");
        value = if value == 3 {
            1
        } else {
            value.checked_add(1).expect("i32 overflow")
        };
    }
    total
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    assert_eq!(args.len(), 1);
    let n: i32 = args[0].parse().expect("invalid i32");
    println!("Ok({})", cycle_sum(n));
}