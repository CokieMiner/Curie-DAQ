//! Page 1:ACQUIRE: gate series controls, live statistics and plots.
#![expect(
    unused_results,
    clippy::shadow_reuse,
    clippy::non_ascii_literal,
    reason = "egui widgets return a Response that layout-only calls ignore, and egui closures rebind `ui` to the child Ui; labels use physics symbols (plus-minus, sigma, lambda, chi-squared, degree)"
)]

use curie_core::{BinWidthMode, DaqState, StoredPoint};
use eframe::egui::{Align, Button, DragValue, Layout, Rangef, RichText, Stroke, Ui, Vec2};
use egui_plot::{Bar, BarChart, Legend, Line, MarkerShape, Plot, Points};

use crate::app::{AcquireViewMode, CurieApp};
use crate::modals::ExportTarget;
use crate::plot_utils::downsample_rate_series;
use crate::theme::{
    ACCENT_AMBER, ACCENT_LILAC, BG, PANEL_BORDER, STATE_PAUSED, STATE_RUNNING, TAB_ACTIVE_BG,
    TAB_INACTIVE_BG, TEXT_MAIN, TEXT_MUTED, card_frame,
};

const MAX_RENDERED_RATE_POINTS: usize = 4_096;
/// Grid lines closer than 24 px are hidden, so the axes are not labelled at every integer.
pub const GRID_SPACING: Rangef = Rangef {
    min: 24.0,
    max: 300.0,
};
const WINDOW_GUTTER_RIGHT: f32 = 18.0;
const WINDOW_GUTTER_BOTTOM: f32 = 22.0;

pub fn render(app: &mut CurieApp, ui: &mut Ui) {
    app.refresh_analysis(true);
    render_sub_bar(app, ui);
    ui.add_space(6.0);

    let left_panel_width = 270.0;
    let available_w = ui.available_width();
    let content_h = (ui.available_height() - WINDOW_GUTTER_BOTTOM).max(280.0);
    let right_width =
        (available_w - left_panel_width - ui.spacing().item_spacing.x - WINDOW_GUTTER_RIGHT)
            .max(360.0);

    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(
            Vec2::new(left_panel_width, content_h),
            Layout::top_down(Align::Min),
            |ui| {
                card_frame().show(ui, |ui| {
                    render_run_controls(app, ui);
                    render_live_metrics(app, ui);
                });
            },
        );
        ui.allocate_ui_with_layout(
            Vec2::new(right_width, content_h),
            Layout::top_down(Align::Min),
            |ui| {
                card_frame().show(ui, |ui| match app.view_mode {
                    AcquireViewMode::Distribution => render_distribution_plot(app, ui),
                    AcquireViewMode::Temporal => render_temporal_plot(app, ui),
                });
            },
        );
    });
}

