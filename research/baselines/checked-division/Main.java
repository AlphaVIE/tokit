public class Main {
    // BENCH_START
    enum DivError { DivZero }
    sealed interface Result permits Ok, Err {}
    record Ok(int value) implements Result {}
    record Err(DivError error) implements Result {}
    static Result divide(int a, int b) {
        if (b == 0) return new Err(DivError.DivZero);
        return new Ok(a / b);
    }
    // BENCH_END

    public static void main(String[] args) {
        Result result = divide(Integer.parseInt(args[0]), Integer.parseInt(args[1]));
        System.out.println(result instanceof Ok ok ? "Ok(" + ok.value() + ")" : "Err(DivError::DivZero)");
    }
}
