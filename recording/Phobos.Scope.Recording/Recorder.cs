using System;
using System.Collections.Generic;
using System.Diagnostics;
using System.Text;
using System.Threading;

namespace Phobos.Scope.Recording;

public enum CaptureMode { Summary, Detailed }
public enum CounterKind { Gauge, Cumulative, Increment }
public enum StopReason { Manual, WorldChange, ApplicationExit }

public sealed class CaptureOptions
{
    public CaptureMode Mode { get; set; } = CaptureMode.Detailed;
    public int MaxRecords { get; set; } = 20_000;
    public int MaxDepth { get; set; } = 64;
    public TimeSpan MaxDuration { get; set; } = TimeSpan.FromSeconds(60);
    public IReadOnlyDictionary<string, string>? Metadata { get; set; }
}

public readonly struct Metric
{
    internal readonly Recorder? Owner;
    internal readonly int Id;
    internal Metric(Recorder owner, int id) { Owner = owner; Id = id; }
}

public readonly struct TimingScope : IDisposable
{
    private readonly Recorder? owner;
    private readonly long generation;
    private readonly long id;
    internal TimingScope(Recorder owner, long generation, long id)
    { this.owner = owner; this.generation = generation; this.id = id; }
    public void Dispose() { owner?.End(generation, id); }
}

/// <summary>Opt-in synchronous recorder. Setup/lifecycle belong to its creating thread.</summary>
public sealed class Recorder
{
    public const string Version = "0.1.1";
    public const int MaxDefinitions = 256;
    public const int MaxTextBytes = 512;
    public const int MaxRecordLimit = 20_000;
    public const int MaxDepthLimit = 256;
    public const int MaxDurationSeconds = 3_600;
    public const long MaxClockFrequency = 1_000_000_000;
    private readonly int ownerThread = Environment.CurrentManagedThreadId;
    private readonly List<Definition> definitions = new();
    private readonly Func<long> timestamp;
    private readonly long frequency;
    private volatile bool enabled;
    private long generation;
    private long nextScope;
    private long start;
    private long lastTick;
    private long rejected;
    private CaptureData? active;
    private Aggregate?[] aggregates = Array.Empty<Aggregate?>();
    private double?[] cumulative = Array.Empty<double?>();
    private ActiveScope[] stack = Array.Empty<ActiveScope>();
    private int depth;
    private int records;
    public bool IsRecording => enabled;
    public CaptureSnapshot? LastCapture { get; private set; }

    /// <summary>Owner-thread status query. Polls limits without stopping a live capture.</summary>
    public RecordingStatus? GetStatus()
    {
        RequireOwner(); Poll();
        return active != null ? new RecordingStatus(active, true, lastTick, Interlocked.Read(ref rejected), depth) : LastCapture?.Status;
    }

    public Recorder() : this(Stopwatch.GetTimestamp, Stopwatch.Frequency) { }
    // Injectable monotonic clock enables deterministic lifecycle and precision checks.
    internal Recorder(Func<long> timestamp, long frequency)
    {
        if (frequency <= 0 || frequency > MaxClockFrequency) throw new ArgumentOutOfRangeException(nameof(frequency));
        this.timestamp = timestamp; this.frequency = frequency;
    }

