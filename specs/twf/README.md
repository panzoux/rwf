# TWF - Two-pane Window Filer

A comprehensive keyboard-driven, dual-pane file manager application for Windows, inspired by the classic AFxW file manager.

## Key Features

- **Dual-Pane Interface**: Effortless file management between two directories.
- **Background Operations**: Non-blocking Copy, Move, and Delete tasks with a real-time progress log and Job Manager.
- **High Performance & Caching**: Asynchronous directory listing with memory-efficient structures and smart caching for instant navigation.
- **Enhanced Tab Management**: Tabbed browsing with horizontal scrolling, custom colors, and a filterable Tab Selector Dialog for high-density workflows.
- **Power User Tools**: Custom macro-based functions, wildcard marking, and regex filtering.
- **Archive Support**: Browse and extract archives as virtual folders.

## Project Structure

```
TWF/
├── Controllers/         # UI controllers and orchestration
├── Models/             # Data models and entities
├── Services/           # Business logic services
├── Providers/          # Data access and external integrations
├── UI/                 # UI components
├── Infrastructure/     # Cross-cutting concerns (logging, etc.)
├── old/                # Legacy code (to be migrated)
└── Program.cs          # Application entry point
```

## Dependencies

- .NET 8.0
- Terminal.Gui 1.19.0
- Microsoft.Extensions.Logging 8.0.0
- Microsoft.Extensions.Logging.Console 8.0.0
- Microsoft.Extensions.Logging.Debug 8.0.0

## Building

```bash
dotnet restore
dotnet build
```

```bash
dotnet build
dotnet build -c Release
dotnet publish -c Release -r win-x64 --self-contained false -o bin/publish   
```

```bash
dotnet build
dotnet build -c Release
dotnet publish -c Release -r win-x64 --self-contained false -o bin/publish2
```

## Running

```bash
dotnet run
```

## Logging

Application logs are written to:
- Console output (during development)
- File: `%APPDATA%\TWF\twf_errors.log`

Log files are automatically rotated when they exceed 10MB.

## Development Status

This project is currently under active development following a spec-driven approach. See `.kiro/specs/twf-file-manager/` for detailed requirements, design, and implementation tasks.