/// View toggles and plot / histogram options (fixed height, vertically centred).
fn render_sub_bar(app: &mut CurieApp, ui: &mut Ui) {
    let toggle = |label: &str, size: f32, active: bool| {
        let (text_color, fill, accent) = if active {
            (ACCENT_LILAC, TAB_ACTIVE_BG, ACCENT_LILAC)
        } else {
            (TEXT_MUTED, TAB_INACTIVE_BG, PANEL_BORDER)
        };
        Button::new(
            RichText::new(label)
                .monospace()
                .size(size)
                .color(text_color),
        )
        .fill(fill)
        .stroke(Stroke::new(1.0, accent))
    };
    let caption = |text: &str| RichText::new(text).monospace().size(12.0).color(TEXT_MUTED);
    let row = Layout::left_to_right(Align::Center);
    ui.allocate_ui_with_layout(Vec2::new(ui.available_width(), 30.0), row, |ui| {
        for (label, mode) in [
            ("TEMPORAL (t)", AcquireViewMode::Temporal),
            ("DISTRIBUTION (counts/gate)", AcquireViewMode::Distribution),
        ] {
            if ui.add(toggle(label, 13.0, app.view_mode == mode)).clicked() {
                app.view_mode = mode;
            }
        }
        ui.add_space(6.0);
        ui.separator();
        ui.add_space(6.0);

        if app.view_mode == AcquireViewMode::Temporal {
            for (label, points, log) in [
                ("POINTS", Some(true), None),
                ("LINE", Some(false), None),
                ("LINEAR", None, Some(false)),
                ("LOG (ln)", None, Some(true)),
            ] {
                let active = points.is_some_and(|wanted| wanted == app.temporal_plot_points)
                    || log.is_some_and(|wanted| wanted == app.acquire_log_scale);
                if ui.add(toggle(label, 12.0, active)).clicked() {
                    if let Some(value) = points {
                        app.temporal_plot_points = value;
                    }
                    if let Some(value) = log {
                        app.acquire_log_scale = value;
                    }
                }
            }
        } else {
            ui.label(caption("BIN WIDTH:"));
            let is_auto = app.bin_width_mode == BinWidthMode::Auto;
            if ui
                .selectable_label(is_auto, RichText::new("AUTO").monospace())
                .clicked()
            {
                app.bin_width_mode = BinWidthMode::Auto;
            }
            if ui
                .selectable_label(!is_auto, RichText::new("FIXED").monospace())
                .clicked()
            {
                app.bin_width_mode = BinWidthMode::Fixed(app.fixed_bin_width.max(1));
            }
            if is_auto {
                ui.label(caption("~BINS:"));
                ui.add(DragValue::new(&mut app.histogram_bins).range(4..=512));
            } else {
                let width = DragValue::new(&mut app.fixed_bin_width)
                    .range(1..=100_000)
                    .suffix(" counts");
                if ui.add(width).changed() {
                    app.bin_width_mode = BinWidthMode::Fixed(app.fixed_bin_width.max(1));
                }
            }
        }
    });
}

/// START / PAUSE / RESET / SAVE / EXPORT, the status line and the run parameters.
fn render_run_controls(app: &mut CurieApp, ui: &mut Ui) {
    let is_running = app.session.state == DaqState::Acquiring;
    let is_paused = app.session.state == DaqState::Paused;
    let caption = |text: &str| RichText::new(text).monospace().size(11.5).color(TEXT_MUTED);
    let heading = RichText::new("GATE SERIES").monospace().size(14.5).strong();
    ui.heading(heading.color(TEXT_MAIN));
    ui.add_space(8.0);

    ui.horizontal(|ui| {
        let (label, fill) = if is_paused {
            ("RESUME", STATE_PAUSED)
        } else {
            ("START", STATE_RUNNING)
        };
        let start_label = RichText::new(label).monospace().size(13.5).strong();
        let start = Button::new(start_label.color(BG))
            .fill(fill)
            .stroke(Stroke::new(1.0, PANEL_BORDER));
        if ui.add_enabled(!is_running, start).clicked() {
            app.start_acquisition();
        }
        let pause = Button::new(RichText::new("PAUSE").monospace().size(13.5))
            .fill(TAB_INACTIVE_BG)
            .stroke(Stroke::new(1.0, PANEL_BORDER));
        if ui.add_enabled(is_running, pause).clicked() {
            app.pause_acquisition();
        }
    });
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        if ui
            .button(RichText::new("RESET").monospace().size(12.5))
            .clicked()
        {
            app.reset_acquisition();
        }
        let has_data = !app.active_dataset.points.is_empty();
        let save = Button::new(RichText::new("SAVE RUN").monospace().size(12.0));
        if ui.add_enabled(has_data, save).clicked() {
            app.open_save_modal(false);
        }
        let export = Button::new(RichText::new("EXPORT").monospace().size(12.0));
        if ui.add_enabled(has_data, export).clicked() {
            app.export_modal.is_open = true;
            app.export_modal.target = ExportTarget::ActiveDataset;
            "gate_series.csv".clone_into(&mut app.export_modal.file_name);
        }
    });

    ui.add_space(4.0);
    if let Some(notice) = &app.export_notice {
        ui.label(
            RichText::new(notice)
                .monospace()
                .size(11.0)
                .color(ACCENT_AMBER),
        );
    }
    ui.add_space(8.0);
    ui.separator();
    ui.add_space(8.0);

    // Parameters (fixed while a run is in progress).
    let editable = matches!(app.session.state, DaqState::Idle | DaqState::Completed);
    ui.label(caption("GATE TIME Δt"));
    let gate = DragValue::new(&mut app.sample_interval_s)
        .range(0.1..=3_600.0)
        .speed(0.1)
        .max_decimals(3)
        .suffix(" s");
    ui.add_enabled(editable, gate);
    ui.add_space(4.0);
    ui.label(caption("NUMBER OF GATES (0 = until stopped)"));
    ui.horizontal(|ui| {
        ui.add_enabled(
            editable,
            DragValue::new(&mut app.target_samples).range(0..=10_000_000),
        );
        if app.target_samples > 0 {
            let total = f64::from(app.target_samples) * app.sample_interval_s;
            ui.label(caption(&format!("≈ {total:.0} s")));
        }
    });
    let timing = if app.device_kind.times_gates() {
        "Gates are timed by the device. PAUSE discards the gate in progress."
    } else {
        "Gates are timed by the PC clock. PAUSE discards the gate in progress."
    };
    ui.label(
        RichText::new(timing)
            .monospace()
            .size(10.0)
            .color(TEXT_MUTED),
    );
    ui.add_space(10.0);
    ui.separator();
    ui.add_space(8.0);
}

