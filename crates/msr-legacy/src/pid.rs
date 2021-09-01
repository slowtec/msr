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

use super::{Controller, PureController};
use crate::util::limit;
use std::{f64, time::Duration};

/// PID controller implementation
#[derive(Debug, Clone)]
pub struct Pid {
    cfg: PidConfig,
    /// Current PID state
    pub state: PidState,
}

/// Internal PID controller state
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PidState {
    /// Current target
    pub target: f64,
    /// Value of the previous step
    pub prev_value: Option<f64>,
    /// Proportional portion
    pub p: f64,
    /// Integral portion (error sum)
    pub i: f64,
    /// Derivative portion
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
        state.target = cfg.default_target;
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
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct PidConfig {
    /// Proportional coefficient
    pub k_p: f64,
    /// Integral coefficient
    pub k_i: f64, // Ki = Kp / Ti
    /// Derivative coefficient
    pub k_d: f64, // Kd = Kp * Td
    /// The default setpoint
    pub default_target: f64,
    /// Minimum output value
    pub min: Option<f64>,
    /// Maximum output value
    pub max: Option<f64>,
    /// Minimum proportional portion
    pub p_min: Option<f64>,
    /// Maximum proportional portion
    pub p_max: Option<f64>,
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
            p_min: None,
            p_max: None,
            i_min: None,
            i_max: None,
        }
    }
}

impl<'a> Controller<(f64, &'a Duration), f64> for Pid {
    fn next(&mut self, input: (f64, &Duration)) -> f64 {
        let (actual, duration) = input;
        let (state, result) = self.cfg.next((self.state, actual, duration));
        self.state = state;
        result
    }
}

impl<'a> PureController<(PidState, f64, &'a Duration), (PidState, f64)> for PidConfig {
    fn next(&self, input: (PidState, f64, &Duration)) -> (PidState, f64) {
        let (state, actual, duration) = input;

        let delta_t = DurationInSeconds::from(*duration);
        debug_assert!(delta_t.is_valid());

        let mut state = state;

        let err_p = state.target - actual;
        state.p = self.k_p * err_p;
        state.p = limit(self.p_min, self.p_max, state.p);

        let err_i = err_p * f64::from(delta_t);
        state.i += self.k_i * err_i;
        state.i = limit(self.i_min, self.i_max, state.i);

        state.d = if delta_t.is_empty() {
            0.0
        } else if let Some(prev_value) = state.prev_value {
            let delta_v = prev_value - actual;
            // Both delta_v and delta_t are correlated somehow. Calculating
            // their ratio before multiplying with k_d should improve the
            // numeric robustness of the algorithm.
            let err_d = delta_v / f64::from(delta_t);
            self.k_d * err_d
        } else {
            0.0
        };

        state.prev_value = Some(actual);

        let result = state.p + state.i + state.d;

        let result = limit(self.min, self.max, result);

        (state, result)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
struct DurationInSeconds(f64);

impl DurationInSeconds {
    // Number of nanoseconds in a second.
    const NANOS_PER_SEC: f64 = 1e9;

    pub fn is_empty(self) -> bool {
        self.0 == 0.0
    }

    pub fn is_valid(self) -> bool {
        self.0 >= 0.0
    }

    pub fn seconds(self) -> f64 {
        self.0
    }
}

impl From<Duration> for DurationInSeconds {
    fn from(from: Duration) -> Self {
        DurationInSeconds(
            from.as_secs() as f64 + f64::from(from.subsec_nanos()) / Self::NANOS_PER_SEC,
        )
    }
}

impl From<DurationInSeconds> for f64 {
    fn from(from: DurationInSeconds) -> Self {
        from.seconds()
    }
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
        assert_eq!(cfg.p_min, None);
        assert_eq!(cfg.p_max, None);
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
    fn calculate_i_with_limits() {
        let mut cfg = PidConfig::default();
        cfg.k_p = 0.0;
        cfg.k_i = 2.0;
        cfg.i_max = Some(1.0);
        let mut pid = Pid::new(cfg);
        let dt = Duration::from_secs(1);
        assert_eq!(pid.next((0.0, &dt)), 0.0);
        pid.set_target(1.0);
        assert_eq!(pid.next((0.0, &dt)), 1.0);
    }

    #[test]
    fn calculate_p_with_limits() {
        let mut cfg = PidConfig::default();
        cfg.k_p = 2.0;
        cfg.p_max = Some(1.7);
        cfg.p_min = Some(-0.5);
        let mut pid = Pid::new(cfg);
        let dt = Duration::from_secs(1);
        assert_eq!(pid.next((0.0, &dt)), 0.0);
        pid.set_target(10.0);
        assert_eq!(pid.next((0.0, &dt)), 1.7);
        assert_eq!(pid.next((40.0, &dt)), -0.5);
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
