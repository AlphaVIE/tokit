public class Main {
    // BENCH_START
    static int sumPositive(int[] xs) {
        int total = 0;
        for (int x : xs) {
            if (x > 0) total += x;
        }
        return total;
    }
    // BENCH_END

    public static void main(String[] args) {
        int[] xs = new int[args.length];
        for (int i = 0; i < args.length; i++) xs[i] = Integer.parseInt(args[i]);
        System.out.println(sumPositive(xs));
    }
}
