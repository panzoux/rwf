use chrono::{DateTime, Local};

pub struct StatusMessage {
    pub timestamp: DateTime<Local>,
    pub text: String,
}

pub struct StatusMessageModel {
    messages: Vec<StatusMessage>,
    max_lines: usize,
}

impl StatusMessageModel {
    pub fn new(max_lines: usize) -> Self {
        Self {
            messages: Vec::new(),
            max_lines,
        }
    }

    pub fn push(&mut self, message: StatusMessage) {
        self.messages.push(message);
        if self.messages.len() > self.max_lines {
            self.messages.remove(0);
        }
    }

    pub fn messages(&self) -> &[StatusMessage] {
        &self.messages
    }
}
