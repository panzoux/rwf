using System;
using System.Collections.Generic;
using System.IO;
using System.Text;
using System.Threading;
using System.Threading.Tasks;

namespace TWF.Services
{
    public enum FileViewMode
    {
        Text,
        Hex
    }

    /// <summary>
    /// Engine for handling large files with lazy loading and asynchronous indexing.
    /// </summary>
    public class LargeFileEngine : IDisposable
    {
        private readonly string _filePath;
        private FileStream? _fileStream;
        private readonly object _streamLock = new object();
        private const int BufferSize = 64 * 1024; // 64KB read buffer

        // Text Mode State
        private readonly List<long> _lineOffsets = new List<long>();
        private CancellationTokenSource? _indexingCts;
        private bool _isIndexing = false;
        private long _indexedLength = 0;
        private Encoding _encoding = Encoding.UTF8;
        private int _currentEncodingIndex = 0;

        // Supported encodings for cycling
        private static readonly Encoding[] SupportedEncodings = GetSupportedEncodings();

        private static Encoding[] GetSupportedEncodings()
        {
            var encodings = new List<Encoding>
            {
                Encoding.UTF8,
                Encoding.Unicode,      // UTF-16 LE
                Encoding.ASCII,
                Encoding.Latin1
            };

            try { encodings.Add(Encoding.GetEncoding("shift_jis")); } catch { }
            try { encodings.Add(Encoding.GetEncoding("euc-jp")); } catch { }

            return encodings.ToArray();
        }
        
        // Stats
        public string FilePath => _filePath;
        public long FileSize { get; private set; }
        public int LineCount { get { lock (_lineOffsets) return _lineOffsets.Count; } }
        public bool IsIndexing => _isIndexing;
        public double IndexingProgress => FileSize > 0 ? (double)Interlocked.Read(ref _indexedLength) / FileSize : 0;
        public Encoding CurrentEncoding => _encoding;

        public event EventHandler? IndexingProgressChanged;
        public event EventHandler? IndexingCompleted;

        public LargeFileEngine(string filePath)
        {
            _filePath = filePath;
            FileSize = new FileInfo(filePath).Length;
            _lineOffsets.Add(0); // Line 0 starts at offset 0
        }

        public void Initialize(Encoding? encoding = null)
        {
            if (encoding != null)
            {
                _encoding = encoding;
                _currentEncodingIndex = Array.FindIndex(SupportedEncodings, e => e.CodePage == encoding.CodePage);
                if (_currentEncodingIndex < 0) _currentEncodingIndex = 0;
            }

            // Open with FileShare.ReadWrite to allow viewing logs that are being written to
            _fileStream = new FileStream(_filePath, FileMode.Open, FileAccess.Read, FileShare.ReadWrite, BufferSize);
        }

        public void SetEncoding(Encoding encoding)
        {
            _encoding = encoding;
            _currentEncodingIndex = Array.FindIndex(SupportedEncodings, e => e.CodePage == encoding.CodePage);
            if (_currentEncodingIndex < 0) _currentEncodingIndex = 0;
            StartIndexing();
        }

        public void CycleEncoding()
        {
            _currentEncodingIndex = (_currentEncodingIndex + 1) % SupportedEncodings.Length;
            _encoding = SupportedEncodings[_currentEncodingIndex];
            StartIndexing();
        }

        /// <summary>
        /// Starts or restarts the line indexing background task.
        /// </summary>
        public void StartIndexing()
        {
            CancelIndexing();

            _indexingCts = new CancellationTokenSource();
            _isIndexing = true;
            lock (_lineOffsets)
            {
                _lineOffsets.Clear();
                _lineOffsets.Add(0);
            }
            Interlocked.Exchange(ref _indexedLength, 0);

            Task.Run(() => IndexFileWorker(_indexingCts.Token));
        }

        public void CancelIndexing()
        {
            if (_indexingCts != null)
            {
                _indexingCts.Cancel();
                _indexingCts.Dispose();
                _indexingCts = null;
            }
            _isIndexing = false;
        }

