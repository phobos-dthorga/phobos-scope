using System;
using System.IO;
using System.Linq;
using System.Text;
using System.Text.Json;
using System.Threading.Tasks;
using Phobos.Scope.Recording;

int checks = 0;
void Check(bool ok, string name) { if (!ok) throw new Exception(name); checks++; }
JsonElement Data(CaptureSnapshot capture)
{
    using var stream = new MemoryStream(); capture.WriteJson(stream);
    using var document = JsonDocument.Parse(stream.ToArray()); return document.RootElement.Clone();
}
long Field(JsonElement data, string name) => data.GetProperty(name).GetInt64();
JsonElement Aggregate(JsonElement data, int index = 0) => data.GetProperty("aggregates")[index];
void Save(string name, CaptureSnapshot capture)
{
    if (args.Length > 0) capture.Export(Path.Combine(args[0], name + ".json"));
}

long clock = 9_007_199_254_740_992; int reads = 0;
var r = new Recorder(() => { reads++; return clock; }, 1_000);
var outer = r.RegisterOperation("fixture.outer", "test");
var inner = r.RegisterOperation("fixture.inner", "test");
var counter = r.RegisterCounter("fixture.queue", "test", "items", CounterKind.Gauge);
var context = r.RegisterContext("fixture.speed", "context");
Check(r.GetStatus() == null && reads == 0, "Idle status has no fabricated capture and reads no clock");
for (var i = 0; i < 100; i++) { using (r.Measure(outer)) { } r.Sample(counter, 1); r.Context(context, "1x"); r.Poll(); }
Check(reads == 0, "Disabled paths read no clocks");
long beforeBytes = GC.GetAllocatedBytesForCurrentThread();
for (var i = 0; i < 10_000; i++) { using (r.Measure(outer)) { } r.Sample(counter, 1); r.Context(context, "1x"); r.Poll(); }
Check(GC.GetAllocatedBytesForCurrentThread() == beforeBytes, "Disabled instrumentation allocates no managed memory after warmup");
r.Start();
r.Context(context, "1x");
var first = r.Measure(outer); clock += 2;
var live = r.GetStatus()!;
Check(live.IsRecording && live.OpenScopes == 1 && live.CompletedScopes == 0 && live.ElapsedSeconds == .002, "Live status observes without closing an open scope");
var second = r.Measure(inner); clock += 3; second.Dispose(); second.Dispose();
r.Sample(counter, 7); clock += 5; first.Dispose(); clock += 10;
r.Context(context, "4x");
var snapshot = r.Stop()!; var data = Data(snapshot);
Check(Field(data, "end_tick") == 20, "Large absolute clock is subtracted before export");
Check(Field(Aggregate(data), "total_ticks") == 10 && Field(Aggregate(data, 1), "total_ticks") == 3, "Nested inclusive durations remain distinct");
Check(Field(Aggregate(data, 1), "calls") == 1, "Double disposal is idempotent");
Check(data.GetProperty("contexts")[1].GetProperty("tick").GetInt64() == 20, "Changing context has its own timestamp");
Check(!r.GetStatus()!.IsRecording && snapshot.Status.CompletedScopes == 2 && live.CompletedScopes == 0, "Stopped and previously read status views are immutable");
Save("known-detailed", snapshot);

r.Start(new CaptureOptions { Mode = CaptureMode.Summary });
using (r.Measure(outer)) clock += 4;
snapshot = r.Stop()!; data = Data(snapshot);
Check(data.GetProperty("events").GetArrayLength() == 0 && Field(Aggregate(data), "total_ticks") == 4, "Summary mode retains aggregates without events");
Save("known-summary", snapshot);

r.Start(new CaptureOptions { MaxRecords = 1 });
using (r.Measure(outer)) clock += 2;
using (r.Measure(outer)) clock += 3;
r.Sample(counter, 2);
snapshot = r.Stop()!; data = Data(snapshot);
Check(snapshot.DroppedRecords == 2 && Field(Aggregate(data), "calls") == 2 && Field(Aggregate(data), "total_ticks") == 5, "Record cap preserves complete timing aggregates and counts lost records");
Save("dropped", snapshot);

r.Start(); var abandoned = r.Measure(outer); clock += 5; snapshot = r.Stop(StopReason.WorldChange)!;
Check(Field(Aggregate(Data(snapshot)), "incomplete") == 1 && Field(Aggregate(Data(snapshot)), "calls") == 0, "Stopping open scopes marks them incomplete");
Save("incomplete", snapshot);
r.Start(); abandoned.Dispose(); using (r.Measure(outer)) clock += 2;
Check(Field(Aggregate(Data(r.Stop()!)), "calls") == 1, "Old session scopes cannot enter a new capture");

r.Start();
var gameplay = new InvalidOperationException("gameplay exception");
try { using (r.Measure(outer)) { clock += 3; throw gameplay; } }
catch (InvalidOperationException caught) { Check(ReferenceEquals(caught, gameplay), "Original workload exception is preserved"); }
Check(Field(Aggregate(Data(r.Stop()!)), "calls") == 1, "Exception unwinding closes the timing scope");

