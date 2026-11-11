//! Page 3:ANALYSIS: decay, Gaussian and Poisson fits of saved or live runs.
#![expect(
    unused_results,
    clippy::shadow_reuse,
    clippy::non_ascii_literal,
    reason = "egui widgets return a Response that layout-only calls ignore, and egui closures rebind `ui` to the child Ui; labels use physics symbols (plus-minus, sigma, lambda, chi-squared, degree)"
)]

use core::time::Duration;

use curie_core::{BinWidthMode, DatasetType, ExponentialDecayFit, F64Math};
use eframe::egui::{Align, Button, DragValue, Layout, RichText, ScrollArea, Stroke, Ui, Vec2};
use egui_plot::{Bar, BarChart, Legend, Line, MarkerShape, Plot, PlotUi, Points};

use crate::app::{BackgroundChoice, CurieApp, FitModel};
use crate::modals::ExportTarget;
use crate::pages::acquire::GRID_SPACING;
use crate::theme::Theme;

pub fn render(app: &mut CurieApp, ui: &mut Ui) {
    app.refresh_analysis(false);
    if app.decay_job.is_some() {
        ui.ctx().request_repaint_after(Duration::from_millis(30));
    }

    let left_panel_width = 300.0;
    let content_h = ui.available_height();
    let right_width =
        (ui.available_width() - left_panel_width - ui.spacing().item_spacing.x).max(380.0);

    ui.with_layout(Layout::left_to_right(Align::Min), |ui| {
        ui.allocate_ui_with_layout(
            Vec2::new(left_panel_width, content_h),
            Layout::top_down(Align::Min),
            |ui| {
                ScrollArea::vertical()
                    .id_salt("analysis_controls_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        Theme::card_frame().show(ui, |ui| {
                            render_dataset_and_params(app, ui);
                        });
                        ui.add_space(8.0);
                        Theme::card_frame().show(ui, |ui| {
                            render_fit_summary(app, ui);
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

fn render_dataset_and_params(app: &mut CurieApp, ui: &mut Ui) {
    ui.heading(
        RichText::new("DATASET & PARAMETERS")
            .monospace()
            .size(13.5)
            .strong()
            .color(Theme::TEXT_MAIN),
    );
    ui.add_space(6.0);
    render_dataset_selector(app, ui);
    ui.add_space(8.0);
    ui.separator();
    ui.add_space(8.0);
    let w2 = ((ui.available_width() - ui.spacing().item_spacing.x) / 2.0).max(1.0);
    if app.fit_model == FitModel::Exponential {
        render_decay_options(app, ui, w2);
    } else {
        render_histogram_options(app, ui, w2);
    }
}

fn render_dataset_selector(app: &mut CurieApp, ui: &mut Ui) {
    ui.label(Theme::caption("ACTIVE DATASET"));
    ui.add_space(2.0);
    let shows_live = app.analysis_shows_live();
    let live_label = format!(
        "[LIVE] current run ({} gates)",
        app.active_dataset.points.len()
    );
    if ui
        .selectable_label(shows_live, RichText::new(live_label).monospace().size(11.5))
        .clicked()
    {
        app.analysis_dataset_is_live = true;
    }
    let mut chosen = None;
    for (index, dataset) in app.registry.datasets().iter().enumerate() {
        if dataset.kind != DatasetType::AcquireTemporal {
            continue;
        }
        let selected = !shows_live && app.registry.selected_index() == index;
        let label = format!(
            "#{} {} ({} gates)",
            dataset.id,
            dataset.name,
            dataset.points.len()
        );
        if ui
            .selectable_label(selected, RichText::new(label).monospace().size(11.5))
            .clicked()
        {
            chosen = Some(index);
        }
    }
    if let Some(index) = chosen {
        app.analysis_dataset_is_live = false;
        app.registry.select_index(index);
    }
    ui.add_space(4.0);
    let export_btn = Button::new(RichText::new("EXPORT CSV").monospace().size(11.5))
        .min_size(Vec2::new(ui.available_width(), 26.0));
    if ui.add(export_btn).clicked() {
        app.export_modal.is_open = true;
        if shows_live {
            app.export_modal.target = ExportTarget::ActiveDataset;
            "gate_series.csv".clone_into(&mut app.export_modal.file_name);
        } else {
            let index = app.registry.selected_index();
            app.export_modal.target = ExportTarget::RegistryDataset(index);
            app.export_modal.file_name = app.registry.datasets().get(index).map_or_else(
                || "dataset.csv".to_owned(),
                |dataset| format!("{}.csv", dataset.name),
            );
        }
    }
}

fn render_decay_options(app: &mut CurieApp, ui: &mut Ui, w2: f32) {
    ui.heading(Theme::heading("DECAY FIT WINDOW", Theme::ACCENT_AMBER));
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label(Theme::caption("SKIP FIRST:"));
        let skip = DragValue::new(&mut app.decay_skip_points)
            .range(0..=100_000)
            .suffix(" gates");
        ui.add_sized([120.0, 24.0], skip);
    });
    ui.horizontal(|ui| {
        ui.label(Theme::caption("START AT t ≥"));
        let start = DragValue::new(&mut app.decay_start_time_sec)
            .speed(1.0)
            .range(0.0..=1e6)
            .suffix(" s");
        ui.add_sized([120.0, 24.0], start);
    });
    ui.add_space(6.0);
    ui.label(Theme::caption("BACKGROUND B:"));
    ui.horizontal(|ui| {
        for (value, text) in [
            (BackgroundChoice::Fit, "FIT"),
            (BackgroundChoice::Fixed, "FIXED (0 effect)"),
        ] {
            let selected = app.decay_background == value;
            let btn = Theme::segmented_button(text, selected, Vec2::new(w2, 26.0));
            if ui.add(btn).clicked() {
                app.decay_background = value;
            }
        }
    });
    if app.decay_background == BackgroundChoice::Fixed {
        let background = DragValue::new(&mut app.decay_background_cps)
            .speed(0.01)
            .range(0.0..=1e5)
            .suffix(" /s");
        ui.add_sized([ui.available_width(), 24.0], background);
    }
    ui.add_space(4.0);
    ui.label(Theme::note(
        "Fit: Poisson maximum likelihood of A·e^(-λ(t-t0)) + B on raw counts; \
         points at background level are kept, not dropped.",
    ));
}

fn render_histogram_options(app: &mut CurieApp, ui: &mut Ui, w2: f32) {
    ui.heading(Theme::heading("HISTOGRAM BINS", Theme::ACCENT_LILAC));
    ui.add_space(6.0);
    let is_auto = app.bin_width_mode == BinWidthMode::Auto;
    ui.horizontal(|ui| {
        for (auto_val, text) in [(true, "AUTO"), (false, "FIXED")] {
            let selected = is_auto == auto_val;
            let btn = Theme::segmented_button(text, selected, Vec2::new(w2, 26.0));
            if ui.add(btn).clicked() {
                if auto_val {
                    app.bin_width_mode = BinWidthMode::Auto;
                } else {
                    app.bin_width_mode = BinWidthMode::Fixed(app.fixed_bin_width.max(1));
                }
            }
        }
    });
    if is_auto {
        let bins = DragValue::new(&mut app.histogram_bins)
            .range(4..=512)
            .prefix("~")
            .suffix(" bins");
        ui.add_sized([ui.available_width(), 24.0], bins);
    } else {
        let width = DragValue::new(&mut app.fixed_bin_width)
            .range(1..=100_000)
            .suffix(" counts/bin");
        if ui.add_sized([ui.available_width(), 24.0], width).changed() {
            app.bin_width_mode = BinWidthMode::Fixed(app.fixed_bin_width.max(1));
        }
    }
    ui.add_space(4.0);
    ui.label(Theme::note(
        "Bins are whole numbers of counts; the χ² merges neighbouring bins until each \
         expects ≥ 5 gates.",
    ));
}

fn render_fit_summary(app: &CurieApp, ui: &mut Ui) {
    let heading = RichText::new("FIT RESULTS").monospace().size(13.5).strong();
    ui.heading(heading.color(Theme::TEXT_MAIN));
    ui.add_space(8.0);

    match app.fit_model {
        FitModel::Exponential => render_exponential_results(app, ui),
        FitModel::Gaussian => render_gaussian_results(app, ui),
        FitModel::Poisson => render_poisson_results(app, ui),
    }
}

fn render_exponential_results(app: &CurieApp, ui: &mut Ui) {
    if let Some(fit) = &app.decay_fit {
        render_decay_convergence_status(app, fit.converged, ui);
        render_decay_fit_metrics(fit, ui);
    } else {
        render_no_fit_placeholder(
            ui,
            "Awaiting decaying series…",
            &[
                ("HALF-LIFE T½", "—"),
                ("DECAY CONSTANT λ", "—"),
                ("INITIAL RATE R(t0)", "—"),
                ("BACKGROUND B", "—"),
            ],
            "Select a temporal dataset with decaying counts, or adjust the start time t ≥ and skip gates above.",
        );
    }
}

fn render_decay_convergence_status(app: &CurieApp, converged: bool, ui: &mut Ui) {
    let (text, color) = if app.decay_job.is_some() {
        ("UPDATING FIT...", Theme::ACCENT_AMBER)
    } else if converged {
        ("CONVERGED (POISSON ML)", Theme::STATE_RUNNING)
    } else {
        ("WARNING: DID NOT FULLY CONVERGE", Theme::ACCENT_AMBER)
    };
    ui.label(RichText::new(text).monospace().size(11.0).color(color));
    ui.add_space(4.0);
}

fn render_decay_fit_metrics(fit: &ExponentialDecayFit, ui: &mut Ui) {
    ui.label(Theme::caption("HALF-LIFE T½"));
    ui.add_space(2.0);
    let t_half = format!("{:.2} ± {:.2} s", fit.half_life_s, fit.half_life_error);
    ui.label(
        RichText::new(t_half)
            .monospace()
            .size(17.0)
            .strong()
            .color(Theme::ACCENT_AMBER),
    );
    ui.add_space(8.0);

    ui.label(Theme::caption("DECAY CONSTANT λ"));
    ui.add_space(2.0);
    let lambda = format!("{:.5} ± {:.5} s⁻¹", fit.lambda, fit.lambda_error);
    ui.label(
        RichText::new(lambda)
            .monospace()
            .size(13.5)
            .strong()
            .color(Theme::TEXT_MAIN),
    );
    ui.add_space(8.0);

    ui.label(Theme::caption("INITIAL RATE R(t0)"));
    ui.add_space(2.0);
    let r0 = format!(
        "{:.3} ± {:.3} s⁻¹",
        fit.rate_at_start, fit.rate_at_start_error
    );
    let t0_note = format!("at t0 = {:.1} s", fit.start_time_s);
    ui.label(
        RichText::new(r0)
            .monospace()
            .size(12.5)
            .strong()
            .color(Theme::ACCENT_LILAC),
    );
    ui.label(
        RichText::new(t0_note)
            .monospace()
            .size(10.5)
            .color(Theme::TEXT_MUTED),
    );
    ui.add_space(8.0);

    ui.label(Theme::caption("BACKGROUND B"));
    ui.add_space(2.0);
    let bg_text = if fit.background_fitted {
        format!(
            "{:.4} ± {:.4} s⁻¹ (fitted)",
            fit.background_cps, fit.background_error
        )
    } else {
        format!("{:.4} s⁻¹ (fixed)", fit.background_cps)
    };
    ui.label(
        RichText::new(bg_text)
            .monospace()
            .size(12.0)
            .color(Theme::TEXT_MAIN),
    );
    ui.add_space(8.0);

    ui.separator();
    ui.add_space(6.0);

    ui.label(Theme::caption("GOODNESS OF FIT"));
    ui.add_space(2.0);
    let reduced = fit
        .reduced_chi_squared
        .map_or_else(|| "—".to_owned(), |v| format!("{v:.3}"));
    let chi_text = format!(
        "χ²/ndf = {:.1}/{} = {reduced}",
        fit.chi_squared, fit.degrees_of_freedom
    );
    ui.label(
        RichText::new(chi_text)
            .monospace()
            .size(11.5)
            .color(Theme::TEXT_MUTED),
    );
    let pts_text = format!("gates used: {}", fit.points_used);
    ui.label(
        RichText::new(pts_text)
            .monospace()
            .size(11.0)
            .color(Theme::TEXT_MUTED),
    );
}

fn render_gaussian_results(app: &CurieApp, ui: &mut Ui) {
    let cache = app.analysis_cache.as_ref();
    let fit = cache.and_then(|c| c.gaussian.as_ref());
    if let Some(fit) = fit {
        ui.label(Theme::caption("MEAN μ"));
        ui.add_space(2.0);
        let mean = format!("{:.3} ± {:.3} counts", fit.mean, fit.mean_error);
        ui.label(
            RichText::new(mean)
                .monospace()
                .size(17.0)
                .strong()
                .color(Theme::ACCENT_LILAC),
        );
        ui.add_space(8.0);

        ui.label(Theme::caption("STANDARD DEVIATION σ"));
        ui.add_space(2.0);
        let sigma = format!("{:.3} ± {:.3}", fit.sigma, fit.sigma_error);
        ui.label(
            RichText::new(sigma)
                .monospace()
                .size(13.5)
                .strong()
                .color(Theme::TEXT_MAIN),
        );
        ui.add_space(8.0);

        ui.label(Theme::caption("FULL WIDTH AT HALF MAXIMUM (FWHM)"));
        ui.add_space(2.0);
        let fwhm = format!("{:.3} ± {:.3}", fit.fwhm, fit.fwhm_error);
        ui.label(
            RichText::new(fwhm)
                .monospace()
                .size(12.5)
                .strong()
                .color(Theme::ACCENT_AMBER),
        );
        ui.add_space(4.0);
        let poisson_sigma = format!("√μ = {:.3} (expected Poisson σ)", F64Math::sqrt(fit.mean));
        ui.label(
            RichText::new(poisson_sigma)
                .monospace()
                .size(11.0)
                .color(Theme::TEXT_MUTED),
        );
        ui.add_space(8.0);

        ui.separator();
        ui.add_space(6.0);

        ui.label(Theme::caption("GOODNESS OF FIT"));
        ui.add_space(2.0);
        let goodness_str = fit.goodness.map_or_else(
            || "χ²: not enough gates".to_owned(),
            |g| {
                let red = g
                    .reduced_chi_squared
                    .map_or_else(|| "—".to_owned(), |v| format!("{v:.3}"));
                format!(
                    "χ²/ndf = {:.2}/{} = {red} ({} groups)",
                    g.chi_squared, g.degrees_of_freedom, g.merged_groups
                )
            },
        );
        ui.label(
            RichText::new(goodness_str)
                .monospace()
                .size(11.5)
                .color(Theme::TEXT_MUTED),
        );
    } else {
        render_no_fit_placeholder(
            ui,
            "Need at least two gates with different counts",
            &[
                ("MEAN μ", "—"),
                ("STANDARD DEVIATION σ", "—"),
                ("FWHM", "—"),
                ("POISSON σ (√μ)", "—"),
            ],
            "Select a dataset with multiple counts per gate to fit a Gaussian normal distribution.",
        );
    }
}

fn render_poisson_results(app: &CurieApp, ui: &mut Ui) {
    let cache = app.analysis_cache.as_ref();
    let fit = cache.and_then(|c| c.poisson.as_ref());
    if let Some(fit) = fit {
        ui.label(Theme::caption("INTENSITY PARAMETER λ"));
        ui.add_space(2.0);
        let lambda = format!("{:.4} ± {:.4} counts/gate", fit.lambda, fit.lambda_error);
        ui.label(
            RichText::new(lambda)
                .monospace()
                .size(17.0)
                .strong()
                .color(Theme::ACCENT_LILAC),
        );
        ui.add_space(8.0);

        ui.label(Theme::caption("SAMPLE VARIANCE"));
        ui.add_space(2.0);
        let var = format!("{:.4}", fit.sample_variance);
        ui.label(
            RichText::new(var)
                .monospace()
                .size(13.5)
                .strong()
                .color(Theme::TEXT_MAIN),
        );
        ui.add_space(8.0);

        ui.label(Theme::caption("FANO FACTOR (Var / Mean)"));
        ui.add_space(2.0);
        let fano = fit
            .fano_factor
            .map_or_else(|| "—".to_owned(), |v| format!("{v:.4}"));
        ui.label(
            RichText::new(fano)
                .monospace()
                .size(12.5)
                .strong()
                .color(Theme::ACCENT_AMBER),
        );
        ui.add_space(8.0);

        ui.separator();
        ui.add_space(6.0);

        ui.label(Theme::caption("GOODNESS OF FIT"));
        ui.add_space(2.0);
        let goodness_str = fit.goodness.map_or_else(
            || "χ²: not enough gates".to_owned(),
            |g| {
                let red = g
                    .reduced_chi_squared
                    .map_or_else(|| "—".to_owned(), |v| format!("{v:.3}"));
                format!(
                    "χ²/ndf = {:.2}/{} = {red} ({} groups)",
                    g.chi_squared, g.degrees_of_freedom, g.merged_groups
                )
            },
        );
        ui.label(
            RichText::new(goodness_str)
                .monospace()
                .size(11.5)
                .color(Theme::TEXT_MUTED),
        );
    } else {
        render_no_fit_placeholder(
            ui,
            "No gates recorded",
            &[
                ("INTENSITY PARAMETER λ", "—"),
                ("SAMPLE VARIANCE", "—"),
                ("FANO FACTOR", "—"),
            ],
            "Select a dataset with recorded gates to compute the Poisson distribution and χ² test.",
        );
    }
}

fn render_no_fit_placeholder(ui: &mut Ui, status: &str, metrics: &[(&str, &str)], help_note: &str) {
    ui.label(Theme::caption("STATUS"));
    ui.add_space(2.0);
    ui.label(
        RichText::new(status)
            .monospace()
            .size(11.5)
            .color(Theme::TEXT_MUTED),
    );
    ui.add_space(8.0);

    for (label, val) in metrics {
        ui.label(Theme::caption(*label));
        ui.add_space(2.0);
        ui.label(
            RichText::new(*val)
                .monospace()
                .size(13.0)
                .color(Theme::TEXT_MUTED),
        );
        ui.add_space(6.0);
    }

    ui.separator();
    ui.add_space(6.0);
    ui.label(Theme::note(help_note));
}

fn render_plot_card(app: &mut CurieApp, ui: &mut Ui) {
    ui.horizontal(|ui| {
        for (model, label) in [
            (FitModel::Exponential, "1: EXP (T1/2)"),
            (FitModel::Gaussian, "2: GAUSS"),
            (FitModel::Poisson, "3: POISSON"),
        ] {
            let button =
                Theme::segmented_button(label, app.fit_model == model, Vec2::new(0.0, 26.0));
            if ui.add(button).clicked() {
                app.fit_model = model;
            }
        }
        if app.fit_model == FitModel::Exponential {
            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);
            for (log, text) in [(false, "LINEAR"), (true, "LOG (net)")] {
                let selected = app.decay_log_scale == log;
                let btn = Theme::segmented_button(text, selected, Vec2::new(0.0, 26.0));
                if ui.add(btn).clicked() {
                    app.decay_log_scale = log;
                }
            }
        }
    });
    ui.add_space(6.0);
    ui.separator();
    ui.add_space(6.0);

    let (x_label, y_label) = match app.fit_model {
        FitModel::Exponential if app.decay_log_scale => ("t (s)", "ln(rate − B)"),
        FitModel::Exponential => ("t (s)", "rate (s⁻¹)"),
        FitModel::Gaussian | FitModel::Poisson => ("counts per gate", "number of gates"),
    };
    Plot::new("analysis_plot")
        .height(ui.available_height().max(200.0))
        .legend(Legend::default())
        .grid_spacing(GRID_SPACING)
        .x_axis_label(x_label)
        .y_axis_label(y_label)
        .show(ui, |plot_ui| {
            if app.fit_model == FitModel::Exponential {
                plot_decay(app, plot_ui);
            } else {
                plot_distribution(app, plot_ui);
            }
        });
}

/// Rates inside and outside the fit window, and the fitted curve.
fn plot_decay(app: &CurieApp, plot_ui: &mut PlotUi<'_>) {
    let dataset = if app.analysis_shows_live() {
        &app.active_dataset
    } else {
        app.registry.selected().unwrap_or(&app.active_dataset)
    };
    let background = app.decay_fit.as_ref().map_or(0.0, |fit| fit.background_cps);
    let transform = |rate: f64| {
        if app.decay_log_scale {
            (rate - background > 0.0).then(|| (rate - background).ln())
        } else {
            Some(rate)
        }
    };
    let mut used = Vec::new();
    let mut skipped = Vec::new();
    for (index, point) in dataset.points.iter().enumerate() {
        let t = f64::from(point.x);
        if let Some(y) = transform(f64::from(point.y)) {
            if index >= app.decay_skip_points && t >= app.decay_start_time_sec {
                used.push([t, y]);
            } else {
                skipped.push([t, y]);
            }
        }
    }
    plot_ui.points(
        Points::new("fitted gates", used)
            .shape(MarkerShape::Circle)
            .radius(3.0)
            .color(Theme::ACCENT_PERIWINKLE),
    );
    plot_ui.points(
        Points::new("outside window", skipped)
            .shape(MarkerShape::Cross)
            .radius(3.0)
            .color(Theme::TEXT_MUTED),
    );
    if let Some(fit) = &app.decay_fit {
        let end = dataset
            .points
            .last()
            .map_or(fit.start_time_s, |point| f64::from(point.x));
        let curve: Vec<[f64; 2]> = (0..=300)
            .filter_map(|step| {
                let t = (end - fit.start_time_s).mul_add(f64::from(step) / 300.0, fit.start_time_s);
                let rate = f64::mul_add(
                    fit.rate_at_start,
                    F64Math::exp(-fit.lambda * (t - fit.start_time_s)),
                    fit.background_cps,
                );
                transform(rate).map(|y| [t, y])
            })
            .collect();
        plot_ui.line(
            Line::new("fit A·e^(-λt)+B", curve)
                .color(Theme::ACCENT_AMBER)
                .width(2.2),
        );
    }
}

/// Histogram of counts per gate with the expected Gaussian or Poisson frequencies.
fn plot_distribution(app: &CurieApp, plot_ui: &mut PlotUi<'_>) {
    let cache = app.analysis_cache.as_ref();
    let Some(histogram) = cache.and_then(|cached| cached.histogram.as_ref()) else {
        return;
    };
    let width = f64::from(histogram.width);
    let bins: Vec<_> = (0..histogram.frequencies.len())
        .filter_map(|index| histogram.bin(index))
        .collect();
    let bars: Vec<Bar> = bins
        .iter()
        .map(|bin| {
            Bar::new(bin.center, f64::from(bin.frequency))
                .width(width * 0.9)
                .fill(Theme::TAB_ACTIVE_BG)
                .stroke(Stroke::new(1.0, Theme::ACCENT_LILAC))
        })
        .collect();
    plot_ui.bar_chart(BarChart::new("observed", bars).color(Theme::ACCENT_LILAC));
    if app.fit_model == FitModel::Gaussian {
        if let Some(fit) = cache.and_then(|cached| cached.gaussian.as_ref()) {
            let curve: Vec<[f64; 2]> = bins
                .iter()
                .map(|bin| {
                    let low_z = (f64::from(bin.low) - 0.5 - fit.mean) / fit.sigma;
                    let high_z = (f64::from(bin.high) + 0.5 - fit.mean) / fit.sigma;
                    let prob = (F64Math::normal_cdf(high_z) - F64Math::normal_cdf(low_z)).max(0.0);
                    [bin.center, fit.samples * prob]
                })
                .collect();
            plot_ui.points(
                Points::new("Gaussian", curve.clone())
                    .shape(MarkerShape::Diamond)
                    .radius(4.0)
                    .color(Theme::ACCENT_AMBER),
            );
            plot_ui.line(
                Line::new("Gaussian", curve)
                    .color(Theme::ACCENT_AMBER)
                    .width(1.5),
            );
        }
    } else if let Some(fit) = cache.and_then(|cached| cached.poisson.as_ref()) {
        let curve: Vec<[f64; 2]> = bins
            .iter()
            .map(|bin| {
                let mut prob = 0.0;
                for k in bin.low..=bin.high {
                    prob += F64Math::poisson_pmf(k, fit.lambda);
                }
                [bin.center, fit.samples * prob]
            })
            .collect();
        plot_ui.points(
            Points::new("Poisson", curve.clone())
                .shape(MarkerShape::Diamond)
                .radius(4.0)
                .color(Theme::ACCENT_AMBER),
        );
        plot_ui.line(
            Line::new("Poisson", curve)
                .color(Theme::ACCENT_AMBER)
                .width(1.5),
        );
    }
}