        private void IndexFileWorker(CancellationToken token)
        {
            try
            {
                System.Diagnostics.Debug.WriteLine($"[LargeFileEngine] Starting indexing for {_filePath} (Size: {FileSize})");
                using (var fs = new FileStream(_filePath, FileMode.Open, FileAccess.Read, FileShare.ReadWrite, BufferSize))
                {
                    byte[] buffer = new byte[BufferSize];
                    long position = 0;
                    int bytesRead;

                    while ((bytesRead = fs.Read(buffer, 0, buffer.Length)) > 0)
                    {
                        if (token.IsCancellationRequested) 
                        {
                            System.Diagnostics.Debug.WriteLine("[LargeFileEngine] Indexing cancelled.");
                            break;
                        }

                        for (int i = 0; i < bytesRead; i++)
                        {
                            if (buffer[i] == 10) // \n
                            {
                                lock (_lineOffsets)
                                {
                                    _lineOffsets.Add(position + i + 1);
                                }
                            }
                        }

                        position += bytesRead;
                        Interlocked.Exchange(ref _indexedLength, position);

                        // Throttle progress updates
                        if (position % (1024 * 1024) == 0) // Every 1MB
                        {
                            System.Diagnostics.Debug.WriteLine($"[LargeFileEngine] Indexing progress: {_indexedLength}/{FileSize} ({IndexingProgress:P1})");
                            IndexingProgressChanged?.Invoke(this, EventArgs.Empty);
                        }
                    }
                }
            }
            catch (Exception ex)
            {
                System.Diagnostics.Debug.WriteLine($"[LargeFileEngine] Indexing error: {ex.Message}");
            }
            finally
            {
                _isIndexing = false;
                System.Diagnostics.Debug.WriteLine($"[LargeFileEngine] Indexing completed/stopped. Final lines: {_lineOffsets.Count}");
                IndexingCompleted?.Invoke(this, EventArgs.Empty);
            }
        }

        /// <summary>
        /// Reads lines for text view.
        /// </summary>
        public List<string> GetTextLines(int startLine, int count)
        {
            var lines = new List<string>();
            if (_fileStream == null) return lines;

            long startOffset = 0;
            long endOffset = 0;

            lock (_lineOffsets)
            {
                if (startLine >= _lineOffsets.Count) return lines; 
                startOffset = _lineOffsets[startLine];

                int lastLineIndex = startLine + count;
                if (lastLineIndex < _lineOffsets.Count)
                {
                    endOffset = _lineOffsets[lastLineIndex];
                }
                else
                {
                    endOffset = FileSize; 
                }
            }

            int length = (int)(endOffset - startOffset);
            if (length <= 0) return lines;
            if (length > 10 * 1024 * 1024) length = 10 * 1024 * 1024; 

            byte[] buffer = new byte[length];
            lock (_streamLock)
            {
                _fileStream.Seek(startOffset, SeekOrigin.Begin);
                _fileStream.Read(buffer, 0, length);
            }

            string text = _encoding.GetString(buffer);
            using (var reader = new StringReader(text))
            {
                string? line;
                while ((line = reader.ReadLine()) != null && lines.Count < count)
                {
                    lines.Add(line);
                }
            }

            return lines;
        }

        /// <summary>
        /// Reads bytes for hex view.
        /// </summary>
        public byte[] GetBytes(long offset, int length)
        {
            if (_fileStream == null) return Array.Empty<byte>();

            byte[] buffer = new byte[length];
            int read = 0;
            lock (_streamLock)
            {
                if (offset >= _fileStream.Length) return Array.Empty<byte>();
                _fileStream.Seek(offset, SeekOrigin.Begin);
                read = _fileStream.Read(buffer, 0, length);
            }

            if (read < length)
            {
                Array.Resize(ref buffer, read);
            }
            return buffer;
        }

        /// <summary>
        /// Synchronously finds all occurrences of a pattern. (Mainly for compatibility/tests)
        /// </summary>
        public List<int> Search(string pattern)
        {
            var matches = new List<int>();
            if (string.IsNullOrEmpty(pattern) || _fileStream == null) return matches;

            byte[] buffer = new byte[1024 * 1024]; 
            long position = 0;
            
            lock (_streamLock)
            {
                _fileStream.Seek(0, SeekOrigin.Begin);
            }

            while (position < FileSize)
            {
                int bytesRead;
                lock (_streamLock)
                {
                    _fileStream.Seek(position, SeekOrigin.Begin);
                    bytesRead = _fileStream.Read(buffer, 0, buffer.Length);
                }
                if (bytesRead == 0) break;

                string chunk = _encoding.GetString(buffer, 0, bytesRead);
                int matchIndex = chunk.IndexOf(pattern, StringComparison.OrdinalIgnoreCase);
                
                while (matchIndex >= 0)
                {
                    long matchAbsOffset = position + _encoding.GetByteCount(chunk.Substring(0, matchIndex));
                    matches.Add((int)GetLineNumberFromOffset(matchAbsOffset));
                    matchIndex = chunk.IndexOf(pattern, matchIndex + 1, StringComparison.OrdinalIgnoreCase);
                }
                position += bytesRead;
            }
            return matches;
        }

