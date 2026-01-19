using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;

namespace RhythmOpenExchange
{
    /// <summary>
    /// Format identifiers for encode/decode operations.
    /// </summary>
    public enum RoxFormat
    {
        Rox = 0,
        Osu = 1,
        Sm = 2,
        Qua = 3,
        Fnf = 4
    }

    /// <summary>
    /// Type of note.
    /// </summary>
    public enum NoteType : byte
    {
        Tap = 0,
        Hold = 1,
        Burst = 2,
        Mine = 3
    }

    /// <summary>
    /// Represents a single note in the chart.
    /// </summary>
    [StructLayout(LayoutKind.Sequential)]
    public struct RoxNote
    {
        /// <summary>Time in microseconds.</summary>
        public long TimeUs;
        /// <summary>Column index (0-indexed).</summary>
        public byte Column;
        /// <summary>Type of note.</summary>
        public NoteType Type;
        /// <summary>Duration in microseconds (for Hold/Burst, 0 otherwise).</summary>
        public long DurationUs;

        /// <summary>Time in seconds.</summary>
        public double TimeSeconds => TimeUs / 1_000_000.0;

        /// <summary>Duration in seconds.</summary>
        public double DurationSeconds => DurationUs / 1_000_000.0;

        /// <summary>End time in microseconds.</summary>
        public long EndTimeUs => TimeUs + DurationUs;

        public override string ToString() => $"Note({Type}, t={TimeSeconds:F3}s, col={Column})";
    }

    /// <summary>
    /// Result structure for byte operations.
    /// </summary>
    [StructLayout(LayoutKind.Sequential)]
    internal struct FfiBytesResult
    {
        public int Success;
        public IntPtr Error;
        public IntPtr Data;
        public nuint Len;
    }

    /// <summary>
    /// Wrapper for rhythm-open-exchange library.
    /// Provides chart loading, saving, modification, and hashing.
    /// </summary>
    public class RoxChart : IDisposable
    {
        private const string LibraryName = "rhythm_open_exchange";

        private IntPtr _handle;
        private bool _disposed;

        #region Native Methods

        // Decode/Encode
        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr rox_decode_bytes(byte[] data, nuint len);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr rox_decode_string([MarshalAs(UnmanagedType.LPStr)] string data);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr rox_decode_with_format(byte[] data, nuint len, int format);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        private static extern FfiBytesResult rox_encode_with_format(IntPtr chart, int format);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr rox_encode_to_string(IntPtr chart, int format);

        // Create
        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr rox_chart_new(byte keyCount);

        // Getters
        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr rox_chart_title(IntPtr chart);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr rox_chart_artist(IntPtr chart);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr rox_chart_creator(IntPtr chart);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr rox_chart_difficulty(IntPtr chart);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        private static extern int rox_chart_key_count(IntPtr chart);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        private static extern nuint rox_chart_note_count(IntPtr chart);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        private static extern long rox_chart_duration_us(IntPtr chart);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        private static extern int rox_chart_is_coop(IntPtr chart);

        // Hash functions
        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr rox_chart_hash(IntPtr chart);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr rox_chart_notes_hash(IntPtr chart);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr rox_chart_short_hash(IntPtr chart);

        // Setters
        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        private static extern void rox_chart_set_title(IntPtr chart, [MarshalAs(UnmanagedType.LPStr)] string title);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        private static extern void rox_chart_set_artist(IntPtr chart, [MarshalAs(UnmanagedType.LPStr)] string artist);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        private static extern void rox_chart_set_creator(IntPtr chart, [MarshalAs(UnmanagedType.LPStr)] string creator);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        private static extern void rox_chart_set_difficulty(IntPtr chart, [MarshalAs(UnmanagedType.LPStr)] string difficulty);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        private static extern void rox_chart_set_key_count(IntPtr chart, byte keyCount);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        private static extern void rox_chart_set_coop(IntPtr chart, int isCoop);

        // Note manipulation
        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        private static extern void rox_chart_add_tap(IntPtr chart, long timeUs, byte column);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        private static extern void rox_chart_add_hold(IntPtr chart, long timeUs, long durationUs, byte column);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        private static extern void rox_chart_add_burst(IntPtr chart, long timeUs, long durationUs, byte column);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        private static extern void rox_chart_add_mine(IntPtr chart, long timeUs, byte column);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        private static extern int rox_chart_remove_note(IntPtr chart, nuint index);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        private static extern void rox_chart_clear_notes(IntPtr chart);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        private static extern int rox_chart_get_note(IntPtr chart, nuint index, out RoxNote note);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        private static extern void rox_chart_sort_notes(IntPtr chart);

        // Memory management
        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        private static extern void rox_free_chart(IntPtr chart);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        private static extern void rox_free_string(IntPtr s);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        private static extern void rox_free_bytes(IntPtr data, nuint len);

        #endregion

        private RoxChart(IntPtr handle)
        {
            _handle = handle;
        }