    private void RequireOwner()
    {
        if (Environment.CurrentManagedThreadId != ownerThread)
            throw new InvalidOperationException("scope.owner_thread: Configure and control recording on its creating thread.");
    }
    private static bool ValidText(string? value) => !string.IsNullOrEmpty(value) && value!.Length <= MaxTextBytes && !value.Contains('\0') && Encoding.UTF8.GetByteCount(value) <= MaxTextBytes;
    private Metric Register(string name, string category, string unit, string kind)
    {
        RequireOwner();
        if (enabled) throw new InvalidOperationException("scope.registration: Register metrics before starting recording.");
        if (!ValidText(name) || !ValidText(category) || !ValidText(unit)) throw new ArgumentException("scope.text: Labels must be nonempty and at most 512 UTF-8 bytes.");
        foreach (var d in definitions) if (d.Name == name) throw new ArgumentException("scope.name: Metric names must be unique.");
        if (definitions.Count == MaxDefinitions) throw new InvalidOperationException("scope.definitions: Definition limit reached.");
        var metric = new Metric(this, definitions.Count);
        definitions.Add(new Definition { Id = metric.Id, Name = name, Category = category, Unit = unit, Kind = kind });
        return metric;
    }
    public Metric RegisterOperation(string name, string category) => Register(name, category, "ticks", "operation");
    public Metric RegisterContext(string name, string category) => Register(name, category, "text", "context");
    public Metric RegisterCounter(string name, string category, string unit, CounterKind kind)
        => Register(name, category, unit, kind switch { CounterKind.Gauge => "gauge", CounterKind.Cumulative => "cumulative", CounterKind.Increment => "increment", _ => throw new ArgumentOutOfRangeException(nameof(kind)) });

    public void Start(CaptureOptions? options = null)
    {
        RequireOwner();
        if (enabled) throw new InvalidOperationException("scope.active: Stop the current capture first.");
        options ??= new CaptureOptions();
        if (!Enum.IsDefined(typeof(CaptureMode), options.Mode) || options.MaxRecords < 1 || options.MaxRecords > MaxRecordLimit || options.MaxDepth < 1 || options.MaxDepth > MaxDepthLimit || options.MaxDuration <= TimeSpan.Zero || options.MaxDuration.TotalSeconds > MaxDurationSeconds)
            throw new ArgumentOutOfRangeException(nameof(options), "scope.limits: Recording limits are outside supported bounds.");
        var durationTicks = (long)(options.MaxDuration.TotalSeconds * frequency);
        if (durationTicks < 1) throw new ArgumentOutOfRangeException(nameof(options), "scope.duration: Duration must span at least one clock tick.");
        var data = new CaptureData {
            CaptureId = Guid.NewGuid().ToString("N"), Mode = options.Mode == CaptureMode.Detailed ? "detailed" : "summary",
            ClockFrequencyHz = frequency, Definitions = new List<Definition>(definitions),
            Limits = new Limits { MaxRecords = options.MaxRecords, MaxDepth = options.MaxDepth, MaxDurationTicks = durationTicks }
        };
        if (options.Metadata != null)
        {
            if (options.Metadata.Count > 32) throw new ArgumentException("scope.metadata: At most 32 metadata entries are supported.");
            foreach (var pair in options.Metadata)
            {
                if (!ValidText(pair.Key) || !ValidText(pair.Value)) throw new ArgumentException("scope.metadata: Metadata must be bounded nonempty text.");
                data.Metadata.Add(new Metadata { Key = pair.Key, Value = pair.Value });
            }
        }
        aggregates = new Aggregate?[definitions.Count]; cumulative = new double?[definitions.Count];
        foreach (var d in definitions) if (d.Kind == "operation")
        { var a = new Aggregate { Metric = d.Id }; aggregates[d.Id] = a; data.Aggregates.Add(a); }
        stack = new ActiveScope[options.MaxDepth]; depth = records = 0;
        Interlocked.Exchange(ref rejected, 0); lastTick = 0;
        start = timestamp(); active = data; generation++; enabled = true;
    }

    private bool OnThread()
    {
        if (Environment.CurrentManagedThreadId == ownerThread) return true;
        Interlocked.Increment(ref rejected); return false;
    }
    private bool MetricKind(Metric metric, string kind)
    {
        if (metric.Owner == this && metric.Id < definitions.Count && definitions[metric.Id].Kind == kind) return true;
        Interlocked.Increment(ref rejected); return false;
    }
    private bool Now(out long tick)
    {
        tick = lastTick;
        try
        {
            var current = timestamp();
            tick = checked(current - start);
            if (tick < lastTick) { Finish(lastTick, "clock_error"); return false; }
        }
        catch (Exception) { Finish(lastTick, "clock_error"); return false; }
        if (tick >= active!.Limits.MaxDurationTicks) { Finish(active.Limits.MaxDurationTicks, "duration_limit"); return false; }
        lastTick = tick; return true;
    }