        /// <summary>
        /// Asynchronously finds the next occurrence of a pattern or regex.
        /// </summary>
        public async Task<long?> FindNextAsync(string pattern, long startLineIndex, bool searchBackwards, bool useRegex, CancellationToken token)
        {
            if (string.IsNullOrEmpty(pattern) || _fileStream == null) return null;

            return await Task.Run<long?>(() => 
            {
                long startByteOffset = 0;
                lock (_lineOffsets)
                {
                    if (startLineIndex < _lineOffsets.Count)
                        startByteOffset = _lineOffsets[(int)startLineIndex];
                    else
                        startByteOffset = _lineOffsets[_lineOffsets.Count - 1];
                }

                byte[] buffer = new byte[1024 * 1024]; 
                long position = startByteOffset;
                int overlap = _encoding.GetMaxByteCount(pattern.Length);
                
                System.Text.RegularExpressions.Regex? regex = null;
                if (useRegex)
                {
                    try { regex = new System.Text.RegularExpressions.Regex(pattern, System.Text.RegularExpressions.RegexOptions.IgnoreCase); } 
                    catch { return null; }
                }

                if (!searchBackwards)
                {
                    while (position < FileSize)
                    {
                        if (token.IsCancellationRequested) return null;

                        int bytesRead;
                        lock (_streamLock)
                        {
                            _fileStream.Seek(position, SeekOrigin.Begin);
                            bytesRead = _fileStream.Read(buffer, 0, buffer.Length);
                        }
                        if (bytesRead == 0) break;

                        string chunk = _encoding.GetString(buffer, 0, bytesRead);
                        int matchIndex = -1;

                        if (useRegex && regex != null)
                        {
                            var match = regex.Match(chunk);
                            if (match.Success) matchIndex = match.Index;
                        }
                        else
                        {
                            matchIndex = chunk.IndexOf(pattern, StringComparison.OrdinalIgnoreCase);
                        }

                        if (matchIndex >= 0)
                        {
                            long matchAbsOffset = position + _encoding.GetByteCount(chunk.Substring(0, matchIndex));
                            return GetLineNumberFromOffset(matchAbsOffset);
                        }
                        position += Math.Max(1, bytesRead - overlap); 
                    }
                }
                else
                {
                    while (position > 0)
                    {
                        if (token.IsCancellationRequested) return null;

                        long readStart = Math.Max(0, position - buffer.Length);
                        int bytesToRead = (int)(position - readStart);
                        
                        lock (_streamLock)
                        {
                            _fileStream.Seek(readStart, SeekOrigin.Begin);
                            _fileStream.Read(buffer, 0, bytesToRead);
                        }

                        string chunk = _encoding.GetString(buffer, 0, bytesToRead);
                        int matchIndex = -1;

                        if (useRegex && regex != null)
                        {
                            var matches = regex.Matches(chunk);
                            if (matches.Count > 0) matchIndex = matches[matches.Count - 1].Index; 
                        }
                        else
                        {
                            matchIndex = chunk.LastIndexOf(pattern, StringComparison.OrdinalIgnoreCase);
                        }

                        if (matchIndex >= 0)
                        {
                            long matchAbsOffset = readStart + _encoding.GetByteCount(chunk.Substring(0, matchIndex));
                            return GetLineNumberFromOffset(matchAbsOffset);
                        }
                        if (readStart == 0) break;
                        position = readStart + Math.Min(bytesToRead - 1, overlap);
                    }
                }

                return null;
            });
        }

        private long GetLineNumberFromOffset(long offset)
        {
            long currentOffset = 0;
            long currentLine = 0;

            lock (_lineOffsets)
            {
                if (_lineOffsets.Count > 0 && Interlocked.Read(ref _indexedLength) >= offset)
                {
                    int index = _lineOffsets.BinarySearch(offset);
                    if (index < 0) index = ~index - 1;
                    return Math.Max(0, index);
                }

                if (_lineOffsets.Count > 0)
                {
                    currentLine = _lineOffsets.Count - 1;
                    currentOffset = _lineOffsets[(int)currentLine];
                }
            }

            if (offset <= currentOffset) return currentLine;

            try
            {
                using (var fs = new FileStream(_filePath, FileMode.Open, FileAccess.Read, FileShare.ReadWrite, BufferSize))
                {
                    fs.Seek(currentOffset, SeekOrigin.Begin);
                    byte[] buffer = new byte[BufferSize];
                    long pos = currentOffset;
                    
                        while (pos < offset)
                        {
                            int toRead = (int)Math.Min(buffer.Length, offset - pos);
                            int read = fs.Read(buffer, 0, toRead);
                            if (read <= 0) break;

                            for (int i = 0; i < read; i++)
                            {
                                if (buffer[i] == 10)
                                {
                                    currentLine++;
                                    lock (_lineOffsets)
                                    {
                                        long newOffset = pos + i + 1;
                                        if (!_lineOffsets.Contains(newOffset)) _lineOffsets.Add(newOffset);
                                    }
                                }
                            }
                            pos += read;
                        }
                        Interlocked.Exchange(ref _indexedLength, pos);
                    }
                }
            catch { }

            return currentLine;
        }

        public void Dispose()
        {
            CancelIndexing();
            _fileStream?.Dispose();
        }
    }
}
