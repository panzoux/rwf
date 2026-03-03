//! Navigation history

use super::{Location, ui::ActivePane};

/// Navigation history per pane
#[derive(Debug)]
pub struct NavigationHistory {
    pub left_stack: Vec<Location>,
    pub right_stack: Vec<Location>,
    pub left_pos: usize,
    pub right_pos: usize,
}

impl NavigationHistory {
    pub fn new() -> Self {
        Self {
            left_stack: Vec::new(),
            right_stack: Vec::new(),
            left_pos: 0,
            right_pos: 0,
        }
    }
    
    /// Push a new location to history
    pub fn push(&mut self, pane: ActivePane, location: Location) {
        match pane {
            ActivePane::Left => {
                // Truncate forward history
                self.left_stack.truncate(self.left_pos + 1);
                self.left_stack.push(location);
                self.left_pos = self.left_stack.len() - 1;
            }
            ActivePane::Right => {
                self.right_stack.truncate(self.right_pos + 1);
                self.right_stack.push(location);
                self.right_pos = self.right_stack.len() - 1;
            }
        }
    }
    
    /// Go back in history
    pub fn go_back(&mut self, pane: ActivePane) -> Option<Location> {
        match pane {
            ActivePane::Left => {
                if self.left_pos > 0 {
                    self.left_pos -= 1;
                    self.left_stack.get(self.left_pos).cloned()
                } else {
                    None
                }
            }
            ActivePane::Right => {
                if self.right_pos > 0 {
                    self.right_pos -= 1;
                    self.right_stack.get(self.right_pos).cloned()
                } else {
                    None
                }
            }
        }
    }
    
    /// Go forward in history
    pub fn go_forward(&mut self, pane: ActivePane) -> Option<Location> {
        match pane {
            ActivePane::Left => {
                if self.left_pos + 1 < self.left_stack.len() {
                    self.left_pos += 1;
                    self.left_stack.get(self.left_pos).cloned()
                } else {
                    None
                }
            }
            ActivePane::Right => {
                if self.right_pos + 1 < self.right_stack.len() {
                    self.right_pos += 1;
                    self.right_stack.get(self.right_pos).cloned()
                } else {
                    None
                }
            }
        }
    }
}
