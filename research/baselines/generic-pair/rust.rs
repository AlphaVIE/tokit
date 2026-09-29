// BENCH_START
struct Pair<T> { left: T, right: T }
fn flip<T>(p: Pair<T>) -> Pair<T> { Pair { left: p.right, right: p.left } }
// BENCH_END

fn main() {
    let mut args = std::env::args().skip(1);
    let kind = args.next().unwrap();
    let left = args.next().unwrap();
    let right = args.next().unwrap();
    match kind.as_str() {
        "i32" => {
            let p = flip(Pair { left: left.parse::<i32>().unwrap(), right: right.parse::<i32>().unwrap() });
            println!("Pair(left:{},right:{})", p.left, p.right);
        }
        "bool" => {
            let p = flip(Pair { left: left.parse::<bool>().unwrap(), right: right.parse::<bool>().unwrap() });
            println!("Pair(left:{},right:{})", p.left, p.right);
        }
        "String" => {
            let p = flip(Pair { left, right });
            println!("Pair(left:{:?},right:{:?})", p.left, p.right);
        }
        _ => panic!("unknown type"),
    }
}
