//! # Example
//! ```rust,norun
//! use msr::{Controller,bang_bang::*};
//!
//! let mut cfg = BangBangConfig::default();
//! cfg.threshold = 5.8;
//! cfg.hysteresis = 0.1;
//! let mut c = BangBang::new(cfg);
//!
//! assert_eq!(c.next(5.89), false); // 5.89 < threshold + hysteresis
//! assert_eq!(c.next(5.9),  true);
//! assert_eq!(c.next(5.89), true);  // 5.89 > threshold - hysteresis
//! assert_eq!(c.next(5.71), true);
//! assert_eq!(c.next(5.69),  false);
//! ```

use super::Controller;

/// A Bang-bang controller implementation
#[derive(Debug)]
pub struct BangBang {
    current: bool,
    cfg: BangBangConfig,
}

/// Bang-bang controller configuration
#[derive(Debug, Clone)]
pub struct BangBangConfig {
    pub threshold: f64,
    pub hysteresis: f64,
}

impl Default for BangBangConfig {
    fn default() -> Self {
        BangBangConfig {
            threshold: 0.0,
            hysteresis: 0.0,
        }
    }
}

impl BangBang {
    /// Create a new controller instance with the given configuration.
    pub fn new(cfg: BangBangConfig) -> Self {
        BangBang {
            cfg,
            current: false,
        }
    }
}

impl Controller<f64, bool> for BangBang {
    fn next(&mut self, actual: f64) -> bool {
        if actual > self.cfg.threshold + self.cfg.hysteresis {
            self.current = true;
        } else if actual < self.cfg.threshold - self.cfg.hysteresis {
            self.current = false;
        }
        self.current
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn default_bang_bang_config() {
        let cfg = BangBangConfig::default();
        assert_eq!(cfg.threshold, 0.0);
        assert_eq!(cfg.hysteresis, 0.0);
    }

    #[test]
    fn calculate_with_default_cfg() {
        let mut bb = BangBang::new(BangBangConfig::default());
        assert_eq!(bb.next(0.1), true);
        assert_eq!(bb.next(0.0), true);
        assert_eq!(bb.next(-0.1), false);
        assert_eq!(bb.next(0.0), false);
    }

    #[test]
    fn calculate_with_custom_threshold() {
        let mut cfg = BangBangConfig::default();
        cfg.threshold = 3.3;
        let mut bb = BangBang::new(cfg);
        assert_eq!(bb.next(1.0), false);
        assert_eq!(bb.next(3.3), false);
        assert_eq!(bb.next(3.4), true);
        assert_eq!(bb.next(3.3), true);
        assert_eq!(bb.next(3.2), false);
    }

    #[test]
    fn calculate_with_hysteresis() {
        let mut cfg = BangBangConfig::default();
        cfg.hysteresis = 0.5;
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
        use std::f64::*;
        let cfg = BangBangConfig::default();
        let mut bb = BangBang::new(cfg);
        assert_eq!(bb.next(INFINITY), true);
        assert_eq!(bb.next(0.0), true);
        assert_eq!(bb.next(NEG_INFINITY), false);
    }

    #[test]
    fn calculate_with_infinity_threshold() {
        use std::f64::*;
        let mut cfg = BangBangConfig::default();
        cfg.threshold = INFINITY;
        let mut bb = BangBang::new(cfg);
        assert_eq!(bb.next(INFINITY * 2.0), false);

        let mut cfg = BangBangConfig::default();
        cfg.threshold = NEG_INFINITY;
        let mut bb = BangBang::new(cfg);
        assert_eq!(bb.next(NEG_INFINITY * 2.0), false);
    }

    #[test]
    fn ignore_nan_input() {
        use std::f64::*;
        let mut cfg = BangBangConfig::default();
        cfg.hysteresis = 0.5;
        let mut bb = BangBang::new(cfg);
        assert_eq!(bb.next(0.6), true);
        assert_eq!(bb.next(NAN), true);
        assert_eq!(bb.next(-0.49), true);
        assert_eq!(bb.next(-0.6), false);
        assert_eq!(bb.next(NAN), false);
    }

}
