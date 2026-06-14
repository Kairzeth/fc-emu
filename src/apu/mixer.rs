pub fn mix(pulse_1: f32, pulse_2: f32, triangle: f32, noise: f32, dmc: f32) -> f32 {
    let pulse_sum = pulse_1 + pulse_2;
    let pulse_out = if pulse_sum == 0.0 {
        0.0
    } else {
        95.88 / ((8128.0 / pulse_sum) + 100.0)
    };

    let tnd_sum = triangle / 8227.0 + noise / 12_241.0 + dmc / 22_638.0;
    let tnd_out = if tnd_sum == 0.0 {
        0.0
    } else {
        159.79 / ((1.0 / tnd_sum) + 100.0)
    };

    (pulse_out + tnd_out).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamps_mixed_samples_to_output_range() {
        assert!((-1.0..=1.0).contains(&mix(15.0, 15.0, 15.0, 15.0, 127.0)));
    }
}
