//! Page 1:ACQUIRE: gate series controls, live statistics and plots.
#![expect(
    unused_results,
    clippy::shadow_reuse,
    clippy::non_ascii_literal,
    reason = "egui widgets return a Response that layout-only calls ignore, and egui closures rebind `ui` to the child Ui; labels use physics symbols (plus-minus, sigma, lambda, chi-squared, degree)"
)]

use curie_core::{BinWidthMode, DaqState, F64Math, SampleMoments, StoredPoint};
use eframe::egui::{
    Align, Button, DragValue, Layout, Rangef, RichText, ScrollArea, Stroke, Ui, Vec2,
};
use egui_plot::{Bar, BarChart, Legend, Line, MarkerShape, Plot, Points};

use crate::app::{AcquireViewMode, CurieApp};
use crate::modals::ExportTarget;
use crate::plot_utils::downsample_rate_series;
use crate::theme::Theme;

const MAX_RENDERED_RATE_POINTS: usize = 4_096;
/// Grid lines closer than 24 px are hidden, so the axes are not labelled at every integer.
pub const GRID_SPACING: Rangef = Rangef {
    min: 24.0,
    max: 300.0,
};

pub fn render(app: &mut CurieApp, ui: &mut Ui) {
    app.refresh_analysis(true);

    let left_panel_width = 300.0;
    let available_w = ui.available_width();
    let content_h = ui.available_height();
    let right_width = (available_w - left_panel_width - ui.spacing().item_spacing.x).max(360.0);

    ui.with_layout(Layout::left_to_right(Align::Min), |ui| {
        ui.allocate_ui_with_layout(
            Vec2::new(left_panel_width, content_h),
            Layout::top_down(Align::Min),
            |ui| {
                ScrollArea::vertical()
                    .id_salt("acquire_controls_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        Theme::card_frame().show(ui, |ui| {
                            render_run_controls(app, ui);
                        });
                        ui.add_space(8.0);
                        Theme::card_frame().show(ui, |ui| {
                            render_live_metrics(app, ui);
                        });
                    });
            },
        );
        ui.allocate_ui_with_layout(
            Vec2::new(right_width, content_h),
            Layout::top_down(Align::Min),
            |ui| {
                Theme::card_frame().show(ui, |ui| {
                    render_plot_card(app, ui);
                    let remaining = ui.available_height();
                    if remaining > 0.0 {
                        ui.allocate_space(Vec2::new(ui.available_width(), remaining));
                    }
                });
            },
        );
    });
}

/// Plot card containing the mode and view toolbar in its header, followed by the active plot.
fn render_plot_card(app: &mut CurieApp, ui: &mut Ui) {
    render_plot_toolbar(app, ui);
    ui.add_space(6.0);
    ui.separator();
    ui.add_space(6.0);
    match app.view_mode {
        AcquireViewMode::Distribution => render_distribution_plot(app, ui),
        AcquireViewMode::Temporal => render_temporal_plot(app, ui),
    }
}

/// View toggles and plot / histogram options embedded in the plot card header.
fn render_plot_toolbar(app: &mut CurieApp, ui: &mut Ui) {
    ui.horizontal(|ui| {
        for (label, mode) in [
            ("TEMPORAL (t)", AcquireViewMode::Temporal),
            ("DISTRIBUTION (counts/gate)", AcquireViewMode::Distribution),
        ] {
            let button = Theme::segmented_button_sized(
                label,
                app.view_mode == mode,
                Vec2::new(0.0, 26.0),
                12.0,
                true,
            );
            if ui.add(button).clicked() {
                app.view_mode = mode;
            }
        }
        ui.add_space(6.0);
        ui.separator();
        ui.add_space(6.0);

        if app.view_mode == AcquireViewMode::Temporal {
            render_temporal_toolbar_options(app, ui);
        } else {
            render_distribution_toolbar_options(app, ui);
        }
    });
}

fn render_temporal_toolbar_options(app: &mut CurieApp, ui: &mut Ui) {
    for (label, points, log) in [
        ("POINTS", Some(true), None),
        ("LINE", Some(false), None),
        ("LINEAR", None, Some(false)),
        ("LOG (ln)", None, Some(true)),
    ] {
        let active = points.is_some_and(|wanted| wanted == app.temporal_plot_points)
            || log.is_some_and(|wanted| wanted == app.acquire_log_scale);
        let button =
            Theme::segmented_button_sized(label, active, Vec2::new(0.0, 26.0), 11.5, false);
        if ui.add(button).clicked() {
            if let Some(value) = points {
                app.temporal_plot_points = value;
            }
            if let Some(value) = log {
                app.acquire_log_scale = value;
            }
        }
    }
}

