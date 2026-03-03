using System.Text;
using TWF.Models;

namespace TWF.Services
{
    /// <summary>
    /// Manages text and image viewers
    /// </summary>
    public class ViewerManager
    {
        private LargeFileEngine? _currentFileEngine;
        private ImageViewer? _currentImageViewer;
        private readonly SearchEngine _searchEngine;

        public ViewerManager(SearchEngine searchEngine)
        {
            _searchEngine = searchEngine ?? throw new ArgumentNullException(nameof(searchEngine));
        }

        /// <summary>
        /// Opens a text file in the text viewer (using LargeFileEngine)
        /// </summary>
        /// <param name="filePath">Path to the text file</param>
        /// <param name="encoding">Initial encoding to use</param>
        public void OpenTextViewer(string filePath, Encoding? encoding = null)
        {
            CloseCurrentViewer();
            _currentFileEngine = new LargeFileEngine(filePath);
            _currentFileEngine.Initialize(encoding);
            // Start indexing in background
            _currentFileEngine.StartIndexing();
        }

        /// <summary>
        /// Opens an image file in the image viewer
        /// </summary>
        /// <param name="filePath">Path to the image file</param>
        public void OpenImageViewer(string filePath)
        {
            CloseCurrentViewer();
            _currentImageViewer = new ImageViewer();
            _currentImageViewer.LoadImage(filePath);
        }

        /// <summary>
        /// Closes the currently open viewer
        /// </summary>
        public void CloseCurrentViewer()
        {
            if (_currentFileEngine != null)
            {
                _currentFileEngine.Dispose();
                _currentFileEngine = null;
            }
            _currentImageViewer = null;
        }

        /// <summary>
        /// Gets the current file engine if one is open
        /// </summary>
        public LargeFileEngine? CurrentTextViewer => _currentFileEngine;

        /// <summary>
        /// Gets the current image viewer if one is open
        /// </summary>
        public ImageViewer? CurrentImageViewer => _currentImageViewer;

        /// <summary>
        /// Gets the search engine
        /// </summary>
        public SearchEngine SearchEngine => _searchEngine;
    }

    // TextViewer class removed in favor of LargeFileEngine


    /// <summary>
    /// Image file viewer with zoom and rotation
    /// </summary>
    public class ImageViewer
    {
        private string _filePath = string.Empty;
        private ViewMode _viewMode = ViewMode.FitToScreen;
        private int _rotation = 0;
        private bool _flipHorizontal = false;
        private bool _flipVertical = false;
        private double _zoomFactor = 1.0;

        /// <summary>
        /// Gets the current file path
        /// </summary>
        public string FilePath => _filePath;

        /// <summary>
        /// Gets the current view mode
        /// </summary>
        public ViewMode ViewMode => _viewMode;

        /// <summary>
        /// Gets the current rotation in degrees
        /// </summary>
        public int Rotation => _rotation;

        /// <summary>
        /// Gets whether the image is flipped horizontally
        /// </summary>
        public bool FlipHorizontal => _flipHorizontal;

        /// <summary>
        /// Gets whether the image is flipped vertically
        /// </summary>
        public bool FlipVertical => _flipVertical;

        /// <summary>
        /// Gets the current zoom factor
        /// </summary>
        public double ZoomFactor => _zoomFactor;

        /// <summary>
        /// Loads an image file
        /// </summary>
        /// <param name="path">Path to the image file</param>
        public void LoadImage(string path)
        {
            if (!File.Exists(path))
            {
                throw new FileNotFoundException($"Image file not found: {path}");
            }

            _filePath = path;
            _viewMode = ViewMode.FitToScreen;
            _rotation = 0;
            _flipHorizontal = false;
            _flipVertical = false;
            _zoomFactor = 1.0;
        }

        /// <summary>
        /// Rotates the image by the specified degrees
        /// </summary>
        /// <param name="degrees">Degrees to rotate (typically 90, 180, 270)</param>
        public void Rotate(int degrees)
        {
            _rotation = (_rotation + degrees) % 360;
            if (_rotation < 0)
            {
                _rotation += 360;
            }
        }

        /// <summary>
        /// Flips the image in the specified direction
        /// </summary>
        /// <param name="direction">Direction to flip</param>
        public void Flip(FlipDirection direction)
        {
            switch (direction)
            {
                case FlipDirection.Horizontal:
                    _flipHorizontal = !_flipHorizontal;
                    break;
                case FlipDirection.Vertical:
                    _flipVertical = !_flipVertical;
                    break;
            }
        }

        /// <summary>
        /// Zooms the image by the specified factor
        /// </summary>
        /// <param name="factor">Zoom factor (1.0 = 100%, 2.0 = 200%, etc.)</param>
        public void Zoom(double factor)
        {
            if (factor > 0)
            {
                _zoomFactor = factor;
                _viewMode = ViewMode.FixedZoom;
            }
        }

        /// <summary>
        /// Sets the view mode
        /// </summary>
        /// <param name="mode">View mode to set</param>
        public void SetViewMode(ViewMode mode)
        {
            _viewMode = mode;
            
            // Reset zoom factor for non-fixed zoom modes
            if (mode != ViewMode.FixedZoom)
            {
                _zoomFactor = 1.0;
            }
        }
    }

    /// <summary>
    /// Direction for flipping images
    /// </summary>
    public enum FlipDirection
    {
        Horizontal,
        Vertical
    }
}
