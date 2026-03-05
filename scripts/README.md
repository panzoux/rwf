# Shell Integration Scripts for RWF

These wrapper scripts enable "cd on exit" functionality, allowing your shell's working directory to change to the last directory you were viewing in RWF when you exit the application.

## Features

- **Bash/Zsh**: Use `rwf-cd.sh` or `rwf-cd.zsh`
- **PowerShell**: Use `rwf-cd.ps1`
- **Automatic directory change**: When you exit RWF with `Shift+Q`, your shell changes to the active pane's directory
- **Manual mode**: Use the `-cwd` flag to enable directory change on any exit

## Installation

### Bash

Add to your `~/.bashrc`:

```bash
source /path/to/rwf/scripts/rwf-cd.sh
alias rwf='rwf_cd'
```

Then reload your configuration:
```bash
source ~/.bashrc
```

### Zsh

Add to your `~/.zshrc`:

```zsh
source /path/to/rwf/scripts/rwf-cd.zsh
alias rwf='rwf_cd'
```

Then reload your configuration:
```zsh
source ~/.zshrc
```

### PowerShell

1. Open your PowerShell profile:
   ```powershell
   notepad $PROFILE
   ```

2. Add these lines:
   ```powershell
   . C:\path\to\rwf\scripts\rwf-cd.ps1
   Set-Alias rwf Invoke-RwfCd
   ```

3. Reload your profile:
   ```powershell
   . $PROFILE
   ```

## Usage

### With Wrapper Scripts

Once installed, simply run:
```bash
rwf
```

When you exit with `Shift+Q`, your shell will automatically change to the directory you were viewing in the active pane.

### Manual Mode (Without Wrapper)

You can also use the `-cwd` flag directly:
```bash
cd $(rwf --cwd)
```

This will change to the directory you were viewing when you exit RWF (with any exit method).

## How It Works

1. The wrapper script runs RWF with the `--cwd` flag
2. When you exit RWF (especially with `Shift+Q`), it outputs the current active pane's directory to stdout
3. The wrapper script captures this output and uses `cd` to change to that directory
4. Your shell's working directory is now synchronized with where you were in RWF

## Key Bindings

- `Q` or `Escape`: Normal quit (no directory change without `-cwd` flag)
- `Shift+Q`: Exit and change directory (outputs directory even without `-cwd` flag)

## Troubleshooting

### Directory not changing

- Make sure you've sourced the wrapper script in your shell configuration
- Verify the alias is set correctly: `alias rwf`
- Check that RWF is in your PATH: `which rwf`

### PowerShell execution policy

If you get an execution policy error, you may need to allow script execution:
```powershell
Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser
```

## Examples

```bash
# Start RWF
rwf

# Navigate to a directory in RWF
# Press Shift+Q to exit

# Your shell is now in that directory
pwd  # Shows the directory you were viewing in RWF
```
