namespace Phobos.Scope.Recording;

/// <summary>Read-only diagnostic view, created only when explicitly requested.</summary>
public sealed class RecordingStatus
{
    public bool IsRecording { get; }
    public string CaptureId { get; }
    public string Mode { get; }
    public string StopReason { get; }
    public double ElapsedSeconds { get; }
    public double MaximumSeconds { get; }
    public int RetainedRecords { get; }
    public int MaximumRecords { get; }
    public long DroppedRecords { get; }
    public long RejectedMeasurements { get; }
    public long CompletedScopes { get; }
    public long IncompleteScopes { get; }
    public int OpenScopes { get; }

    internal RecordingStatus(CaptureData data, bool recording, long elapsed, long rejected, int open)
    {
        IsRecording = recording; CaptureId = data.CaptureId; Mode = data.Mode;
        StopReason = recording ? "" : data.StopReason;
        ElapsedSeconds = (double)elapsed / data.ClockFrequencyHz;
        MaximumSeconds = (double)data.Limits.MaxDurationTicks / data.ClockFrequencyHz;
        RetainedRecords = data.Events.Count + data.Counters.Count + data.Contexts.Count;
        MaximumRecords = data.Limits.MaxRecords; DroppedRecords = data.DroppedRecords;
        RejectedMeasurements = rejected; OpenScopes = open;
        foreach (var aggregate in data.Aggregates)
        { CompletedScopes += aggregate.Calls; IncompleteScopes += aggregate.Incomplete; }
    }
}
