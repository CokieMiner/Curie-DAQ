//! Downsampling of long rate series.

use curie_core::StoredPoint;

use crate::plot_utils::downsample_rate_series;

#[test]
fn downsampling_preserves_extremes_and_endpoints() {
    let points: Vec<StoredPoint> = (0..1_000_u16)
        .map(|index| {
            let y = match index {
                450 => 9_999.0,
                500 => 0.05,
                _ => 25.0,
            };
            StoredPoint::new(f32::from(index), y, 0.0, 0)
        })
        .collect();

    let plotted = downsample_rate_series(&points, 128);
    let pts = plotted.points();

    assert!(pts.len() <= 128);
    assert_eq!(pts.first().map(|p| p.x), Some(0.0));
    assert_eq!(pts.last().map(|p| p.x), Some(999.0));
    assert!(
        pts.iter()
            .any(|p| (p.x - 450.0).abs() < 1e-3 && (p.y - 9_999.0).abs() < 1e-3),
        "the spike survives downsampling"
    );
}