r.Start(new CaptureOptions { MaxDuration = TimeSpan.FromMilliseconds(5) });
var overrun = r.Measure(outer); clock += 10; overrun.Dispose(); snapshot = r.Stop()!;
Check(snapshot.StopReason == "duration_limit" && Field(Data(snapshot), "end_tick") == 5 && Field(Aggregate(Data(snapshot)), "incomplete") == 1, "Duration limit clamps boundary without fabricating a completion");
Save("duration-limit", snapshot);

r.Start(new CaptureOptions { MaxDepth = 1 });
using (r.Measure(outer)) { using (r.Measure(inner)) clock += 1; }
snapshot = r.Stop()!;
Check(snapshot.RejectedMeasurements == 1 && Field(Aggregate(Data(snapshot)), "calls") == 1, "Depth cap rejects nested measurements without breaking outer scope");
Save("depth-limit", snapshot);

r.Start(); first = r.Measure(outer); second = r.Measure(inner); clock += 1; first.Dispose(); second.Dispose(); snapshot = r.Stop()!;
Check(snapshot.StopReason == "nesting_error" && Data(snapshot).GetProperty("aggregates").EnumerateArray().All(a => Field(a, "incomplete") == 1), "Non-LIFO scopes stop cleanly with incomplete records");
Save("nesting-error", snapshot);

r.Start(); Task.Run(() => { using (r.Measure(outer)) { } r.Sample(counter, 1); }).GetAwaiter().GetResult(); snapshot = r.Stop()!;
Check(snapshot.RejectedMeasurements == 2 && Field(Aggregate(Data(snapshot)), "calls") == 0, "Wrong-thread calls are reported without recording false data");
Save("wrong-thread", snapshot);

var cumulative = r.RegisterCounter("fixture.total", "test", "items", CounterKind.Cumulative);
r.Start(); r.Sample(cumulative, 3); r.Sample(cumulative, 2); r.Sample(counter, double.NaN); r.Context(context, "");
snapshot = r.Stop()!;
Check(snapshot.RejectedMeasurements == 3 && Data(snapshot).GetProperty("counters").GetArrayLength() == 1, "Invalid numbers, cumulative resets and empty context are rejected");
Save("invalid-samples-rejected", snapshot);

r.Start(); clock -= 1; r.Poll(); snapshot = r.Stop()!;
Check(snapshot.StopReason == "clock_error", "Backwards clock stops capture");
Save("clock-error", snapshot);

bool throwClock = false;
var failing = new Recorder(() => throwClock ? throw new IOException("clock failed") : clock, 1_000);
var failureOp = failing.RegisterOperation("failure.work", "test"); failing.Start();
var failedScope = failing.Measure(failureOp); throwClock = true; failedScope.Dispose();
Check(failing.LastCapture!.StopReason == "clock_error", "A clock failure in Dispose never replaces a gameplay exception");
Save("clock-failure", failing.LastCapture!);

var temp = Path.Combine(Path.GetTempPath(), "scope-tests-" + Guid.NewGuid().ToString("N"));
Directory.CreateDirectory(temp);
try
{
    var path = Path.Combine(temp, "capture.json"); File.WriteAllText(path, "original");
    try { snapshot.Export(path); throw new Exception("Expected overwrite refusal"); } catch (IOException) { }
    Check(File.ReadAllText(path) == "original", "Export never overwrites an existing file");
    snapshot.Export(Path.Combine(temp, "retry.json"));
    Check(File.Exists(Path.Combine(temp, "retry.json")), "A failed export leaves the snapshot available for retry");
    using var brokenStream = new BrokenStream();
    try { snapshot.WriteJson(brokenStream); throw new Exception("Expected stream failure"); } catch (IOException) { }
    Check(Data(snapshot).GetProperty("capture_id").GetString() == snapshot.CaptureId, "Write failure leaves capture immutable and recoverable");
}
finally { Directory.Delete(temp, recursive: true); }

var escaped = new Recorder(() => clock, 1_000);
r.Start(); snapshot = r.StopAfterDiagnosticFailure()!;
Check(snapshot.RejectedMeasurements == 1 && !r.IsRecording, "Adapter failure stops with a quality warning rather than an apparently complete capture");
Save("adapter-failure", snapshot);
var escapedOp = escaped.RegisterOperation("=formula,\"quoted\"\nline", "csv");
var escapedContext = escaped.RegisterContext("fixture.text", "context");
escaped.Start(); using (escaped.Measure(escapedOp)) clock++;
escaped.Context(escapedContext, "Unicode: λ 😀 \\ \"\nline");
snapshot = escaped.Stop()!;
Check(Data(snapshot).GetProperty("contexts")[0].GetProperty("value").GetString()!.Contains("😀"), "Serializer preserves Unicode and escapes");
Save("escaped", snapshot);
Console.WriteLine($"{checks} recorder behaviour checks passed.");

sealed class BrokenStream : MemoryStream
{
    public override void Write(byte[] buffer, int offset, int count) => throw new IOException("Synthetic full destination");
}