        #region Static Factory Methods

        /// <summary>
        /// Create a new empty chart with the given key count.
        /// </summary>
        public static RoxChart Create(int keyCount = 4)
        {
            var handle = rox_chart_new((byte)keyCount);
            return new RoxChart(handle);
        }

        /// <summary>
        /// Decode a chart from bytes with auto-detection.
        /// </summary>
        public static RoxChart? FromBytes(byte[] data)
        {
            var handle = rox_decode_bytes(data, (nuint)data.Length);
            return handle == IntPtr.Zero ? null : new RoxChart(handle);
        }

        /// <summary>
        /// Decode a chart from a string with auto-detection.
        /// </summary>
        public static RoxChart? FromString(string data)
        {
            var handle = rox_decode_string(data);
            return handle == IntPtr.Zero ? null : new RoxChart(handle);
        }

        /// <summary>
        /// Decode a chart from bytes with a specific format.
        /// </summary>
        public static RoxChart? FromBytes(byte[] data, RoxFormat format)
        {
            var handle = rox_decode_with_format(data, (nuint)data.Length, (int)format);
            return handle == IntPtr.Zero ? null : new RoxChart(handle);
        }

        /// <summary>
        /// Load a chart from a file.
        /// </summary>
        public static RoxChart? FromFile(string path)
        {
            var data = System.IO.File.ReadAllBytes(path);
            return FromBytes(data);
        }

        /// <summary>
        /// Load a chart from a file with a specific format.
        /// </summary>
        public static RoxChart? FromFile(string path, RoxFormat format)
        {
            var data = System.IO.File.ReadAllBytes(path);
            return FromBytes(data, format);
        }

        #endregion

        #region Encoding Methods

        /// <summary>
        /// Encode the chart to bytes with a specific format.
        /// </summary>
        public byte[]? ToBytes(RoxFormat format)
        {
            var result = rox_encode_with_format(_handle, (int)format);
            if (result.Success == 0)
            {
                if (result.Error != IntPtr.Zero)
                {
                    rox_free_string(result.Error);
                }
                return null;
            }

            var bytes = new byte[(int)result.Len];
            Marshal.Copy(result.Data, bytes, 0, (int)result.Len);
            rox_free_bytes(result.Data, result.Len);
            return bytes;
        }

        /// <summary>
        /// Encode the chart to a string (for text-based formats).
        /// </summary>
        public string? ToString(RoxFormat format)
        {
            if (format == RoxFormat.Rox)
            {
                throw new ArgumentException("Rox format is binary, use ToBytes instead.");
            }

            var ptr = rox_encode_to_string(_handle, (int)format);
            if (ptr == IntPtr.Zero) return null;

            var result = Marshal.PtrToStringAnsi(ptr);
            rox_free_string(ptr);
            return result;
        }

        /// <summary>
        /// Save the chart to a file.
        /// </summary>
        public bool ToFile(string path, RoxFormat format)
        {
            var data = ToBytes(format);
            if (data == null) return false;
            System.IO.File.WriteAllBytes(path, data);
            return true;
        }

        #endregion

        #region Metadata Properties

        /// <summary>
        /// Get or set the title of the chart.
        /// </summary>
        public string Title
        {
            get => GetString(rox_chart_title);
            set => rox_chart_set_title(_handle, value);
        }

        /// <summary>
        /// Get or set the artist of the chart.
        /// </summary>
        public string Artist
        {
            get => GetString(rox_chart_artist);
            set => rox_chart_set_artist(_handle, value);
        }

        /// <summary>
        /// Get or set the creator/mapper of the chart.
        /// </summary>
        public string Creator
        {
            get => GetString(rox_chart_creator);
            set => rox_chart_set_creator(_handle, value);
        }

        /// <summary>
        /// Get or set the difficulty name of the chart.
        /// </summary>
        public string Difficulty
        {
            get => GetString(rox_chart_difficulty);
            set => rox_chart_set_difficulty(_handle, value);
        }

        /// <summary>
        /// Get or set the key count of the chart.
        /// </summary>
        public int KeyCount
        {
            get => rox_chart_key_count(_handle);
            set => rox_chart_set_key_count(_handle, (byte)value);
        }

        /// <summary>
        /// Get or set whether this is a coop chart.
        /// </summary>
        public bool IsCoop
        {
            get => rox_chart_is_coop(_handle) != 0;
            set => rox_chart_set_coop(_handle, value ? 1 : 0);
        }

        /// <summary>
        /// Get the note count of the chart.
        /// </summary>
        public ulong NoteCount => (ulong)rox_chart_note_count(_handle);

        /// <summary>
        /// Get the duration of the chart in microseconds.
        /// </summary>
        public long DurationUs => rox_chart_duration_us(_handle);

        /// <summary>
        /// Get the duration of the chart in seconds.
        /// </summary>
        public double DurationSeconds => DurationUs / 1_000_000.0;

        #endregion

        #region Hash Properties

