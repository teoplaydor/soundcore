using System;
using System.Collections.Concurrent;
using System.IO;
using System.IO.Pipes;
using System.Security.Principal;
using System.Threading;
using System.Threading.Tasks;
using Google.Protobuf;
using Soundcore.V1;

namespace SoundCore.UI.Services;

/// <summary>
/// Talks to the SoundCore core service over the named pipe defined in
/// <c>soundcore-ipc</c>. Frames are length-delimited protobuf messages
/// (the same wire format the Rust side uses).
///
/// A single background read loop parses every inbound frame and dispatches
/// responses to the matching <see cref="SendAsync"/> caller by request id,
/// so concurrent requests (e.g. several pages loading at once) cannot
/// corrupt framing or steal each other's replies. Server-initiated events
/// are surfaced via <see cref="EventReceived"/>.
/// </summary>
public sealed class IpcClient : IAsyncDisposable
{
    public const string PipeName = "SoundCore.Service";

    private NamedPipeClientStream? _pipe;
    private readonly SemaphoreSlim _writeLock = new(1, 1);
    private readonly ConcurrentDictionary<ulong, TaskCompletionSource<Response>> _pending = new();
    private readonly CancellationTokenSource _shutdown = new();
    private Task? _readLoop;
    private long _nextRequestId;

    public bool IsConnected => _pipe is { IsConnected: true };

    /// <summary>Raised on the read-loop thread for each server push (request_id == 0).</summary>
    public event EventHandler<Event>? EventReceived;

    public async Task ConnectAsync(CancellationToken ct = default)
    {
        var pipe = new NamedPipeClientStream(
            ".",
            PipeName,
            PipeDirection.InOut,
            PipeOptions.Asynchronous,
            // Don't let a pipe-squatting local process impersonate the UI
            // user: hand the server only identification-level rights.
            TokenImpersonationLevel.Identification);
        await pipe.ConnectAsync(2000, ct).ConfigureAwait(false);
        _pipe = pipe;
        _readLoop = Task.Run(() => ReadLoopAsync(_shutdown.Token));
    }

    public async Task<Response> SendAsync(Request request, CancellationToken ct = default)
    {
        var pipe = _pipe;
        if (pipe is null || !pipe.IsConnected)
            throw new InvalidOperationException("IpcClient is not connected.");

        var requestId = unchecked((ulong)Interlocked.Increment(ref _nextRequestId));
        var tcs = new TaskCompletionSource<Response>(TaskCreationOptions.RunContinuationsAsynchronously);
        if (!_pending.TryAdd(requestId, tcs))
            throw new InvalidOperationException("duplicate request id");

        // Serialize the framed request once, then write it under the lock as a
        // single async operation so frames never interleave on the wire.
        byte[] frameBytes;
        using (var ms = new MemoryStream())
        {
            new Frame { RequestId = requestId, Request = request }.WriteDelimitedTo(ms);
            frameBytes = ms.ToArray();
        }

        await _writeLock.WaitAsync(ct).ConfigureAwait(false);
        try
        {
            await pipe.WriteAsync(frameBytes, ct).ConfigureAwait(false);
            await pipe.FlushAsync(ct).ConfigureAwait(false);
        }
        catch
        {
            _pending.TryRemove(requestId, out _);
            throw;
        }
        finally
        {
            _writeLock.Release();
        }

        // Cancel the await if the caller's token trips, without leaking the
        // pending entry.
        using (ct.Register(() =>
        {
            if (_pending.TryRemove(requestId, out var t))
                t.TrySetCanceled();
        }))
        {
            return await tcs.Task.ConfigureAwait(false);
        }
    }

    private async Task ReadLoopAsync(CancellationToken ct)
    {
        var pipe = _pipe!;
        try
        {
            while (!ct.IsCancellationRequested && pipe.IsConnected)
            {
                // ParseDelimitedFrom reads the varint length then the payload.
                // It blocks on the pipe; that's fine on this background loop.
                var frame = await Task.Run(() => Frame.Parser.ParseDelimitedFrom(pipe), ct)
                    .ConfigureAwait(false);
                if (frame is null)
                    break; // server closed the pipe

                switch (frame.BodyCase)
                {
                    case Frame.BodyOneofCase.Response:
                        if (_pending.TryRemove(frame.RequestId, out var tcs))
                            tcs.TrySetResult(frame.Response);
                        break;
                    case Frame.BodyOneofCase.Event:
                        EventReceived?.Invoke(this, frame.Event);
                        break;
                }
            }
        }
        catch (OperationCanceledException)
        {
            // normal shutdown
        }
        catch (Exception ex)
        {
            FailAllPending(ex);
            return;
        }
        FailAllPending(new EndOfStreamException("server closed pipe"));
    }

    private void FailAllPending(Exception ex)
    {
        foreach (var kv in _pending)
        {
            if (_pending.TryRemove(kv.Key, out var tcs))
                tcs.TrySetException(ex);
        }
    }

    public async ValueTask DisposeAsync()
    {
        _shutdown.Cancel();
        if (_pipe is not null)
        {
            try { await _pipe.DisposeAsync().ConfigureAwait(false); }
            catch { /* ignore */ }
            _pipe = null;
        }
        if (_readLoop is not null)
        {
            try { await _readLoop.ConfigureAwait(false); }
            catch { /* ignore */ }
        }
        FailAllPending(new ObjectDisposedException(nameof(IpcClient)));
        _writeLock.Dispose();
        _shutdown.Dispose();
    }
}
