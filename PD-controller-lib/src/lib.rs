use signal_filters::MedianFilter5f64;

pub struct PdController {
    last_time: u32,
    last_dist: i32,
    last_error: f64,
    prev_time: u32,
    errors: MedianFilter5f64,
    error_changes: MedianFilter5f64,
    product_weight: f64,
    derivative_weight: f64,
    last_output: f64,
    initialized: bool,
    has_error: bool,
}

impl PdController {
    pub fn new(product_weight: f64, derivative_weight: f64) -> PdController {
        PdController {
            last_time: 0,
            last_dist: 0,
            last_error: 0.0,
            prev_time: 0,
            errors: MedianFilter5f64::new(),
            error_changes: MedianFilter5f64::new(),
            product_weight: product_weight,
            derivative_weight: derivative_weight,
            last_output: 0.0,
            initialized: false,
            has_error: false,
        }
    }

    pub fn update(&mut self, time: u32, dist: i32, aim_speed: f64) -> f64 {
        if !self.initialized {
            self.last_time = time;
            self.last_dist = dist;
            self.prev_time = time;
            self.initialized = true;
            return self.last_output;
        }

        if time <= self.last_time {
            return self.last_output;
        }

        let time_diff = (time - self.last_time) as f64;
        let dist_diff = (i64::from(dist) - i64::from(self.last_dist)) as f64;

        let speed = dist_diff / time_diff;
        let error = aim_speed - speed;

        // Speed measurements represent interval averages, so their
        // effective measurement times are at the interval midpoints.
        let time_diff2 = (time - self.prev_time) as f64 / 2.0;

        let error_change = if self.has_error {
            (error - self.last_error) / time_diff2
        } else {
            0.0
        };

        // Apply median filters.
        let clean_error = self.errors.update(error);
        let clean_error_change = if self.has_error {
            self.error_changes.update(error_change)
        } else {
            0.0
        };

        // Update history.
        self.last_error = error;
        self.has_error = true;

        self.prev_time = self.last_time;
        self.last_time = time;
        self.last_dist = dist;

        // Calculate PD output.
        self.last_output =
            self.product_weight * clean_error + self.derivative_weight * clean_error_change;

        self.last_output
    }
}