        /// <summary>
        /// Get the full BLAKE3 hash of the entire chart.
        /// </summary>
        public string Hash => GetString(rox_chart_hash);

        /// <summary>
        /// Get the BLAKE3 hash of just the notes (ignoring metadata).
        /// Useful for comparing charts with different metadata but same gameplay.
        /// </summary>
        public string NotesHash => GetString(rox_chart_notes_hash);

        /// <summary>
        /// Get the short hash (first 16 hex chars).
        /// </summary>
        public string ShortHash => GetString(rox_chart_short_hash);

        #endregion

        #region Note Manipulation

        /// <summary>
        /// Add a tap note.
        /// </summary>
        /// <param name="timeUs">Time in microseconds.</param>
        /// <param name="column">Column index (0-indexed).</param>
        public void AddTap(long timeUs, int column)
        {
            rox_chart_add_tap(_handle, timeUs, (byte)column);
        }

        /// <summary>
        /// Add a tap note using seconds.
        /// </summary>
        public void AddTapSeconds(double timeSeconds, int column)
        {
            AddTap((long)(timeSeconds * 1_000_000), column);
        }

        /// <summary>
        /// Add a hold note.
        /// </summary>
        /// <param name="timeUs">Start time in microseconds.</param>
        /// <param name="durationUs">Duration in microseconds.</param>
        /// <param name="column">Column index (0-indexed).</param>
        public void AddHold(long timeUs, long durationUs, int column)
        {
            rox_chart_add_hold(_handle, timeUs, durationUs, (byte)column);
        }

        /// <summary>
        /// Add a hold note using seconds.
        /// </summary>
        public void AddHoldSeconds(double timeSeconds, double durationSeconds, int column)
        {
            AddHold((long)(timeSeconds * 1_000_000), (long)(durationSeconds * 1_000_000), column);
        }

        /// <summary>
        /// Add a burst/roll note.
        /// </summary>
        public void AddBurst(long timeUs, long durationUs, int column)
        {
            rox_chart_add_burst(_handle, timeUs, durationUs, (byte)column);
        }

        /// <summary>
        /// Add a burst/roll note using seconds.
        /// </summary>
        public void AddBurstSeconds(double timeSeconds, double durationSeconds, int column)
        {
            AddBurst((long)(timeSeconds * 1_000_000), (long)(durationSeconds * 1_000_000), column);
        }

        /// <summary>
        /// Add a mine note.
        /// </summary>
        public void AddMine(long timeUs, int column)
        {
            rox_chart_add_mine(_handle, timeUs, (byte)column);
        }

        /// <summary>
        /// Add a mine note using seconds.
        /// </summary>
        public void AddMineSeconds(double timeSeconds, int column)
        {
            AddMine((long)(timeSeconds * 1_000_000), column);
        }

        /// <summary>
        /// Get a note by index.
        /// </summary>
        public RoxNote? GetNote(int index)
        {
            if (rox_chart_get_note(_handle, (nuint)index, out var note) != 0)
            {
                return note;
            }
            return null;
        }

        /// <summary>
        /// Get all notes as a list.
        /// </summary>
        public List<RoxNote> GetNotes()
        {
            var notes = new List<RoxNote>();
            var count = (int)NoteCount;
            for (int i = 0; i < count; i++)
            {
                var note = GetNote(i);
                if (note.HasValue)
                {
                    notes.Add(note.Value);
                }
            }
            return notes;
        }

        /// <summary>
        /// Remove a note by index.
        /// </summary>
        /// <returns>True if the note was removed.</returns>
        public bool RemoveNote(int index)
        {
            return rox_chart_remove_note(_handle, (nuint)index) != 0;
        }

        /// <summary>
        /// Clear all notes from the chart.
        /// </summary>
        public void ClearNotes()
        {
            rox_chart_clear_notes(_handle);
        }

        /// <summary>
        /// Sort all notes by time.
        /// </summary>
        public void SortNotes()
        {
            rox_chart_sort_notes(_handle);
        }

        #endregion

        #region Helper Methods

        private string GetString(Func<IntPtr, IntPtr> getter)
        {
            var ptr = getter(_handle);
            if (ptr == IntPtr.Zero) return string.Empty;
            var result = Marshal.PtrToStringAnsi(ptr) ?? string.Empty;
            rox_free_string(ptr);
            return result;
        }

        #endregion

        #region IDisposable

        protected virtual void Dispose(bool disposing)
        {
            if (!_disposed)
            {
                if (_handle != IntPtr.Zero)
                {
                    rox_free_chart(_handle);
                    _handle = IntPtr.Zero;
                }
                _disposed = true;
            }
        }

        ~RoxChart()
        {
            Dispose(false);
        }

        public void Dispose()
        {
            Dispose(true);
            GC.SuppressFinalize(this);
        }

        #endregion

        public override string ToString()
        {
            return $"RoxChart({Artist} - {Title} [{KeyCount}K, {NoteCount} notes])";
        }
    }
}
