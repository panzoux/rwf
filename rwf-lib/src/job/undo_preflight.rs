//! Pre-flight validation for Undo/Redo (Phase 7.6).
//!
//! Checked *before* submitting a reversal job, so the user sees "N will be
//! reversed / M blocked, with reasons" up front — see
//! `plan/7.6.operation_report_ui.md` "事前検証".

use crate::model::ReversalAction;

/// Outcome of checking one `ReversalAction` against the current filesystem
/// state, immediately before it would run.
#[derive(Debug, Clone, PartialEq)]
pub enum PreflightOutcome {
    Ready,
    Blocked(String),
}

/// Splits `actions` into what's safe to run now vs. what's currently
/// blocked, with a reason for each blocked one. Pure filesystem existence
/// checks only (`std::fs::metadata`) — this does not touch the async
/// backend trait, since these are synchronous, cheap, local-path checks
/// (Location::Local is assumed here; non-local Locations pass through as
/// Ready, since remote backends don't support this Phase's pre-flight yet).
pub fn preflight_check(
    actions: &[ReversalAction],
) -> (Vec<ReversalAction>, Vec<(ReversalAction, String)>) {
    let mut ready = Vec::new();
    let mut blocked = Vec::new();

    for action in actions {
        match check_one(action) {
            PreflightOutcome::Ready => ready.push(action.clone()),
            PreflightOutcome::Blocked(reason) => blocked.push((action.clone(), reason)),
        }
    }

    (ready, blocked)
}

fn check_one(action: &ReversalAction) -> PreflightOutcome {
    match action {
        ReversalAction::Copy { to, .. } | ReversalAction::CreateArchive { dest: to, .. } => {
            check_destination_not_occupied(to)
        }
        ReversalAction::Move { from, to } | ReversalAction::Rename { from, to } => {
            if !exists(from) {
                return PreflightOutcome::Blocked(format!("{} no longer exists", display(from)));
            }
            check_destination_not_occupied(to)
        }
        ReversalAction::Delete { target, .. } => {
            if !exists(target) {
                return PreflightOutcome::Blocked(format!("{} no longer exists", display(target)));
            }
            PreflightOutcome::Ready
        }
        ReversalAction::Mkdir { location } | ReversalAction::CreateFile { location } => {
            check_destination_not_occupied(location)
        }
        ReversalAction::CreateLink { link_path, .. } => check_destination_not_occupied(link_path),
        ReversalAction::RestoreAttributes { target, .. }
        | ReversalAction::RestoreTimestamps { target, .. }
        | ReversalAction::MoveToTrash { target } => {
            if !exists(target) {
                PreflightOutcome::Blocked(format!("{} no longer exists", display(target)))
            } else {
                PreflightOutcome::Ready
            }
        }
        ReversalAction::RestoreFromTrash { record } => {
            check_destination_not_occupied(&record.original)
        }
    }
}

fn check_destination_not_occupied(loc: &crate::model::Location) -> PreflightOutcome {
    // Non-local Location: can't check synchronously here — don't block on
    // something we can't verify (the opposite of `exists()`'s "assume it
    // exists" fallback below, which is right for the *must-still-exist*
    // family but would wrongly report every non-local destination as
    // already-occupied here if reused as-is).
    let Some(path) = loc.path() else {
        return PreflightOutcome::Ready;
    };
    if path.exists() {
        PreflightOutcome::Blocked(format!("{} already exists", display(loc)))
    } else {
        PreflightOutcome::Ready
    }
}

fn exists(loc: &crate::model::Location) -> bool {
    match loc.path() {
        Some(path) => path.exists(),
        // Non-local Location: can't check synchronously here — assume it
        // exists (don't block on something we can't verify).
        None => true,
    }
}

fn display(loc: &crate::model::Location) -> String {
    loc.display_path()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Location;
    use tempfile::TempDir;

    #[test]
    fn delete_action_ready_when_target_exists() {
        let dir = TempDir::new().unwrap();
        let target = dir.path().join("copy.txt");
        std::fs::write(&target, b"x").unwrap();

        let action = ReversalAction::Delete {
            target: Location::Local(target),
            recreate: None,
        };
        let (ready, blocked) = preflight_check(&[action]);
        assert_eq!(ready.len(), 1);
        assert!(blocked.is_empty());
    }

    #[test]
    fn delete_action_blocked_when_target_already_gone() {
        let dir = TempDir::new().unwrap();
        let missing = dir.path().join("nope.txt");

        let action = ReversalAction::Delete {
            target: Location::Local(missing),
            recreate: None,
        };
        let (ready, blocked) = preflight_check(&[action]);
        assert!(ready.is_empty());
        assert_eq!(blocked.len(), 1);
        assert!(blocked[0].1.contains("no longer exists"));
    }

    #[test]
    fn move_action_blocked_when_destination_already_exists() {
        let dir = TempDir::new().unwrap();
        let from = dir.path().join("here.txt");
        let to = dir.path().join("there.txt");
        std::fs::write(&from, b"x").unwrap();
        std::fs::write(&to, b"y").unwrap(); // occupies the move's target

        let action = ReversalAction::Move {
            from: Location::Local(from),
            to: Location::Local(to),
        };
        let (ready, blocked) = preflight_check(&[action]);
        assert!(ready.is_empty());
        assert_eq!(blocked.len(), 1);
        assert!(blocked[0].1.contains("already exists"));
    }

    #[test]
    fn non_local_destination_is_ready_not_blocked() {
        // check_destination_not_occupied's `exists()` fallback for a
        // non-local Location returns `true` ("assume it exists" — correct
        // for the *must-still-exist* family, e.g. Delete/RestoreAttributes).
        // Reusing that same fallback here would make every non-local
        // destination report as "already exists" and always be Blocked,
        // contradicting this module's own documented contract that
        // unverifiable Locations pass through as Ready.
        let action = ReversalAction::Mkdir {
            location: Location::Ssh {
                host: "example.com".to_string(),
                port: 22,
                path: "/remote/new_dir".into(),
            },
        };
        let (ready, blocked) = preflight_check(&[action]);
        assert_eq!(ready.len(), 1);
        assert!(blocked.is_empty());
    }

    #[test]
    fn mixed_batch_splits_ready_and_blocked() {
        let dir = TempDir::new().unwrap();
        let ok_target = dir.path().join("ok.txt");
        std::fs::write(&ok_target, b"x").unwrap();
        let missing_target = dir.path().join("missing.txt");

        let actions = vec![
            ReversalAction::Delete {
                target: Location::Local(ok_target),
                recreate: None,
            },
            ReversalAction::Delete {
                target: Location::Local(missing_target),
                recreate: None,
            },
        ];
        let (ready, blocked) = preflight_check(&actions);
        assert_eq!(ready.len(), 1);
        assert_eq!(blocked.len(), 1);
    }
}
