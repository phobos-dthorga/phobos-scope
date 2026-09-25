using System;
using System.IO;
using System.Runtime.Serialization.Json;

namespace Phobos.Scope.Recording;

/// <summary>A stopped, immutable capture. Export failures never consume its data.</summary>
public sealed class CaptureSnapshot
{
    private readonly CaptureData data;
    internal CaptureSnapshot(CaptureData data) { this.data = data; }
    public string CaptureId => data.CaptureId;
    public string StopReason => data.StopReason;
    public long DroppedRecords => data.DroppedRecords;
    public long RejectedMeasurements => data.RejectedMeasurements;

    public void WriteJson(Stream destination)
    {
        if (destination == null) throw new ArgumentNullException(nameof(destination));
        new DataContractJsonSerializer(typeof(CaptureData)).WriteObject(destination, data);
    }

    /// <summary>Creates a new file via a sibling temporary file. Never overwrites an existing capture.</summary>
    public void Export(string path)
    {
        var fullPath = Path.GetFullPath(path);
        var directory = Path.GetDirectoryName(fullPath)!;
        Directory.CreateDirectory(directory);
        var temporary = Path.Combine(directory, ".scope-" + Guid.NewGuid().ToString("N") + ".tmp");
        try
        {
            using (var stream = new FileStream(temporary, FileMode.CreateNew, FileAccess.Write, FileShare.None))
            { WriteJson(stream); stream.Flush(); }
            File.Move(temporary, fullPath);
        }
        finally
        {
            // Cleanup must not mask the original error; the snapshot always remains usable.
            try { if (File.Exists(temporary)) File.Delete(temporary); }
            catch (IOException) { }
            catch (UnauthorizedAccessException) { }
        }
    }
}
