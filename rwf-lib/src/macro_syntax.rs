//! Pure text checks on macro syntax (moved down from `macro_expander` so config
//! validation in `dialog` need not depend on the expander, which needs `AppState`).

/// True if `command` still uses the removed `$M` macro.
///
/// `$M` was replaced by the systematic `$MFS`/`$MPS`/`$MFL`/`$MPL` set. A leftover
/// `$M` would not error on its own — it would fall through to bare-`$VAR` env
/// expansion, find no variable named `M`, and survive as the literal text `$M` in
/// the command. Config loading calls this so the removal fails loudly instead.
pub fn has_stale_marked_macro(command: &str) -> bool {
    let bytes = command.as_bytes();
    let mut search_from = 0;
    while let Some(offset) = command[search_from..].find("$M") {
        let at = search_from + offset;
        // Valid forms are exactly $M + {F,P} + {S,L}
        let valid = matches!(bytes.get(at + 2), Some(b'F') | Some(b'P'))
            && matches!(bytes.get(at + 3), Some(b'S') | Some(b'L'));
        if !valid {
            return true;
        }
        search_from = at + 2;
    }
    false
}
