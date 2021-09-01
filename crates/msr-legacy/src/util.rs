//! Little helpers

/// Re-maps a number from one range to another.
pub fn map_value(x: f64, x_min: f64, x_max: f64, y_min: f64, y_max: f64) -> f64 {
    let s = (y_max - y_min) / (x_max - x_min);
    (x - x_min) * s + y_min
}

/// Limit a value by minimum and maximum values.
pub fn limit(min: Option<f64>, max: Option<f64>, mut value: f64) -> f64 {
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

    #[test]
    fn map_value() {
        assert_eq!(super::map_value(0.0, 4.0, 20.0, 0.0, 1.0), -0.25);
        assert_eq!(super::map_value(20.0, 4.0, 20.0, 0.16, 3.2), 3.2);
    }
}
