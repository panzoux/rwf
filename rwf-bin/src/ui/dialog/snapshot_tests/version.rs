//! Snapshots for `DialogContent::Version`.

use super::{snapshot_dialog, test_state};
use rwf_lib::model::dialog::Dialog;

#[test]
fn version_default() {
    let state = test_state();
    let dialog = Dialog::version();
    snapshot_dialog("version_default", &dialog, &state);
}
