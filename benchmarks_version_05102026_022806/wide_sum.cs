using System;

static long SumValues(int n, long[] values)
{
    var index = 0;
    long total = 0;
    for (var step = 0; step < n; step++)
    {
        checked { total += values[index]; }
        index++;
        if (index == values.Length) index = 0;
    }
    return total;
}

if (args.Length < 3) throw new ArgumentException("invalid arguments");
var n = int.Parse(args[0]);
var values = Array.ConvertAll(args[1..], long.Parse);
Console.WriteLine($"Ok({SumValues(n, values)})");
