fn walk(n: i32, next: Vec<i32>) -> i32 {
    let mut step = 0_i32;
    let mut index = 0_i32;
    let mut total = 0_i32;
    while step < n {
        index = usize::try_from(index)
            .ok()
            .and_then(|at| next.get(at))
            .copied()
            .expect("array index out of bounds");
        total = total.checked_add(index).expect("i32 overflow");
        step = step.checked_add(1).expect("i32 overflow");
    }
    total
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    assert!(args.len() >= 3);
    let n: i32 = args[0].parse().expect("invalid i32");
    let next: Vec<i32> = args[1..]
        .iter()
        .map(|value| value.parse().expect("invalid i32"))
        .collect();
    println!("Ok({})", walk(n, next));
}
