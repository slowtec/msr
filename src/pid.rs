//! # Example
//!
//! ```rust,no_run
//! use msr::{TimeStepController, pid::*};
//! use std::{thread, time::Duration};
//!
//! // 1. Create the PID configuration
//! let mut cfg = PidConfig::default();
//! cfg.k_p = 2.5; // define the proportional coefficient
//! cfg.k_i = 0.7; // define the integral coefficient
//! cfg.k_d = 5.0; // define the derative coefficient
//!
//! // 2. Create the PID controller
//! let mut pid = Pid::new(cfg);
//!
//! // 3. Set the desired target
//! pid.set_target(33.7);
//!
//! // 4. Define the duration between two steps
//! let delta_t = Duration::from_millis(1000);
//!
//! // 5. Calculate the values
//! loop {
//!
//!     // 1. Fetch the current sensor value.
//!     // Here you'd use s.th. like a `read_sensor` method.
//!     let sensor_value = 11.0;
//!
//!     // 2. Calculate the next step
//!     let actuator_value = pid.next(sensor_value, &delta_t);
//!
//!     // 3. Set the actuator
//!     // Here you'd use s.th. like a `write_actuator` method.
//!
//!     // 4. Wait some time
//!     thread::sleep(delta_t);
//! }
//! ```

use super::Controller;
use std::time::Duration;

/// PID controller implementation
#[derive(Debug, Clone)]
pub struct Pid {
    cfg: PidConfig,
    /// Current PID state
    pub state: PidState,
}

/// Internal PID controller state
#[derive(Debug, Clone, PartialEq)]
pub struct PidState {
    /// Current target
    pub target: f64,
    /// Value of the previous step
    pub prev_value: Option<f64>,
    /// Proportional portion
    pub p: f64,
    /// Integral portion (error sum)
    pub i: f64,
    /// Derative portion
    pub d: f64,
}

impl Default for PidState {
    fn default() -> Self {
        PidState {
            target: 0.0,
            prev_value: None,
            p: 0.0,
            i: 0.0,
            d: 0.0,
        }
    }
}

impl Pid {
    /// Create a new PID controller instance.
    pub fn new(cfg: PidConfig) -> Self {
        let mut state = PidState::default();
        state.target = cfg.default_target.clone();
        Pid { state, cfg }
    }
    /// Set target value.
    pub fn set_target(&mut self, target: f64) {
        self.state.target = target;
    }
    /// Reset the internal controller state.
    pub fn reset(&mut self) {
        self.state = PidState::default();
        self.state.target = self.cfg.default_target;
    }
}

/// PID Configuration
#[derive(Debug, Clone)]
pub struct PidConfig {
    /// Proportional coefficient
    pub k_p: f64,
    /// Integral coefficient
    pub k_i: f64, // Ki = Kp / Ti
    /// Derivativ coefficient
    pub k_d: f64, // Kd = Kp * Td
    /// The default setpoint
    pub default_target: f64,
    /// Minimum output value
    pub min: Option<f64>,
    /// Maximum output value
    pub max: Option<f64>,
    /// Minimum integral portion
    pub i_min: Option<f64>,
    /// Maximum integral portion
    pub i_max: Option<f64>,
}

impl Default for PidConfig {
    fn default() -> Self {
        PidConfig {
            k_p: 1.0,
            k_i: 0.0,
            k_d: 0.0,
            default_target: 0.0,
            min: None,
            max: None,
            i_min: None,
            i_max: None,
        }
    }
}

impl<'a> Controller<(f64, &'a Duration), f64> for Pid {
    fn next(&mut self, input: (f64, &Duration)) -> f64 {
        let (actual, duration) = input;
        let delta_t = duration_as_f64(duration);

        let err = self.state.target - actual;

        self.state.p = self.cfg.k_p * err;
        self.state.i = self.state.i + self.cfg.k_i * err * delta_t;
        self.state.d = if delta_t == 0.0 {
            0.0
        } else {
            self.cfg.k_d * (self.state.prev_value.unwrap_or_else(|| actual) - actual) / delta_t
        };

        self.state.prev_value = Some(actual);

        let result = self.state.p + self.state.i + self.state.d;

        let result = limit(self.cfg.min, self.cfg.max, result);

        result
    }
}

// Number of nanoseconds in a second.
const NANOS_PER_SEC: f64 = 1.0e9;

fn duration_as_f64(d: &Duration) -> f64 {
    d.as_secs() as f64 + (d.subsec_nanos() as f64 / NANOS_PER_SEC)
}

