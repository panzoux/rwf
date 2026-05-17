pub enum AppEvent {
    Input(crossterm::event::KeyEvent),
    Job(crate::worker_pool::JobEvent),
}
