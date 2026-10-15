// Portfolio sample — async WAV play queue (decoupled from Editor export).
// Source: AIVoiceServer/WavPlayQueue.cs

using System.Media;

namespace AIVoiceServer;

/// <summary>
/// Sequential WAV playback queue — independent of Editor export.
/// Plays files in order with PlaySync (no cut-off between items).
/// </summary>
public sealed class WavPlayQueue : IDisposable
{
    readonly object _gate = new();
    readonly Queue<WavJob> _queue = new();
    readonly CancellationTokenSource _cts = new();
    readonly Task _worker;
    readonly bool _deleteAfterPlay;

    int _playing;
    string? _currentPath;

    public WavPlayQueue(bool deleteAfterPlay = true)
    {
        _deleteAfterPlay = deleteAfterPlay;
        _worker = Task.Run(PlayLoop);
        Console.WriteLine("[AIVoice2] wav play queue ready (async vs export)");
    }

    public int PendingCount
    {
        get { lock (_gate) return _queue.Count + _playing; }
    }

    public string? CurrentPath
    {
        get { lock (_gate) return _currentPath; }
    }

    public void Enqueue(string wavPath, string? label = null)
    {
        if (string.IsNullOrWhiteSpace(wavPath) || !File.Exists(wavPath))
        {
            Console.Error.WriteLine($"[AIVoice2] play skip missing wav='{wavPath}'");
            return;
        }

        lock (_gate)
        {
            _queue.Enqueue(new WavJob(wavPath, label ?? Path.GetFileName(wavPath)));
            Monitor.Pulse(_gate);
        }
        Console.WriteLine($"[AIVoice2] play queued '{Path.GetFileName(wavPath)}' depth={PendingCount}");
    }

    void PlayLoop()
    {
        while (!_cts.IsCancellationRequested)
        {
            WavJob? job = null;
            lock (_gate)
            {
                while (_queue.Count == 0 && !_cts.IsCancellationRequested)
                {
                    Monitor.Wait(_gate, 500);
                }
                if (_queue.Count > 0)
                {
                    job = _queue.Dequeue();
                    _playing = 1;
                    _currentPath = job.Value.Path;
                }
            }

            if (job is not { } current) continue;

            try
            {
                PlayFile(current);
            }
            catch (Exception ex)
            {
                Console.Error.WriteLine($"[AIVoice2] play failed '{current.Label}': {ex.Message}");
            }
            finally
            {
                lock (_gate)
                {
                    _playing = 0;
                    _currentPath = null;
                }

                if (_deleteAfterPlay)
                {
                    TryDeleteLater(current.Path);
                }
            }
        }
    }

    static void PlayFile(WavJob job)
    {
        var sw = System.Diagnostics.Stopwatch.StartNew();
        Console.WriteLine($"[AIVoice2] play start '{job.Label}'");

        // Stream path avoids SoundPlayer unicode-filename issues.
        using (var fs = new FileStream(job.Path, FileMode.Open, FileAccess.Read, FileShare.Read))
        using (var player = new SoundPlayer(fs))
        {
            player.Load();
            player.PlaySync(); // blocks until finished — next item starts immediately after
        }

        Console.WriteLine($"[AIVoice2] play done '{job.Label}' ms={sw.ElapsedMilliseconds}");
    }

    static void TryDeleteLater(string path)
    {
        // Soft delete after play — don't block the play loop on IO glitches.
        _ = Task.Run(async () =>
        {
            try
            {
                await Task.Delay(400).ConfigureAwait(false);
                if (File.Exists(path))
                {
                    File.Delete(path);
                    Console.WriteLine($"[AIVoice2] wav deleted '{Path.GetFileName(path)}'");
                }
            }
            catch (Exception ex)
            {
                Console.WriteLine($"[AIVoice2] wav delete skip: {ex.Message}");
            }
        });
    }

    public void Dispose()
    {
        _cts.Cancel();
        lock (_gate) Monitor.PulseAll(_gate);
        try { _worker.Wait(TimeSpan.FromSeconds(2)); }
        catch { /* ignore */ }
        _cts.Dispose();
    }

    readonly record struct WavJob(string Path, string Label);
}
