using System;
using System.Diagnostics;
using System.IO;
using Terminal.Gui;
using Microsoft.Extensions.Logging;
using TWF.Infrastructure;

namespace TWF.Services
{
    public class EditorLauncher
    {
        private readonly ILogger<EditorLauncher> _logger;

        public EditorLauncher(ILogger<EditorLauncher>? logger = null)
        {
            _logger = logger ?? LoggingConfiguration.GetLogger<EditorLauncher>();
        }

        public string GetEditorCommand()
        {
            var editor = Environment.GetEnvironmentVariable("VISUAL")
                         ?? Environment.GetEnvironmentVariable("EDITOR");
            if (!string.IsNullOrWhiteSpace(editor)) return editor;
            return OperatingSystem.IsWindows() ? "notepad.exe" : "vim";
        }

        public int LaunchEditorAndWait(string filePath, string? preferredEditor = null)
        {
            // Handle legacy default "notepad.exe" on non-Windows systems by falling back to a sensible default
            if (!OperatingSystem.IsWindows() && 
                !string.IsNullOrWhiteSpace(preferredEditor) &&
                (preferredEditor.EndsWith("notepad.exe", StringComparison.OrdinalIgnoreCase) || 
                 preferredEditor.Equals("notepad", StringComparison.OrdinalIgnoreCase)))
            {
                preferredEditor = "vim";
            }

            var editorCmd = !string.IsNullOrWhiteSpace(preferredEditor) ? preferredEditor : GetEditorCommand();
            
            // On Windows, if preferredEditor is just "notepad", append ".exe" just in case, though Process.Start usually handles it.
            // But we can trust the input mostly.

            string prog;
            string args;
            
            // Simple split.
            var parts = editorCmd.Split(new[] { ' ' }, 2, StringSplitOptions.RemoveEmptyEntries);
            prog = parts[0];
            args = parts.Length > 1 ? parts[1] : "";
            
            // Append file path
            args = string.IsNullOrEmpty(args) ? Quote(filePath) : args + " " + Quote(filePath);

            int exitCode = -1;
            
            // Suspend Terminal.Gui to restore terminal for the editor
            if (Application.Driver != null)
            {
                Application.Driver.Suspend();
                // Inform user we are waiting, in case the editor is in a separate window (GUI)
                // or if there's a delay.
                Console.WriteLine("Launching editor...");
                Console.WriteLine("Waiting for editor to close...");
            }

            try {
                _logger.LogInformation("Launching editor: {Prog} {Args}", prog, args);
                using var p = new Process();
                p.StartInfo.FileName = prog;
                p.StartInfo.Arguments = args;
                
                // On Windows, GUI apps like Notepad don't block console unless we wait.
                // UseShellExecute=false is standard for modern .NET Core process handling. 
                p.StartInfo.UseShellExecute = false; 
                
                p.Start();
                p.WaitForExit();
                exitCode = p.ExitCode;
            } catch (Exception ex) {
                _logger.LogError(ex, "Failed to start editor '{Prog}'", prog);
                Console.WriteLine($"Failed to start editor '{prog}': {ex.Message}");
                Console.WriteLine("Press Enter to continue...");
                Console.ReadLine();
            } finally {
                if (Application.Driver != null)
                {
                    Application.Refresh();
                }
            }
            
            return exitCode;
        }

        private static string Quote(string s) => s.Contains(' ') ? $"\"{s}\"" : s;
    }
}