fn limit(min: Option<f64>, max: Option<f64>, mut value: f64) -> f64 {
    if let Some(max) = max {
        if value > max {
            value = max;
        }
    }
    if let Some(min) = min {
        if value < min {
            value = min;
        }
    }
    value
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn default_pid_config() {
        let cfg = PidConfig::default();
        assert_eq!(cfg.k_p, 1.0);
        assert_eq!(cfg.k_i, 0.0);
        assert_eq!(cfg.k_d, 0.0);
        assert_eq!(cfg.default_target, 0.0);
        assert_eq!(cfg.min, None);
        assert_eq!(cfg.max, None);
        assert_eq!(cfg.i_min, None);
        assert_eq!(cfg.i_max, None);
    }

    #[test]
    fn set_target() {
        let mut pid = Pid::new(PidConfig::default());
        assert_eq!(pid.state.target, 0.0);
        pid.set_target(3.3);
        assert_eq!(pid.state.target, 3.3);
    }

    #[test]
    fn calculate_with_default_cfg() {
        let dt = Duration::from_secs(1);
        let mut pid = Pid::new(PidConfig::default());
        assert_eq!(pid.next((0.0, &dt)), 0.0);
        assert_eq!(pid.next((1.0, &dt)), -1.0);
        assert_eq!(pid.state.p, -1.0);
        assert_eq!(pid.state.i, 0.0);
        assert_eq!(pid.state.d, 0.0);
        assert_eq!(pid.next((-3.0, &dt)), 3.0);
        assert_eq!(pid.state.p, 3.0);
        assert_eq!(pid.state.i, 0.0);
        assert_eq!(pid.state.d, 0.0);
        pid.state.target = 1.0;
        pid.cfg.k_p = 2.0;
        assert_eq!(pid.next((0.0, &dt)), 2.0);
        assert_eq!(pid.next((0.5, &dt)), 1.0);
        assert_eq!(pid.next((1.0, &dt)), 0.0);
        pid.state.target = 2.0;
        assert_eq!(pid.next((0.0, &dt)), 4.0);
    }

    #[test]
    fn calculate_i() {
        let mut cfg = PidConfig::default();
        cfg.k_p = 0.0;
        cfg.k_i = 2.0;
        let mut pid = Pid::new(cfg);
        let dt = Duration::from_secs(1);
        assert_eq!(pid.next((0.0, &dt)), 0.0);
        pid.set_target(1.0);
        assert_eq!(pid.next((0.0, &dt)), 2.0);
        assert_eq!(pid.state.i, 2.0);
        assert_eq!(pid.next((0.0, &dt)), 4.0);
        assert_eq!(pid.state.p, 0.0);
        assert_eq!(pid.state.i, 4.0);
        assert_eq!(pid.state.d, 0.0);
        assert_eq!(pid.next((0.5, &dt)), 5.0);
        assert_eq!(pid.next((1.0, &dt)), 5.0);
        assert_eq!(pid.next((1.5, &dt)), 4.0);
        assert_eq!(pid.next((3.0, &dt)), 0.0);
    }

    #[test]
    fn calculate_d() {
        let mut cfg = PidConfig::default();
        cfg.k_p = 0.0;
        cfg.k_i = 0.0;
        cfg.k_d = 2.0;
        cfg.default_target = 1.0;
        let mut pid = Pid::new(cfg);
        let dt = Duration::from_secs(1);
        assert_eq!(pid.next((0.0, &dt)), 0.0);
        assert_eq!(pid.next((0.0, &dt)), 0.0);
        assert_eq!(pid.next((0.5, &dt)), -1.0);
        assert_eq!(pid.state.p, 0.0);
        assert_eq!(pid.state.i, 0.0);
        assert_eq!(pid.state.d, -1.0);
    }

    #[test]
    fn calculate_d_with_zero_delta_t() {
        let mut cfg = PidConfig::default();
        cfg.k_p = 0.0;
        cfg.k_i = 0.0;
        cfg.k_d = 2.0;
        let mut pid = Pid::new(cfg);
        let dt = Duration::from_secs(0);
        assert_eq!(pid.next((0.0, &dt)), 0.0);
    }

    #[test]
    fn calculate_with_limits() {
        let mut cfg = PidConfig::default();
        cfg.k_p = 2.0;
        cfg.max = Some(4.0);
        cfg.min = Some(-2.0);
        cfg.default_target = 3.0;
        let mut pid = Pid::new(cfg);
        let dt = Duration::from_secs(1);
        assert_eq!(pid.next((0.0, &dt)), 4.0);
        assert_eq!(pid.next((5.0, &dt)), -2.0);
    }

    #[test]
    fn reset() {
        let mut cfg = PidConfig::default();
        cfg.k_p = 7.0;
        cfg.k_i = 5.0;
        cfg.k_d = 2.0;
        cfg.default_target = 9.9;
        let mut pid = Pid::new(cfg);
        pid.set_target(50.0);
        let dt = Duration::from_secs(1);
        pid.next((3.0, &dt));
        assert!(pid.state.i != 0.0);
        assert!(pid.state.target != 0.0);
        assert!(pid.state.prev_value.is_some());
        pid.reset();
        assert_eq!(pid.state.i, 0.0);
        assert_eq!(pid.state.target, 9.9);
        assert_eq!(pid.state.prev_value, None);
    }
}
