using System;
using System.Runtime.InteropServices;
using System.Text;

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
    /// </summary>
    public class RoxChart : IDisposable
    {
        private const string LibraryName = "rhythm_open_exchange";

        private IntPtr _handle;
        private bool _disposed;

        #region Native Methods

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

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr rox_chart_title(IntPtr chart);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        private static extern IntPtr rox_chart_artist(IntPtr chart);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        private static extern int rox_chart_key_count(IntPtr chart);

        [DllImport(LibraryName, CallingConvention = CallingConvention.Cdecl)]
        private static extern nuint rox_chart_note_count(IntPtr chart);

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
        /// Get the title of the chart.
        /// </summary>
        public string Title
        {
            get
            {
                var ptr = rox_chart_title(_handle);
                if (ptr == IntPtr.Zero) return string.Empty;
                var result = Marshal.PtrToStringAnsi(ptr) ?? string.Empty;
                rox_free_string(ptr);
                return result;
            }
        }

        /// <summary>
        /// Get the artist of the chart.
        /// </summary>
        public string Artist
        {
            get
            {
                var ptr = rox_chart_artist(_handle);
                if (ptr == IntPtr.Zero) return string.Empty;
                var result = Marshal.PtrToStringAnsi(ptr) ?? string.Empty;
                rox_free_string(ptr);
                return result;
            }
        }

        /// <summary>
        /// Get the key count of the chart.
        /// </summary>
        public int KeyCount => rox_chart_key_count(_handle);

        /// <summary>
        /// Get the note count of the chart.
        /// </summary>
        public ulong NoteCount => (ulong)rox_chart_note_count(_handle);

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
    }
}
