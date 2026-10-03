using System;

static int ArraySum(int n, int[] values)
{
    var index = 0;
    var total = 0;

    for (var i = 0; i < n; i += 1)
    {
        checked
        {
            total += values[index];
            index = index == values.Length - 1 ? 0 : index + 1;
        }
    }

    return total;
}

if (args.Length != 4)
{
    throw new ArgumentException("invalid arguments");
}

var n = int.Parse(args[0]);
var values = new[] { int.Parse(args[1]), int.Parse(args[2]), int.Parse(args[3]) };
Console.WriteLine($"Ok({ArraySum(n, values)})");
