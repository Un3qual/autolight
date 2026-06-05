#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SceneViewport {
    pub scroll_seconds: f64,
    pub pixels_per_second: f64,
    pub visible_seconds: f64,
    pub duration_seconds: f64,
}

impl SceneViewport {
    pub fn new(
        scroll_seconds: f64,
        pixels_per_second: f64,
        visible_seconds: f64,
        duration_seconds: f64,
    ) -> Self {
        Self {
            scroll_seconds: finite_non_negative(scroll_seconds),
            pixels_per_second: finite_positive(pixels_per_second),
            visible_seconds: finite_positive(visible_seconds),
            duration_seconds: finite_non_negative(duration_seconds),
        }
    }

    pub fn seconds_to_x(self, seconds: f64) -> f64 {
        (seconds - self.scroll_seconds) * self.pixels_per_second
    }

    pub fn x_to_seconds(self, x: f64) -> f64 {
        self.scroll_seconds + x / self.pixels_per_second
    }
}

fn finite_non_negative(value: f64) -> f64 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        0.0
    }
}

fn finite_positive(value: f64) -> f64 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        1.0
    }
}
