using System.Collections.Generic;
using System.Runtime.Serialization;

namespace Phobos.Scope.Recording;

// Private contract DTOs keep stopped snapshots immutable to callers.
[DataContract]
internal sealed class Definition
{
    [DataMember(Name = "id")] public int Id;
    [DataMember(Name = "name")] public string Name = "";
    [DataMember(Name = "category")] public string Category = "";
    [DataMember(Name = "unit")] public string Unit = "";
    [DataMember(Name = "kind")] public string Kind = "";
}
[DataContract]
internal sealed class Limits
{
    [DataMember(Name = "max_records")] public int MaxRecords;
    [DataMember(Name = "max_depth")] public int MaxDepth;
    [DataMember(Name = "max_duration_ticks")] public long MaxDurationTicks;
}
[DataContract]
internal sealed class Aggregate
{
    [DataMember(Name = "metric")] public int Metric;
    [DataMember(Name = "calls")] public long Calls;
    [DataMember(Name = "total_ticks")] public long TotalTicks;
    [DataMember(Name = "max_ticks")] public long MaxTicks;
    [DataMember(Name = "incomplete")] public long Incomplete;
}
[DataContract]
internal sealed class DurationEvent
{
    [DataMember(Name = "metric")] public int Metric;
    [DataMember(Name = "start_tick")] public long StartTick;
    [DataMember(Name = "duration_ticks")] public long DurationTicks;
}
[DataContract]
internal sealed class CounterSample
{
    [DataMember(Name = "metric")] public int Metric;
    [DataMember(Name = "tick")] public long Tick;
    [DataMember(Name = "value")] public double Value;
}
[DataContract]
internal sealed class ContextSample
{
    [DataMember(Name = "metric")] public int Metric;
    [DataMember(Name = "tick")] public long Tick;
    [DataMember(Name = "value")] public string Value = "";
}
[DataContract]
internal sealed class Metadata
{
    [DataMember(Name = "key")] public string Key = "";
    [DataMember(Name = "value")] public string Value = "";
}
[DataContract]
internal sealed class CaptureData
{
    [DataMember(Name = "format_version")] public int FormatVersion = 1;
    [DataMember(Name = "recorder_version")] public string RecorderVersion = "Phobos.Scope.Recording/0.1.0";
    [DataMember(Name = "capture_id")] public string CaptureId = "";
    [DataMember(Name = "mode")] public string Mode = "";
    [DataMember(Name = "clock_frequency_hz")] public long ClockFrequencyHz;
    [DataMember(Name = "end_tick")] public long EndTick;
    [DataMember(Name = "stop_reason")] public string StopReason = "manual";
    [DataMember(Name = "limits")] public Limits Limits = new();
    [DataMember(Name = "definitions")] public List<Definition> Definitions = new();
    [DataMember(Name = "aggregates")] public List<Aggregate> Aggregates = new();
    [DataMember(Name = "events")] public List<DurationEvent> Events = new();
    [DataMember(Name = "counters")] public List<CounterSample> Counters = new();
    [DataMember(Name = "contexts")] public List<ContextSample> Contexts = new();
    [DataMember(Name = "metadata")] public List<Metadata> Metadata = new();
    [DataMember(Name = "dropped_records")] public long DroppedRecords;
    [DataMember(Name = "rejected_measurements")] public long RejectedMeasurements;
}
