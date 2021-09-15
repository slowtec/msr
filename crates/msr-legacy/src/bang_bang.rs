//! # Example
//! ```rust,no_run
//! use msr_legacy::{Controller, bang_bang::*};
//!
//! let cfg = BangBangConfig {
//!     default_threshold: 5.8,
//!     hysteresis: 0.1,
//! };
//! let mut c = BangBang::new(cfg);
//!
//! assert!(!c.next(5.89)); // 5.89 < threshold + hysteresis
//! assert!(c.next(5.9));
//! assert!(c.next(5.89));  // 5.89 > threshold - hysteresis
//! assert!(c.next(5.71));
//! assert!(!c.next(5.69));
//! ```

use super::{Controller, PureController};

/// A Bang-bang controller implementation
#[derive(Debug, Clone)]
pub struct BangBang {
    cfg: BangBangConfig,
    state: BangBangState,
}

/// Bang-bang controller configuration
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct BangBangConfig {
    pub default_threshold: f64,
    pub hysteresis: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BangBangState {
    pub current: bool,
    pub threshold: f64,
}

impl Default for BangBangState {
    fn default() -> Self {
        BangBangState {
            current: false,
            threshold: 0.0,
        }
    }
}

impl Default for BangBangConfig {
    fn default() -> Self {
        BangBangConfig {
            default_threshold: 0.0,
            hysteresis: 0.0,
        }
    }
}

impl BangBang {
    /// Create a new controller instance with the given configuration.
    pub fn new(cfg: BangBangConfig) -> Self {
        let state = BangBangState {
            threshold: cfg.default_threshold,
            ..Default::default()
        };
        BangBang { cfg, state }
    }
}

impl Controller<f64, bool> for BangBang {
    fn next(&mut self, actual: f64) -> bool {
        self.state = self.cfg.next((self.state, actual));
        self.state.current
    }
}

impl PureController<(BangBangState, f64), BangBangState> for BangBangConfig {
    fn next(&self, input: (BangBangState, f64)) -> BangBangState {
        let (mut state, actual) = input;
        if actual > state.threshold + self.hysteresis {
            state.current = true;
        } else if actual < state.threshold - self.hysteresis {
            state.current = false;
        }
        state
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {

    use super::*;

    use std::f64::{INFINITY, NAN, NEG_INFINITY};

    #[test]
    fn default_bang_bang_config() {
        let cfg = BangBangConfig::default();
        assert_eq!(cfg.default_threshold, 0.0);
        assert_eq!(cfg.hysteresis, 0.0);
    }

    #[test]
    fn calculate_with_default_cfg() {
        let mut bb = BangBang::new(BangBangConfig::default());
        assert!(bb.next(0.1));
        assert!(bb.next(0.0));
        assert!(!bb.next(-0.1));
        assert!(!bb.next(0.0));
    }

    #[test]
    fn calculate_with_custom_threshold() {
        let cfg = BangBangConfig {
            default_threshold: 3.3,
            ..Default::default()
        };
        let mut bb = BangBang::new(cfg);
        assert!(!bb.next(1.0));
        assert!(!bb.next(3.3));
        assert!(bb.next(3.4));
        assert!(bb.next(3.3));
        assert!(!bb.next(3.2));
    }

    #[test]
    fn calculate_with_hysteresis() {
        let cfg = BangBangConfig {
            hysteresis: 0.5,
            ..Default::default()
        };
        let mut bb = BangBang::new(cfg);
        let states = vec![
            (0.0, false),
            (0.5, false),
            (0.6, true),
            (0.5, true),
            (0.0, true),
            (-0.5, true),
            (-0.6, false),
            (0.0, false),
            (0.5, false),
            (0.6, true),
        ];

        for (input, output) in states {
            assert_eq!(bb.next(input), output);
        }
    }

    #[test]
    fn calculate_with_infinity_input() {
        let cfg = BangBangConfig::default();
        let mut bb = BangBang::new(cfg);
        assert!(bb.next(INFINITY));
        assert!(bb.next(0.0));
        assert!(!bb.next(NEG_INFINITY));
    }

    #[test]
    fn calculate_with_infinity_threshold() {
        let cfg = BangBangConfig {
            default_threshold: INFINITY,
            ..Default::default()
        };
        let mut bb = BangBang::new(cfg);
        assert!(!bb.next(INFINITY * 2.0));

        let cfg = BangBangConfig {
            default_threshold: NEG_INFINITY,
            ..Default::default()
        };
        let mut bb = BangBang::new(cfg);
        assert!(!bb.next(NEG_INFINITY * 2.0));
    }

    #[test]
    fn ignore_nan_input() {
        let cfg = BangBangConfig {
            hysteresis: 0.5,
            ..Default::default()
        };
        let mut bb = BangBang::new(cfg);
        assert!(bb.next(0.6));
        assert!(bb.next(NAN));
        assert!(bb.next(-0.49));
        assert!(!bb.next(-0.6));
        assert!(!bb.next(NAN));
    }
}