fn render_distribution_toolbar_options(app: &mut CurieApp, ui: &mut Ui) {
    ui.label(Theme::caption("BIN WIDTH:"));
    let is_auto = app.bin_width_mode == BinWidthMode::Auto;
    let auto_btn =
        Theme::segmented_button_sized("AUTO", is_auto, Vec2::new(0.0, 26.0), 11.5, false);
    if ui.add(auto_btn).clicked() {
        app.bin_width_mode = BinWidthMode::Auto;
    }
    let fixed_btn =
        Theme::segmented_button_sized("FIXED", !is_auto, Vec2::new(0.0, 26.0), 11.5, false);
    if ui.add(fixed_btn).clicked() {
        app.bin_width_mode = BinWidthMode::Fixed(app.fixed_bin_width.max(1));
    }
    if is_auto {
        ui.label(Theme::caption("~BINS:"));
        ui.add_sized(
            [60.0, 26.0],
            DragValue::new(&mut app.histogram_bins).range(4..=512),
        );
    } else {
        let width = DragValue::new(&mut app.fixed_bin_width)
            .range(1..=100_000)
            .suffix(" counts");
        if ui.add_sized([100.0, 26.0], width).changed() {
            app.bin_width_mode = BinWidthMode::Fixed(app.fixed_bin_width.max(1));
        }
    }
}

/// START / PAUSE / RESET / SAVE / EXPORT, the status line and the run parameters.
fn render_run_controls(app: &mut CurieApp, ui: &mut Ui) {
    let is_running = app.session.state == DaqState::Acquiring;
    let is_paused = app.session.state == DaqState::Paused;
    let heading = RichText::new("GATE SERIES").monospace().size(14.5).strong();
    ui.heading(heading.color(Theme::TEXT_MAIN));
    ui.add_space(8.0);

    let item_spacing = ui.spacing().item_spacing.x;
    let avail_w = ui.available_width();
    let w2 = ((avail_w - item_spacing) / 2.0).max(1.0);
    ui.horizontal(|ui| {
        let start = if is_paused {
            Button::new(
                RichText::new("RESUME")
                    .monospace()
                    .size(13.5)
                    .strong()
                    .color(Theme::BG),
            )
            .fill(Theme::STATE_PAUSED)
            .stroke(Stroke::new(1.0, Theme::PANEL_BORDER))
            .min_size(Vec2::new(w2, 30.0))
        } else {
            Theme::primary_button("START", Vec2::new(w2, 30.0))
        };
        if ui.add_enabled(!is_running, start).clicked() {
            app.start_acquisition();
        }
        let pause = Theme::secondary_button("PAUSE", Vec2::new(w2, 30.0));
        if ui.add_enabled(is_running, pause).clicked() {
            app.pause_acquisition();
        }
    });
    ui.add_space(6.0);
    let w3 = ((avail_w - item_spacing - item_spacing) / 3.0).max(1.0);
    ui.horizontal(|ui| {
        let reset = Theme::secondary_button("RESET", Vec2::new(w3, 26.0));
        if ui.add(reset).clicked() {
            app.reset_acquisition();
        }
        let has_data = !app.active_dataset.points.is_empty();
        let save = Theme::secondary_button("SAVE RUN", Vec2::new(w3, 26.0));
        if ui.add_enabled(has_data, save).clicked() {
            app.open_save_modal(false);
        }
        let export = Theme::secondary_button("EXPORT", Vec2::new(w3, 26.0));
        if ui.add_enabled(has_data, export).clicked() {
            app.export_modal.is_open = true;
            app.export_modal.target = ExportTarget::ActiveDataset;
            "gate_series.csv".clone_into(&mut app.export_modal.file_name);
        }
    });

    if let Some(notice) = &app.export_notice {
        ui.add_space(4.0);
        ui.label(
            RichText::new(notice)
                .monospace()
                .size(11.0)
                .color(Theme::ACCENT_AMBER),
        );
    }
    ui.add_space(8.0);
    ui.separator();
    ui.add_space(8.0);

    // Parameters (fixed while a run is in progress).
    let editable = matches!(app.session.state, DaqState::Idle | DaqState::Completed);
    ui.label(Theme::caption("GATE TIME Δt"));
    let gate = DragValue::new(&mut app.sample_interval_s)
        .range(0.1..=3_600.0)
        .speed(0.1)
        .max_decimals(3)
        .suffix(" s");
    ui.add_enabled_ui(editable, |ui| {
        ui.add_sized([ui.available_width(), 26.0], gate);
    });
    ui.add_space(6.0);
    let target_caption = if app.target_samples > 0 {
        let total = f64::from(app.target_samples) * app.sample_interval_s;
        format!("NUMBER OF GATES (≈ {total:.0} s)")
    } else {
        "NUMBER OF GATES (0 = until stopped)".to_owned()
    };
    ui.label(Theme::caption(&target_caption));
    let target = DragValue::new(&mut app.target_samples).range(0..=10_000_000);
    ui.add_enabled_ui(editable, |ui| {
        ui.add_sized([ui.available_width(), 26.0], target);
    });
    ui.add_space(6.0);
    let timing = if app.device_kind.times_gates() {
        "Gates are timed by the device. PAUSE discards the gate in progress."
    } else {
        "Gates are timed by the PC clock. PAUSE discards the gate in progress."
    };
    ui.label(Theme::note(timing));
}

