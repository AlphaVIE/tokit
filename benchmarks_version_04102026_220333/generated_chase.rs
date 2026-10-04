fn walk(n: i32, size: i32, stride: i32) -> i32 {
    let mut next = Vec::new();
    for index in 0..size {
        let value = index + stride;
        next.push(if value >= size { value - size } else { value });
    }
    let mut index = 0;
    let mut total = 0;
    let half = size / 2;
    for _ in 0..n {
        index = next[usize::try_from(index).expect("negative index")];
        if index < half {
            total += 1;
        }
    }
    total
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    assert_eq!(args.len(), 3);
    let n: i32 = args[0].parse().expect("invalid n");
    let size: i32 = args[1].parse().expect("invalid size");
    let stride: i32 = args[2].parse().expect("invalid stride");
    println!("Ok({})", walk(n, size, stride));
}
