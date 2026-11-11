//! Downsampling of long rate series for plotting.

use core::cmp::Ordering;

use curie_core::StoredPoint;
use egui_plot::PlotPoints;

/// At most `max_points` (at least 4) points of a rate series for plotting.
///
/// Long series keep the first and last points and the minimum and maximum of every bucket
/// in between, in time order, so spikes and dips stay visible.
pub fn downsample_rate_series(points: &[StoredPoint], max_points: usize) -> PlotPoints<'static> {
    let Some((first, last)) = points.first().zip(points.last()) else {
        return Vec::<[f64; 2]>::new().into();
    };
    if points.len() <= max_points {
        return points
            .iter()
            .map(|p| [f64::from(p.x), f64::from(p.y)])
            .collect::<Vec<_>>()
            .into();
    }

    let interior = points
        .get(1..points.len().saturating_sub(1))
        .unwrap_or_default();
    let bucket_count = max_points.saturating_sub(2).div_euclid(2);
    let bucket_size = interior.len().div_ceil(bucket_count.max(1));
    let mut sampled = Vec::with_capacity(max_points);
    sampled.push([f64::from(first.x), f64::from(first.y)]);

    for chunk in interior.chunks(bucket_size) {
        let Some(first_point) = chunk.first() else {
            continue;
        };
        let (mut min_index, mut min_point) = (0, first_point);
        let (mut max_index, mut max_point) = (0, first_point);
        for (index, point) in chunk.iter().enumerate().skip(1) {
            if point.y < min_point.y {
                (min_index, min_point) = (index, point);
            }
            if point.y > max_point.y {
                (max_index, max_point) = (index, point);
            }
        }
        // Keep the extremes of each bucket in time order.
        let min_xy = [f64::from(min_point.x), f64::from(min_point.y)];
        let max_xy = [f64::from(max_point.x), f64::from(max_point.y)];
        match min_index.cmp(&max_index) {
            Ordering::Equal => sampled.push(min_xy),
            Ordering::Less => sampled.extend([min_xy, max_xy]),
            Ordering::Greater => sampled.extend([max_xy, min_xy]),
        }
    }

    sampled.push([f64::from(last.x), f64::from(last.y)]);
    sampled.into()
}