/// Live statistics of the raw counts (exact for any number of gates).
fn render_live_metrics(app: &CurieApp, ui: &mut Ui) {
    let heading = RichText::new("LIVE METRICS")
        .monospace()
        .size(13.5)
        .strong();
    ui.heading(heading.color(Theme::TEXT_MAIN));
    ui.add_space(8.0);

    let (status_text, status_color) = match app.session.state {
        DaqState::Acquiring => ("● ACQUIRING", Theme::STATE_RUNNING),
        DaqState::Paused => ("❚❚ PAUSED", Theme::STATE_PAUSED),
        DaqState::Completed => ("✔ COMPLETED", Theme::ACCENT_LILAC),
        DaqState::Idle | _ => ("○ IDLE  -  READY", Theme::TEXT_MUTED),
    };
    ui.label(
        RichText::new(status_text)
            .monospace()
            .size(11.0)
            .color(status_color),
    );
    ui.add_space(6.0);

    let last = app.active_dataset.points.last().copied();
    let moments = app.live_moments;
    let gate_s = f64::from(app.active_dataset.gate_ms) / 1000.0;

    render_moment_metrics(moments, last, gate_s, ui);

    let elapsed = last.map_or(0.0, |point| f64::from(point.x) + gate_s / 2.0);
    let gate_count = app.active_dataset.points.len();
    let target = if app.target_samples > 0 {
        format!("/{}", app.target_samples)
    } else {
        String::new()
    };
    let progress = format!("GATES {gate_count}{target}   ELAPSED {elapsed:.1} s");
    ui.label(Theme::caption("PROGRESS & ELAPSED"));
    ui.add_space(2.0);
    ui.label(
        RichText::new(progress)
            .monospace()
            .size(11.5)
            .color(Theme::TEXT_MAIN),
    );

    if let Some(log) = &app.run_log {
        ui.add_space(6.0);
        let filename = log.path.file_name().map_or_else(
            || log.path.display().to_string(),
            |name| name.to_string_lossy().to_string(),
        );
        ui.horizontal(|ui| {
            ui.label(Theme::caption("AUTOSAVE"));
            ui.label(
                RichText::new(&filename)
                    .monospace()
                    .size(11.0)
                    .strong()
                    .color(Theme::STATE_RUNNING),
            )
            .on_hover_text(format!("Writing every gate to:\n{}", log.path.display()));
        });
    }

    ui.add_space(8.0);
    ui.separator();
    ui.add_space(8.0);

    render_recent_gates_section(app, ui);
}

