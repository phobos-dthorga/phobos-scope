using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.IO;
using System.Runtime.CompilerServices;
using System.Text.Json;
using System.Threading;
using Phobos.Scope.Recording;

if (args.Length == 2 && args[0] == "--benchmark")
{
    Benchmark.Run(args[1]);
    return;
}
if (args.Length != 1) throw new ArgumentException("Usage: Phobos.Scope.Sample NEW_CAPTURE.json | --benchmark NEW_REPORT.json");
var recorder = new Recorder();
var update = recorder.RegisterOperation("sample.update", "workload");
var scan = recorder.RegisterOperation("sample.scan", "workload");
var queue = recorder.RegisterCounter("sample.queue", "workload", "items", CounterKind.Gauge);
var processed = recorder.RegisterCounter("sample.processed", "workload", "items", CounterKind.Cumulative);
var hits = recorder.RegisterCounter("sample.cache_hits", "workload", "hits", CounterKind.Increment);
var speed = recorder.RegisterContext("sample.game_speed", "context");
var panel = recorder.RegisterContext("sample.panel_visible", "context");
recorder.Start(new CaptureOptions { Metadata = new Dictionary<string, string> {
    ["workload"] = "Standalone synthetic workload; no game data", ["sample_version"] = "0.1.0"
} });
recorder.Context(speed, "1x"); recorder.Context(panel, "false");
for (var i = 0; i < 120; i++)
{
    if (i == 60) { recorder.Context(speed, "4x"); recorder.Context(panel, "true"); }
    using (recorder.Measure(update))
    {
        using (recorder.Measure(scan)) { Thread.SpinWait(i < 60 ? 10_000 : 30_000); }
        recorder.Sample(queue, i % 12);
        recorder.Sample(processed, i * 8);
        recorder.Sample(hits, 3);
    }
    Thread.Sleep(4);
}
recorder.Stop()!.Export(args[0]);
Console.WriteLine("Synthetic capture written to " + args[0]);

internal static class Benchmark
{
    private const int Iterations = 10_000;
    private const int Repetitions = 7;
    private static int sink;
    [MethodImpl(MethodImplOptions.NoInlining)]
    private static int Work(int value)
    {
        for (var i = 0; i < 32; i++) value = unchecked(value * 1664525 + 1013904223);
        return value;
    }
    public static void Run(string path)
    {
        var results = new List<object>();
        foreach (var mode in new[] { "baseline", "disabled", "summary", "detailed" })
        {
            var timings = new List<double>(); var allocations = new List<double>();
            for (var repeat = -3; repeat < Repetitions; repeat++)
            {
                var recorder = new Recorder(); var op = recorder.RegisterOperation("benchmark.work", "benchmark");
                if (mode == "summary" || mode == "detailed") recorder.Start(new CaptureOptions {
                    Mode = mode == "summary" ? CaptureMode.Summary : CaptureMode.Detailed,
                    MaxRecords = Iterations, MaxDuration = TimeSpan.FromSeconds(60)
                });
                var bytes = GC.GetAllocatedBytesForCurrentThread(); var start = Stopwatch.GetTimestamp();
                var value = 7;
                if (mode == "baseline")
                { for (var i = 0; i < Iterations; i++) value = Work(value); }
                else
                { for (var i = 0; i < Iterations; i++) { using (recorder.Measure(op)) value = Work(value); } }
                var elapsed = Stopwatch.GetTimestamp() - start;
                var allocated = GC.GetAllocatedBytesForCurrentThread() - bytes;
                sink = value; recorder.Stop();
                if (repeat >= 0) { timings.Add((double)elapsed * 1_000_000_000 / Stopwatch.Frequency / Iterations); allocations.Add((double)allocated / Iterations); }
            }
            timings.Sort(); allocations.Sort();
            results.Add(new { mode, median_ns_per_operation = timings[Repetitions / 2], min_ns_per_operation = timings[0], max_ns_per_operation = timings[^1], median_allocated_bytes_per_operation = allocations[Repetitions / 2] });
        }
        var report = new {
            workload = "32 integer mixing steps per operation; setup, stop and export excluded; no game integration",
            iterations = Iterations, repetitions = Repetitions, warmup_repetitions = 3,
            runtime = Environment.Version.ToString(), os = Environment.OSVersion.VersionString,
            architecture = System.Runtime.InteropServices.RuntimeInformation.ProcessArchitecture.ToString(),
            stopwatch_frequency_hz = Stopwatch.Frequency, results,
            limitations = "Microbenchmark only. Tiering, scheduling and timer cost affect results. Negative differences from baseline are noise. Enabled modes include recorder work and retained-event allocations."
        };
        Directory.CreateDirectory(Path.GetDirectoryName(Path.GetFullPath(path))!);
        using var file = new FileStream(path, FileMode.CreateNew);
        JsonSerializer.Serialize(file, report, new JsonSerializerOptions { WriteIndented = true });
        Console.WriteLine("Overhead report written to " + path + "; checksum " + sink);
    }
}