/// Live statistics of the raw counts (exact for any number of gates).
fn render_live_metrics(app: &CurieApp, ui: &mut Ui) {
    let caption = |text: &str| RichText::new(text).monospace().size(11.0).color(TEXT_MUTED);
    let heading = RichText::new("LIVE METRICS")
        .monospace()
        .size(14.5)
        .strong();
    ui.heading(heading.color(TEXT_MAIN));
    ui.add_space(6.0);
    let last = app.active_dataset.points.last().copied();
    let moments = app.live_moments;
    let gate_s = f64::from(app.active_dataset.gate_ms) / 1000.0;

    ui.label(caption("LAST GATE"));
    let last_text = last.map_or_else(
        || "—".to_owned(),
        |point| format!("{} counts  ({:.2} /s)", point.raw_count, point.y),
    );
    let last_value = RichText::new(last_text).monospace().size(18.0).strong();
    ui.label(last_value.color(ACCENT_LILAC));
    ui.add_space(4.0);

    ui.label(caption("MEAN COUNTS PER GATE"));
    let mean = format!("{:.3} ± {:.3}", moments.mean, moments.std_error_of_mean());
    ui.label(
        RichText::new(mean)
            .monospace()
            .size(15.0)
            .strong()
            .color(TEXT_MAIN),
    );
    if gate_s > 0.0 && moments.count > 0.0 {
        let rate = format!(
            "rate {:.4} ± {:.4} /s",
            moments.mean / gate_s,
            moments.std_error_of_mean() / gate_s
        );
        ui.label(RichText::new(rate).monospace().size(12.0).color(TEXT_MUTED));
    }
    ui.add_space(4.0);
    let fano = moments
        .fano_factor()
        .map_or_else(|| "—".to_owned(), |value| format!("{value:.3}"));
    let spread = format!("VARIANCE {:.3}   FANO {fano}", moments.variance());
    ui.label(
        RichText::new(spread)
            .monospace()
            .size(12.5)
            .color(ACCENT_AMBER),
    );
    let elapsed = last.map_or(0.0, |point| f64::from(point.x) + gate_s / 2.0);
    let gate_count = app.active_dataset.points.len();
    let target = if app.target_samples > 0 {
        format!("/{}", app.target_samples)
    } else {
        String::new()
    };
    let progress = format!("GATES {gate_count}{target}   ELAPSED {elapsed:.1} s");
    ui.label(
        RichText::new(progress)
            .monospace()
            .size(12.0)
            .color(TEXT_MUTED),
    );
    if let Some(log) = &app.run_log {
        ui.add_space(6.0);
        let path = format!("Autosave: {}", log.path.display());
        ui.label(
            RichText::new(path)
                .monospace()
                .size(10.0)
                .color(STATE_RUNNING),
        );
    }
}

