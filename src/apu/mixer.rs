pub fn mix<const N: usize>(samples: [f32; N]) -> f32 {
    samples.into_iter().sum::<f32>().clamp(-1.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamps_mixed_samples_to_output_range() {
        assert_eq!(mix([0.75, 0.75]), 1.0);
        assert_eq!(mix([-0.75, -0.75]), -1.0);
    }
}
