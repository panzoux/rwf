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

impl Default for NavigationHistory {
    fn default() -> Self {
        Self::new()
    }
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
    
    const MAX_HISTORY: usize = 100;

    /// Push a new location to history
    pub fn push(&mut self, pane: ActivePane, location: Location) {
        match pane {
            ActivePane::Left => {
                self.left_stack.truncate(self.left_pos + 1);
                self.left_stack.push(location);
                if self.left_stack.len() > Self::MAX_HISTORY {
                    self.left_stack.remove(0);
                }
                self.left_pos = self.left_stack.len() - 1;
            }
            ActivePane::Right => {
                self.right_stack.truncate(self.right_pos + 1);
                self.right_stack.push(location);
                if self.right_stack.len() > Self::MAX_HISTORY {
                    self.right_stack.remove(0);
                }
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

    /// Swap left and right pane history
    pub fn swap_panes(&mut self) {
        std::mem::swap(&mut self.left_stack, &mut self.right_stack);
        std::mem::swap(&mut self.left_pos, &mut self.right_pos);
    }

    /// Get the history stack and current position for the given pane
    pub fn stack_and_pos(&self, pane: ActivePane) -> (&[Location], usize) {
        match pane {
            ActivePane::Left  => (&self.left_stack,  self.left_pos),
            ActivePane::Right => (&self.right_stack, self.right_pos),
        }
    }

    /// Set the current history position and return the location at that index
    pub fn jump_to_index(&mut self, pane: ActivePane, index: usize) -> Option<Location> {
        match pane {
            ActivePane::Left => {
                if index < self.left_stack.len() {
                    self.left_pos = index;
                    self.left_stack.get(index).cloned()
                } else {
                    None
                }
            }
            ActivePane::Right => {
                if index < self.right_stack.len() {
                    self.right_pos = index;
                    self.right_stack.get(index).cloned()
                } else {
                    None
                }
            }
        }
    }
}
