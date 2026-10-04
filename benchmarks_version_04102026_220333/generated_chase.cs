using System;

static int Walk(int n, int size, int stride)
{
    var next = new int[size];
    for (var i = 0; i < size; i++)
    {
        var value = i + stride;
        next[i] = value >= size ? value - size : value;
    }
    var index = 0;
    var total = 0;
    var half = size / 2;
    for (var step = 0; step < n; step++)
    {
        index = next[index];
        if (index < half) total++;
    }
    return total;
}

if (args.Length != 3) throw new ArgumentException("invalid arguments");
Console.WriteLine($"Ok({Walk(int.Parse(args[0]), int.Parse(args[1]), int.Parse(args[2]))})");
