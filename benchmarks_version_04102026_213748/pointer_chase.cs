using System;

static int Walk(int n, int[] next)
{
    var index = 0;
    var total = 0;
    for (var step = 0; step < n; step += 1)
    {
        checked
        {
            index = next[index];
            total += index;
        }
    }
    return total;
}

if (args.Length < 3)
{
    throw new ArgumentException("invalid arguments");
}
var n = int.Parse(args[0]);
var next = Array.ConvertAll(args[1..], int.Parse);
Console.WriteLine($"Ok({Walk(n, next)})");