fn render_temporal_plot(app: &CurieApp, ui: &mut Ui) {
    let title = if app.acquire_log_scale {
        "ln(RATE) vs t  (gates with 0 counts hidden)"
    } else {
        "RATE vs t  (gate centres)"
    };
    ui.label(
        RichText::new(title)
            .monospace()
            .size(14.5)
            .strong()
            .color(TEXT_MAIN),
    );
    ui.add_space(4.0);

    let points: Vec<StoredPoint> = app
        .active_dataset
        .points
        .iter()
        .filter(|point| !app.acquire_log_scale || point.y > 0.0)
        .map(|point| {
            let y = if app.acquire_log_scale {
                point.y.ln()
            } else {
                point.y
            };
            StoredPoint::new(point.x, y, 0.0, point.raw_count)
        })
        .collect();
    let color = if app.acquire_log_scale {
        ACCENT_AMBER
    } else {
        ACCENT_LILAC
    };

    let mut plot = Plot::new("temporal_rate_plot")
        .height(ui.available_height().max(160.0))
        .grid_spacing(GRID_SPACING)
        .x_axis_label("t (s)")
        .y_axis_label(if app.acquire_log_scale {
            "ln(rate / s⁻¹)"
        } else {
            "rate (s⁻¹)"
        });
    if points.is_empty() {
        plot = plot
            .include_x(0.0)
            .include_x(10.0)
            .include_y(0.0)
            .include_y(10.0);
    }
    plot.show(ui, |plot_ui| {
        if points.is_empty() {
            return;
        }
        let series = downsample_rate_series(&points, MAX_RENDERED_RATE_POINTS);
        if app.temporal_plot_points {
            plot_ui.points(
                Points::new("rate", series)
                    .shape(MarkerShape::Circle)
                    .radius(3.5)
                    .color(color),
            );
        } else {
            plot_ui.line(Line::new("rate", series).color(color).width(1.8));
        }
    });
}

fn render_distribution_plot(app: &CurieApp, ui: &mut Ui) {
    ui.label(
        RichText::new("COUNTS PER GATE — HISTOGRAM")
            .monospace()
            .size(14.5)
            .strong()
            .color(TEXT_MAIN),
    );
    ui.add_space(4.0);
    let cache = app.live_cache.as_ref();
    let histogram = cache.and_then(|cached| cached.histogram.as_ref());
    let poisson = cache.and_then(|cached| cached.poisson.as_ref());

    let mut plot = Plot::new("distribution_plot")
        .height(ui.available_height().max(160.0))
        .grid_spacing(GRID_SPACING)
        .legend(Legend::default())
        .x_axis_label("counts per gate")
        .y_axis_label("number of gates");
    if histogram.is_none() {
        plot = plot
            .include_x(0.0)
            .include_x(10.0)
            .include_y(0.0)
            .include_y(10.0);
    }
    plot.show(ui, |plot_ui| {
        let Some(histogram) = histogram else {
            return;
        };
        let width = f64::from(histogram.width);
        let bars: Vec<Bar> = (0..histogram.frequencies.len())
            .filter_map(|index| histogram.bin(index))
            .map(|bin| {
                Bar::new(bin.center, f64::from(bin.frequency))
                    .width(width * 0.9)
                    .fill(TAB_ACTIVE_BG)
                    .stroke(Stroke::new(1.0, ACCENT_LILAC))
            })
            .collect();
        plot_ui.bar_chart(BarChart::new("observed", bars).color(ACCENT_LILAC));
        if let Some(fit) = poisson {
            let name = fit.goodness.map_or_else(
                || "Poisson (λ = mean)".to_owned(),
                |goodness| {
                    format!(
                        "Poisson (λ = mean), χ²/ndf = {:.1}/{}",
                        goodness.chi_squared, goodness.degrees_of_freedom
                    )
                },
            );
            let expected: Vec<[f64; 2]> = (0..histogram.frequencies.len())
                .filter_map(|index| histogram.bin(index))
                .zip(&fit.expected_per_bin)
                .map(|(bin, &expected)| [bin.center, expected])
                .collect();
            plot_ui.points(
                Points::new(name.as_str(), expected.clone())
                    .shape(MarkerShape::Diamond)
                    .radius(4.0)
                    .color(ACCENT_AMBER),
            );
            plot_ui.line(
                Line::new(name.as_str(), expected)
                    .color(ACCENT_AMBER)
                    .width(1.2),
            );
        }
    });
}
