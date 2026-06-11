# /project:e2e — End-to-End Feature Verification

Run a systematic end-to-end check on the feature or area named in the argument (e.g. `/project:e2e config menu`, `/project:e2e custom functions`, `/project:e2e keybindings`). If no argument is given, check the most recently changed feature area based on git log.

## Steps to execute

### 1. Read the real user config
Read these files (not the `/sample/` copies):
- `C:\Users\user\AppData\Roaming\rwf\custom_functions.json`
- `C:\Users\user\AppData\Roaming\rwf\keybindings.json`
- Any `menu_*.json` files in the same directory

Look for problems before running anything.

### 2. Macro collision audit
Scan every `Command` string in custom_functions.json for bare `$VAR` patterns.
Flag any where the variable name starts with: **P, O, L, R, F, W, E, M** — these are RWF
single-letter macros that expand before env var expansion and will silently corrupt the command.
Recommend `${VAR}` or `$env:VAR` as replacements.

### 3. Trace the full user-facing flow
For the feature being tested, trace the complete path:
- Which key(s) trigger it?
- What dialog(s) open?
- What action/transition fires?
- What job is spawned (if any)?
- What is the final visible result?

Identify every hand-off point and check each one.

### 4. External program check
If the feature spawns an external program (editor, shell, viewer), verify:
- The path/argument passed is a fully expanded string, not a raw macro
- The program exists at that path on this machine
- Quoting is correct for paths with spaces

### 5. Dialog stack check
After the feature's action chain completes:
- Are all opened dialogs popped?
- Is focus returned to the correct pane?
- Is there any path through the flow that could leave a ghost dialog?

### 6. TWF parity check
Search `specs/twf/` for documentation of this feature. List anything the spec describes
that is not yet implemented in RWF. Note it explicitly — do not silently skip unimplemented items.

### 7. Report
Produce a concise report:
- **Working**: what was confirmed working
- **Bugs found**: specific file + line references
- **Not implemented**: TWF spec items missing from RWF
- **Recommendations**: config changes, code fixes, or follow-up tasks
