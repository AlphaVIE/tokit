public class Main {
    // BENCH_START
    record Pair<T>(T left, T right) {}
    static <T> Pair<T> flip(Pair<T> p) { return new Pair<>(p.right(), p.left()); }
    // BENCH_END

    public static void main(String[] args) {
        switch (args[0]) {
            case "i32" -> {
                var p = flip(new Pair<>(Integer.parseInt(args[1]), Integer.parseInt(args[2])));
                System.out.println("Pair(left:" + p.left() + ",right:" + p.right() + ")");
            }
            case "bool" -> {
                var p = flip(new Pair<>(Boolean.parseBoolean(args[1]), Boolean.parseBoolean(args[2])));
                System.out.println("Pair(left:" + p.left() + ",right:" + p.right() + ")");
            }
            case "String" -> {
                var p = flip(new Pair<>(args[1], args[2]));
                System.out.println("Pair(left:\"" + p.left() + "\",right:\"" + p.right() + "\")");
            }
            default -> throw new IllegalArgumentException("unknown type");
        }
    }
}