    public TimingScope Measure(Metric operation)
    {
        if (!enabled) return default;
        if (!OnThread() || !MetricKind(operation, "operation") || !Now(out var tick)) return default;
        if (depth == stack.Length) { Interlocked.Increment(ref rejected); return default; }
        var id = ++nextScope;
        stack[depth++] = new ActiveScope { Id = id, Metric = operation.Id, Tick = tick };
        return new TimingScope(this, generation, id);
    }

    internal void End(long session, long id)
    {
        if (!enabled || session != generation || !OnThread()) return;
        var index = depth - 1;
        while (index >= 0 && stack[index].Id != id) index--;
        if (index < 0) return; // Copied/double-disposed scope; count it only once.
        if (!Now(out var tick)) return;
        if (index != depth - 1) { Interlocked.Increment(ref rejected); Finish(tick, "nesting_error"); return; }
        var scope = stack[--depth];
        var duration = tick - scope.Tick;
        var aggregate = aggregates[scope.Metric]!;
        if (aggregate.Calls == long.MaxValue || aggregate.TotalTicks > long.MaxValue - duration)
        { aggregate.Incomplete++; Finish(tick, "overflow"); return; }
        aggregate.Calls++; aggregate.TotalTicks += duration; aggregate.MaxTicks = Math.Max(aggregate.MaxTicks, duration);
        if (active!.Mode == "detailed" && Retain())
            active.Events.Add(new DurationEvent { Metric = scope.Metric, StartTick = scope.Tick, DurationTicks = duration });
    }

    private bool Retain()
    {
        if (records < active!.Limits.MaxRecords) { records++; return true; }
        active.DroppedRecords++; return false;
    }

    public void Sample(Metric counter, double value)
    {
        if (!enabled) return;
        if (!OnThread()) return;
        if (counter.Owner != this || counter.Id >= definitions.Count || double.IsNaN(value) || double.IsInfinity(value))
        { Interlocked.Increment(ref rejected); return; }
        var kind = definitions[counter.Id].Kind;
        if (kind != "gauge" && kind != "cumulative" && kind != "increment") { Interlocked.Increment(ref rejected); return; }
        if (!Now(out var tick)) return;
        if (kind == "cumulative")
        {
            if (value < 0 || (cumulative[counter.Id].HasValue && value < cumulative[counter.Id]!.Value)) { Interlocked.Increment(ref rejected); return; }
            cumulative[counter.Id] = value;
        }
        if (Retain()) active!.Counters.Add(new CounterSample { Metric = counter.Id, Tick = tick, Value = value });
    }

    public void Context(Metric context, string value)
    {
        if (!enabled) return;
        if (!OnThread() || !MetricKind(context, "context")) return;
        if (!ValidText(value)) { Interlocked.Increment(ref rejected); return; }
        if (Now(out var tick) && Retain()) active!.Contexts.Add(new ContextSample { Metric = context.Id, Tick = tick, Value = value });
    }

    /// <summary>Call from the owner-thread update loop to enforce duration while no scopes run.</summary>
    public void Poll() { if (enabled && OnThread()) Now(out _); }

    public CaptureSnapshot? Stop(StopReason reason = StopReason.Manual)
    {
        RequireOwner();
        var key = reason switch { StopReason.Manual => "manual", StopReason.WorldChange => "world_change", StopReason.ApplicationExit => "application_exit", _ => throw new ArgumentOutOfRangeException(nameof(reason)) };
        if (enabled && Now(out var tick)) Finish(tick, key);
        return LastCapture;
    }
    private void Finish(long tick, string reason)
    {
        enabled = false;
        var data = active!;
        while (depth > 0) aggregates[stack[--depth].Metric]!.Incomplete++;
        data.EndTick = tick; data.StopReason = reason;
        data.RejectedMeasurements = Interlocked.Read(ref rejected);
        LastCapture = new CaptureSnapshot(data); active = null;
    }
    private struct ActiveScope { public long Id; public int Metric; public long Tick; }
}
