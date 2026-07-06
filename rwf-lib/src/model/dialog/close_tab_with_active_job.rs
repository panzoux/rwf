//! Close tab with active job dialog content.

#[derive(Debug, Clone)]
pub struct CloseTabWithActiveJobDialog {
    pub tab_index: usize,
    pub tab_name: String,
    pub job_ids: Vec<u32>,
    pub focused_field: usize,
}

impl CloseTabWithActiveJobDialog {
    pub fn new(
        tab_index: usize,
        tab_name: String,
        job_ids: Vec<u32>,
        focused_field: usize,
    ) -> Self {
        Self {
            tab_index,
            tab_name,
            job_ids,
            focused_field,
        }
    }
}
