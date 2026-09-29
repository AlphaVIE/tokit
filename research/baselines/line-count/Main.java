import java.nio.file.Files;
import java.nio.file.NoSuchFileException;
import java.nio.file.Path;
import java.io.IOException;

public class Main {
    // BENCH_START
    static int lineCount(String path) throws IOException {
        String text = Files.readString(Path.of(path));
        int count = 0;
        for (int i = 0; i < text.length(); i++) if (text.charAt(i) == '\n') count++;
        if (!text.isEmpty() && text.charAt(text.length() - 1) != '\n') count++;
        return count;
    }
    // BENCH_END

    public static void main(String[] args) throws IOException {
        try {
            System.out.println("Ok(" + lineCount(args[0]) + ")");
        } catch (NoSuchFileException error) {
            System.out.println("Err(IoError::NotFound)");
        }
    }
}
