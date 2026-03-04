//! Performance profiling module
//!
//! This module provides utilities for profiling UI rendering and input processing performance.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Performance metrics for UI rendering
#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    /// Frame times for FPS calculation (last 60 frames)
    frame_times: VecDeque<Duration>,
    /// Input processing times (last 100 events)
    input_times: VecDeque<Duration>,
    /// Last frame start time
    last_frame_start: Option<Instant>,
    /// Total frames rendered
    total_frames: u64,
    /// Total input events processed
    total_inputs: u64,
}

impl PerformanceMetrics {
    /// Create a new performance metrics tracker
    pub fn new() -> Self {
        Self {
            frame_times: VecDeque::with_capacity(60),
            input_times: VecDeque::with_capacity(100),
            last_frame_start: None,
            total_frames: 0,
            total_inputs: 0,
        }
    }

    /// Start timing a frame
    pub fn start_frame(&mut self) {
        self.last_frame_start = Some(Instant::now());
    }

    /// End timing a frame and record the duration
    pub fn end_frame(&mut self) {
        if let Some(start) = self.last_frame_start.take() {
            let duration = start.elapsed();
            
            // Keep only last 60 frame times
            if self.frame_times.len() >= 60 {
                self.frame_times.pop_front();
            }
            self.frame_times.push_back(duration);
            
            self.total_frames += 1;
        }
    }

    /// Record input processing time
    pub fn record_input_time(&mut self, duration: Duration) {
        // Keep only last 100 input times
        if self.input_times.len() >= 100 {
            self.input_times.pop_front();
        }
        self.input_times.push_back(duration);
        
        self.total_inputs += 1;
    }

    /// Get current FPS (frames per second)
    pub fn current_fps(&self) -> f64 {
        if self.frame_times.is_empty() {
            return 0.0;
        }

        let total_time: Duration = self.frame_times.iter().sum();
        let avg_frame_time = total_time.as_secs_f64() / self.frame_times.len() as f64;
        
        if avg_frame_time > 0.0 {
            1.0 / avg_frame_time
        } else {
            0.0
        }
    }

    /// Get average frame time in milliseconds
    pub fn avg_frame_time_ms(&self) -> f64 {
        if self.frame_times.is_empty() {
            return 0.0;
        }

        let total_time: Duration = self.frame_times.iter().sum();
        total_time.as_secs_f64() * 1000.0 / self.frame_times.len() as f64
    }

    /// Get maximum frame time in milliseconds
    pub fn max_frame_time_ms(&self) -> f64 {
        self.frame_times
            .iter()
            .max()
            .map(|d| d.as_secs_f64() * 1000.0)
            .unwrap_or(0.0)
    }

    /// Get average input processing time in milliseconds
    pub fn avg_input_time_ms(&self) -> f64 {
        if self.input_times.is_empty() {
            return 0.0;
        }

        let total_time: Duration = self.input_times.iter().sum();
        total_time.as_secs_f64() * 1000.0 / self.input_times.len() as f64
    }

    /// Get maximum input processing time in milliseconds
    pub fn max_input_time_ms(&self) -> f64 {
        self.input_times
            .iter()
            .max()
            .map(|d| d.as_secs_f64() * 1000.0)
            .unwrap_or(0.0)
    }

    /// Check if performance meets requirements (30+ FPS, <16ms input)
    #[allow(dead_code)]
    pub fn meets_requirements(&self) -> bool {
        let fps_ok = self.current_fps() >= 30.0;
        let input_ok = self.avg_input_time_ms() < 16.0;
        fps_ok && input_ok
    }

    /// Get a performance summary string
    pub fn summary(&self) -> String {
        format!(
            "FPS: {:.1} (avg frame: {:.2}ms, max: {:.2}ms) | Input: avg {:.2}ms, max {:.2}ms | Frames: {} | Inputs: {}",
            self.current_fps(),
            self.avg_frame_time_ms(),
            self.max_frame_time_ms(),
            self.avg_input_time_ms(),
            self.max_input_time_ms(),
            self.total_frames,
            self.total_inputs
        )
    }

    /// Check if any performance warnings should be issued
    pub fn check_warnings(&self) -> Vec<String> {
        let mut warnings = Vec::new();

        if self.current_fps() < 30.0 {
            warnings.push(format!(
                "FPS below target: {:.1} FPS (target: 30+ FPS)",
                self.current_fps()
            ));
        }

        if self.avg_input_time_ms() >= 16.0 {
            warnings.push(format!(
                "Input processing slow: {:.2}ms (target: <16ms)",
                self.avg_input_time_ms()
            ));
        }

        if self.max_frame_time_ms() > 50.0 {
            warnings.push(format!(
                "Frame time spike detected: {:.2}ms",
                self.max_frame_time_ms()
            ));
        }

        if self.max_input_time_ms() > 50.0 {
            warnings.push(format!(
                "Input processing spike detected: {:.2}ms",
                self.max_input_time_ms()
            ));
        }

        warnings
    }
}

impl Default for PerformanceMetrics {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_fps_calculation() {
        let mut metrics = PerformanceMetrics::new();

        // Simulate 30 FPS (33.33ms per frame)
        for _ in 0..60 {
            metrics.start_frame();
            thread::sleep(Duration::from_millis(33));
            metrics.end_frame();
        }

        let fps = metrics.current_fps();
        assert!(fps >= 28.0 && fps <= 32.0, "FPS should be around 30, got {}", fps);
    }

    #[test]
    fn test_input_timing() {
        let mut metrics = PerformanceMetrics::new();

        // Record some input times
        metrics.record_input_time(Duration::from_millis(10));
        metrics.record_input_time(Duration::from_millis(12));
        metrics.record_input_time(Duration::from_millis(8));

        let avg = metrics.avg_input_time_ms();
        assert!(avg >= 9.0 && avg <= 11.0, "Average should be around 10ms, got {}", avg);
    }

    #[test]
    fn test_requirements_check() {
        let mut metrics = PerformanceMetrics::new();

        // Simulate good performance
        for _ in 0..60 {
            metrics.frame_times.push_back(Duration::from_millis(30)); // ~33 FPS
        }
        metrics.record_input_time(Duration::from_millis(10));

        assert!(metrics.meets_requirements());
    }

    #[test]
    fn test_warnings() {
        let mut metrics = PerformanceMetrics::new();

        // Simulate poor performance
        for _ in 0..60 {
            metrics.frame_times.push_back(Duration::from_millis(50)); // 20 FPS
        }
        metrics.record_input_time(Duration::from_millis(20));

        let warnings = metrics.check_warnings();
        assert!(!warnings.is_empty());
        assert!(warnings.iter().any(|w| w.contains("FPS below target")));
        assert!(warnings.iter().any(|w| w.contains("Input processing slow")));
    }
}