fn render_moment_metrics(
    moments: SampleMoments,
    last: Option<StoredPoint>,
    gate_s: f64,
    ui: &mut Ui,
) {
    ui.label(Theme::caption("LAST GATE"));
    ui.add_space(2.0);
    let (last_text, last_color) = last.map_or_else(
        || (" -  counts  ( -  /s)".to_owned(), Theme::TEXT_MUTED),
        |point| {
            (
                format!("{} counts  ({:.2} /s)", point.raw_count, point.y),
                Theme::ACCENT_LILAC,
            )
        },
    );
    let last_value = RichText::new(last_text).monospace().size(16.5).strong();
    ui.label(last_value.color(last_color));
    ui.add_space(8.0);

    ui.label(Theme::caption("MEAN COUNTS PER GATE"));
    ui.add_space(2.0);
    let mean = format!("{:.3} ± {:.3}", moments.mean, moments.std_error_of_mean());
    ui.label(
        RichText::new(mean)
            .monospace()
            .size(13.5)
            .strong()
            .color(Theme::TEXT_MAIN),
    );
    if gate_s > 0.0 && moments.count > 0.0 {
        ui.add_space(2.0);
        let rate = format!(
            "rate {:.4} ± {:.4} /s",
            moments.mean / gate_s,
            moments.std_error_of_mean() / gate_s
        );
        ui.label(
            RichText::new(rate)
                .monospace()
                .size(11.0)
                .color(Theme::TEXT_MUTED),
        );
    }
    ui.add_space(8.0);

    let fano = moments
        .fano_factor()
        .map_or_else(|| " - ".to_owned(), |value| format!("{value:.3}"));
    let spread = format!("VARIANCE {:.3}   FANO {fano}", moments.variance());
    ui.label(Theme::caption("SPREAD & DISPERSION"));
    ui.add_space(2.0);
    ui.label(
        RichText::new(spread)
            .monospace()
            .size(12.0)
            .color(Theme::ACCENT_AMBER),
    );
    ui.add_space(8.0);
}

fn render_recent_gates_section(app: &CurieApp, ui: &mut Ui) {
    ui.label(Theme::caption("GATE TELEMETRY STREAM"));
    ui.add_space(4.0);
    ui.label(
        RichText::new("GATE       COUNTS       RATE")
            .monospace()
            .size(10.5)
            .color(Theme::TEXT_MUTED),
    );
    ui.add_space(2.0);

    if app.active_dataset.points.is_empty() {
        for placeholder in [
            "#----       -- cts     --.-- /s",
            "#----       -- cts     --.-- /s",
            "#----       -- cts     --.-- /s",
        ] {
            ui.label(
                RichText::new(placeholder)
                    .monospace()
                    .size(11.0)
                    .color(Theme::TEXT_MUTED),
            );
            ui.add_space(2.0);
        }
        ui.add_space(4.0);
        ui.label(Theme::note("Telemetry will stream here upon START."));
    } else {
        ScrollArea::vertical()
            .id_salt("recent_gates_scroll")
            .max_height(140.0)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let count = app.active_dataset.points.len();
                for (i, pt) in app.active_dataset.points.iter().rev().take(50).enumerate() {
                    let gate_num = count.saturating_sub(i);
                    let row = format!(
                        "#{:<4}   {:>6} cts   {:>7.2} /s",
                        gate_num, pt.raw_count, pt.y
                    );
                    ui.label(
                        RichText::new(row)
                            .monospace()
                            .size(11.0)
                            .color(Theme::TEXT_MAIN),
                    );
                    ui.add_space(1.0);
                }
            });
    }
}

fn render_temporal_plot(app: &CurieApp, ui: &mut Ui) {
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
        Theme::ACCENT_AMBER
    } else {
        Theme::ACCENT_LILAC
    };

    let mut plot = Plot::new("temporal_rate_plot")
        .height(ui.available_height().max(160.0))
        .grid_spacing(GRID_SPACING)
        .legend(Legend::default())
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
        let name = if app.acquire_log_scale {
            "ln(rate)"
        } else {
            "rate"
        };
        if app.temporal_plot_points {
            plot_ui.points(
                Points::new(name, series)
                    .shape(MarkerShape::Circle)
                    .radius(3.5)
                    .color(color),
            );
        } else {
            plot_ui.line(Line::new(name, series).color(color).width(1.8));
        }
    });
}

fn render_distribution_plot(app: &CurieApp, ui: &mut Ui) {
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
                    .fill(Theme::TAB_ACTIVE_BG)
                    .stroke(Stroke::new(1.0, Theme::ACCENT_LILAC))
            })
            .collect();
        plot_ui.bar_chart(BarChart::new("observed", bars).color(Theme::ACCENT_LILAC));
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
                .map(|bin| {
                    let mut prob = 0.0;
                    for k in bin.low..=bin.high {
                        prob += F64Math::poisson_pmf(k, fit.lambda);
                    }
                    [bin.center, fit.samples * prob]
                })
                .collect();
            plot_ui.points(
                Points::new(name.as_str(), expected.clone())
                    .shape(MarkerShape::Diamond)
                    .radius(4.0)
                    .color(Theme::ACCENT_AMBER),
            );
            plot_ui.line(
                Line::new(name.as_str(), expected)
                    .color(Theme::ACCENT_AMBER)
                    .width(1.2),
            );
        }
    });
}
