import java.util.concurrent.ExecutionException;
import java.util.concurrent.Executors;

public class Main {
    // BENCH_START
    static int squareAsync(int x) throws InterruptedException, ExecutionException {
        try (var executor = Executors.newSingleThreadExecutor()) {
            return executor.submit(() -> x * x).get();
        }
    }
    // BENCH_END

    public static void main(String[] args) {
        try { System.out.println("Ok(" + squareAsync(Integer.parseInt(args[0])) + ")"); }
        catch (InterruptedException | ExecutionException failure) {
            System.out.println("Err(TaskError::Failed)");
        }
    }
}
