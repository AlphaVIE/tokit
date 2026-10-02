using System;

static int CycleSum(int n)
{
    var value = 1;
    var total = 0;

    for (var i = 0; i < n; i += 1)
    {
        checked
        {
            total += value;
            value = value == 3 ? 1 : value + 1;
        }
    }

    return total;
}

if (args.Length != 1)
{
    throw new ArgumentException("invalid arguments");
}

var n = int.Parse(args[0]);
Console.WriteLine($"Ok({CycleSum(n)})");