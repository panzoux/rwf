//! Extension-association resolution helpers (moved down from `input` so the
//! job-completion handlers in `state` need not depend on `input`).

use crate::model::Location;
use crate::AppState;

/// Expand an `ExtensionAssociation`'s command via `MacroExpander`, returning
/// `(command, working_dir, shell)` on success. Shared by
/// `resolve_extension_association`'s single-match branch and the Open With
/// picker's Confirm handler (`rwf-bin/src/ui/dialog/confirm.rs`) so the two
/// expansion call sites can't drift apart (Phase 7.3).
pub fn expand_association_command(
    state: &AppState,
    assoc: &crate::config::ExtensionAssociation,
) -> Result<(String, Location, Option<String>), String> {
    let expander = crate::state::macro_expander::MacroExpander::new();
    let func = crate::custom_function::CustomFunction::new("open", &assoc.command);
    let func = if let Some(ref shell) = assoc.shell {
        func.with_shell(shell)
    } else {
        func
    };
    let command = expander.expand(state, &func)?;
    let working_dir = state.active_pane().current_location.clone();
    let shell = assoc.shell.clone();
    Ok((command, working_dir, shell))
}

/// Resolve the pure-extension `ExtensionAssociation`(s) whose extension matches
/// `ext_lower` (already lowercased, no leading dot). "Pure-extension" means
/// `file_type.is_none()` — a `FileType`-bearing entry can't be validated without
/// magic-byte detection, so it's excluded here (Phase 7.3b). Shared by:
/// - `resolve_extension_association`'s flag-off / non-Local branch,
/// - `candidates_for`'s fallback step (once a detected kind is known but no
///   FileType entry matched it),
/// - the `DetectFileTypesBatch` completion handler's failure path.
pub fn candidates_for_extension(
    state: &AppState,
    ext_lower: &str,
) -> Vec<crate::config::ExtensionAssociation> {
    state
        .extension_associations
        .iter()
        .filter(|a| {
            a.file_type.is_none()
                && a.extension
                    .as_deref()
                    .map(|e| e.trim_start_matches('.').to_lowercase() == ext_lower)
                    .unwrap_or(false)
        })
        .cloned()
        .collect()
}

/// Resolve `ExtensionAssociation` candidates using both the detected content
/// type and the extension (Phase 7.3b detect-then-resolve pipeline). Shared by
/// the `DetectFileType { ResolveAssociation }` completion handler (single
/// cursor-file / Open With flow) and the `DetectFileTypesBatch` completion
/// handler (batch "Open With..." flow), so the two resolution call sites can't
/// drift apart on the FileType-first/extension-fallback/AND rules.
///
/// - `kind != Unknown`: entries whose `file_type` matches `kind` (via
///   `DetectedKind::matches_file_type_spec`) AND whose `extension` is either
///   unset or matches `ext_lower` are preferred. If that set is non-empty, it's
///   returned as-is.
/// - Otherwise (no FileType match, or `kind == Unknown`): falls back to
///   pure-extension entries via `candidates_for_extension`.
pub fn candidates_for(
    state: &AppState,
    kind: crate::magic::DetectedKind,
    ext_lower: &str,
) -> Vec<crate::config::ExtensionAssociation> {
    if kind != crate::magic::DetectedKind::Unknown {
        let type_matches: Vec<_> = state
            .extension_associations
            .iter()
            .filter(|a| {
                a.file_type
                    .as_deref()
                    .map(|ft| kind.matches_file_type_spec(ft))
                    .unwrap_or(false)
                    && a.extension
                        .as_deref()
                        .map(|e| e.trim_start_matches('.').to_lowercase() == ext_lower)
                        .unwrap_or(true)
            })
            .cloned()
            .collect();
        if !type_matches.is_empty() {
            return type_matches;
        }
    }
    candidates_for_extension(state, ext_lower)
}
