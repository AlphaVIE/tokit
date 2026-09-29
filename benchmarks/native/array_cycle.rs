fn array_sum(n: i32) -> i32 {
    let xs: Vec<i32> = vec![1, 2, 3];
    let mut i = 0_i32;
    let mut index = 0_i32;
    let mut total = 0_i32;
    while i < n {
        let value = usize::try_from(index)
            .ok()
            .and_then(|position| xs.get(position))
            .copied()
            .expect("array index out of bounds");
        total = total.checked_add(value).expect("i32 overflow");
        let last = i32::try_from(xs.len()).expect("array too long") - 1;
        if index == last {
            index = 0;
        } else {
            index = index.checked_add(1).expect("i32 overflow");
        }
        i = i.checked_add(1).expect("i32 overflow");
    }
    total
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    assert_eq!(args.len(), 1);
    let n: i32 = args[0].parse().expect("invalid i32");
    println!("Ok({})", array_sum(n));
}
