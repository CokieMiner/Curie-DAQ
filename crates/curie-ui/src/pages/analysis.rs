//! Page 3:ANALYSIS: decay, Gaussian and Poisson fits of saved or live runs.
#![expect(
    unused_results,
    clippy::shadow_reuse,
    clippy::non_ascii_literal,
    reason = "egui widgets return a Response that layout-only calls ignore, and egui closures rebind `ui` to the child Ui; labels use physics symbols (plus-minus, sigma, lambda, chi-squared, degree)"
)]

use core::time::Duration;

use curie_core::{BinWidthMode, DatasetType, F64Math, GoodnessOfFit};
use eframe::egui::{
    Align, Button, Color32, DragValue, Layout, RichText, ScrollArea, Stroke, Ui, Vec2,
};
use egui_plot::{Bar, BarChart, Legend, Line, MarkerShape, Plot, PlotUi, Points};

use crate::app::{BackgroundChoice, CurieApp, FitModel};
use crate::modals::ExportTarget;
use crate::pages::acquire::GRID_SPACING;
use crate::theme::{
    ACCENT_AMBER, ACCENT_LILAC, ACCENT_PERIWINKLE, PANEL_BORDER, TAB_ACTIVE_BG, TAB_INACTIVE_BG,
    TEXT_MAIN, TEXT_MUTED, card_frame,
};

pub fn render(app: &mut CurieApp, ui: &mut Ui) {
    app.refresh_analysis(false);
    if app.decay_job.is_some() {
        ui.ctx().request_repaint_after(Duration::from_millis(30));
    }

    ui.horizontal(|ui| {
        for (model, label) in [
            (FitModel::Exponential, "1: EXP (T1/2)"),
            (FitModel::Gaussian, "2: GAUSS"),
            (FitModel::Poisson, "3: POISSON"),
        ] {
            let selected = app.fit_model == model;
            let button = Button::new(
                RichText::new(label)
                    .monospace()
                    .strong()
                    .color(if selected { ACCENT_LILAC } else { TEXT_MUTED }),
            )
            .fill(if selected {
                TAB_ACTIVE_BG
            } else {
                TAB_INACTIVE_BG
            })
            .stroke(Stroke::new(
                1.0,
                if selected { ACCENT_LILAC } else { PANEL_BORDER },
            ));
            if ui.add(button).clicked() {
                app.fit_model = model;
            }
        }
    });
    ui.add_space(8.0);

    let left_panel_width = 300.0;
    let content_h = (ui.available_height() - 22.0).max(280.0);
    let right_width =
        (ui.available_width() - left_panel_width - ui.spacing().item_spacing.x - 18.0).max(380.0);

    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(
            Vec2::new(left_panel_width, content_h),
            Layout::top_down(Align::Min),
            |ui| {
                ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        render_dataset_list(app, ui);
                        ui.add_space(8.0);
                        render_options(app, ui);
                        ui.add_space(8.0);
                        render_fit_summary(app, ui);
                    });
            },
        );
        ui.allocate_ui_with_layout(
            Vec2::new(right_width, content_h),
            Layout::top_down(Align::Min),
            |ui| {
                render_plot(app, ui);
            },
        );
    });
}

fn render_dataset_list(app: &mut CurieApp, ui: &mut Ui) {
    card_frame().show(ui, |ui| {
        ui.heading(
            RichText::new("DATASETS")
                .monospace()
                .size(13.0)
                .strong()
                .color(TEXT_MAIN),
        );
        ui.add_space(6.0);
        let shows_live = app.analysis_shows_live();
        let live_label = format!(
            "[LIVE] current run ({} gates)",
            app.active_dataset.points.len()
        );
        if ui
            .selectable_label(shows_live, RichText::new(live_label).monospace())
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
        ui.add_space(6.0);
        if ui
            .button(RichText::new("EXPORT CSV").monospace().size(11.0))
            .clicked()
        {
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
    });
}

fn render_options(app: &mut CurieApp, ui: &mut Ui) {
    let heading = |text: &str, color| {
        RichText::new(text)
            .monospace()
            .size(12.5)
            .strong()
            .color(color)
    };
    let caption = |text: &str| RichText::new(text).monospace().size(10.5).color(TEXT_MUTED);
    let note = |text: &str| RichText::new(text).monospace().size(9.5).color(TEXT_MUTED);
    let choice = |text: &str| RichText::new(text).monospace();
    card_frame().show(ui, |ui| {
        if app.fit_model == FitModel::Exponential {
            ui.heading(heading("DECAY FIT WINDOW", ACCENT_AMBER));
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(caption("SKIP FIRST:"));
                let skip = DragValue::new(&mut app.decay_skip_points)
                    .range(0..=100_000)
                    .suffix(" gates");
                ui.add(skip);
            });
            ui.horizontal(|ui| {
                ui.label(caption("START AT t ≥"));
                let start = DragValue::new(&mut app.decay_start_time_sec)
                    .speed(1.0)
                    .range(0.0..=1e6)
                    .suffix(" s");
                ui.add(start);
            });
            ui.add_space(4.0);
            ui.label(caption("BACKGROUND B:"));
            ui.horizontal(|ui| {
                for (value, text) in [
                    (BackgroundChoice::Fit, "FIT"),
                    (BackgroundChoice::Fixed, "FIXED (zero effect)"),
                ] {
                    let selected = app.decay_background == value;
                    if ui.selectable_label(selected, choice(text)).clicked() {
                        app.decay_background = value;
                    }
                }
            });
            if app.decay_background == BackgroundChoice::Fixed {
                let background = DragValue::new(&mut app.decay_background_cps)
                    .speed(0.01)
                    .range(0.0..=1e5)
                    .suffix(" /s");
                ui.add(background);
            }
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                for (log, text) in [(false, "LINEAR"), (true, "LOG (net)")] {
                    let selected = app.decay_log_scale == log;
                    if ui.selectable_label(selected, choice(text)).clicked() {
                        app.decay_log_scale = log;
                    }
                }
            });
            ui.label(note(
                "Fit: Poisson maximum likelihood of A·e^(-λ(t-t0)) + B on the raw counts; \
                 points at the background level are kept, not dropped.",
            ));
        } else {
            ui.heading(heading("HISTOGRAM BINS", ACCENT_LILAC));
            ui.add_space(4.0);
            let is_auto = app.bin_width_mode == BinWidthMode::Auto;
            ui.horizontal(|ui| {
                if ui.selectable_label(is_auto, choice("AUTO")).clicked() {
                    app.bin_width_mode = BinWidthMode::Auto;
                }
                if ui
                    .selectable_label(!is_auto, choice("FIXED WIDTH"))
                    .clicked()
                {
                    app.bin_width_mode = BinWidthMode::Fixed(app.fixed_bin_width.max(1));
                }
            });
            if is_auto {
                let bins = DragValue::new(&mut app.histogram_bins)
                    .range(4..=512)
                    .prefix("~")
                    .suffix(" bins");
                ui.add(bins);
            } else {
                let width = DragValue::new(&mut app.fixed_bin_width)
                    .range(1..=100_000)
                    .suffix(" counts/bin");
                if ui.add(width).changed() {
                    app.bin_width_mode = BinWidthMode::Fixed(app.fixed_bin_width.max(1));
                }
            }
            ui.label(note(
                "Bins are whole numbers of counts; the χ² merges neighbouring bins until each \
                 expects ≥ 5 gates.",
            ));
        }
    });
}

fn render_fit_summary(app: &CurieApp, ui: &mut Ui) {
    card_frame().show(ui, |ui| {
        let heading = RichText::new("RESULTS").monospace().size(13.0).strong();
        ui.heading(heading.color(TEXT_MAIN));
        ui.add_space(6.0);
        let lines = if app.fit_model == FitModel::Exponential {
            decay_lines(app)
        } else {
            distribution_lines(app)
        };
        for (text, color) in lines {
            ui.label(RichText::new(text).monospace().size(12.0).color(color));
        }
    });
}

/// Result lines of the decay fit (or why there is none).
fn decay_lines(app: &CurieApp) -> Vec<(String, Color32)> {
    let Some(fit) = &app.decay_fit else {
        let reason = if app.decay_job.is_some() {
            "Fitting...".to_owned()
        } else {
            format!(
                "No decay fit ({} gates in the window; needs a decaying series).",
                app.decay_points_used
            )
        };
        return vec![(reason, TEXT_MUTED)];
    };
    let mut lines = Vec::new();
    if app.decay_job.is_some() {
        lines.push(("updating...".to_owned(), ACCENT_AMBER));
    }
    let background = if fit.background_fitted {
        format!(
            "B  = {:.4} ± {:.4} s⁻¹ (fitted)",
            fit.background_cps, fit.background_error
        )
    } else {
        format!("B  = {:.4} s⁻¹ (fixed)", fit.background_cps)
    };
    let reduced = fit
        .reduced_chi_squared
        .map_or_else(|| "—".to_owned(), |value| format!("{value:.3}"));
    lines.extend([
        (
            format!("T½ = {:.2} ± {:.2} s", fit.half_life_s, fit.half_life_error),
            ACCENT_AMBER,
        ),
        (
            format!("λ  = {:.5} ± {:.5} s⁻¹", fit.lambda, fit.lambda_error),
            TEXT_MAIN,
        ),
        (
            format!(
                "R(t0) = {:.3} ± {:.3} s⁻¹  (t0 = {:.1} s)",
                fit.rate_at_start, fit.rate_at_start_error, fit.start_time_s
            ),
            ACCENT_LILAC,
        ),
        (background, TEXT_MAIN),
        (
            format!(
                "χ²/ndf = {:.1}/{} = {reduced}",
                fit.chi_squared, fit.degrees_of_freedom
            ),
            TEXT_MUTED,
        ),
        (format!("gates used: {}", fit.points_used), TEXT_MUTED),
    ]);
    if !fit.converged {
        lines.push((
            "warning: fit did not fully converge".to_owned(),
            ACCENT_AMBER,
        ));
    }
    lines
}

/// Result lines of the Gaussian or Poisson fit of the counts per gate.
fn distribution_lines(app: &CurieApp) -> Vec<(String, Color32)> {
    let cache = app.analysis_cache.as_ref();
    let goodness_text = |goodness: Option<GoodnessOfFit>| {
        goodness.map_or_else(
            || "χ²: not enough gates".to_owned(),
            |fit| {
                let reduced = fit
                    .reduced_chi_squared
                    .map_or_else(|| "—".to_owned(), |value| format!("{value:.3}"));
                format!(
                    "χ²/ndf = {:.2}/{} = {reduced} ({} groups)",
                    fit.chi_squared, fit.degrees_of_freedom, fit.merged_groups
                )
            },
        )
    };
    if app.fit_model == FitModel::Gaussian {
        let Some(fit) = cache.and_then(|cached| cached.gaussian.as_ref()) else {
            return vec![(
                "Need at least two gates with different counts.".to_owned(),
                TEXT_MUTED,
            )];
        };
        return vec![
            (
                format!("μ = {:.3} ± {:.3} counts", fit.mean, fit.mean_error),
                ACCENT_LILAC,
            ),
            (
                format!("σ = {:.3} ± {:.3}", fit.sigma, fit.sigma_error),
                TEXT_MAIN,
            ),
            (
                format!("FWHM = {:.3} ± {:.3}", fit.fwhm, fit.fwhm_error),
                ACCENT_AMBER,
            ),
            (
                format!("√μ = {:.3} (Poisson σ)", F64Math::sqrt(fit.mean)),
                TEXT_MUTED,
            ),
            (goodness_text(fit.goodness), TEXT_MUTED),
        ];
    }
    let Some(fit) = cache.and_then(|cached| cached.poisson.as_ref()) else {
        return vec![("No gates recorded.".to_owned(), TEXT_MUTED)];
    };
    let fano = fit
        .fano_factor
        .map_or_else(|| "—".to_owned(), |value| format!("{value:.4}"));
    vec![
        (
            format!(
                "λ = {:.4} ± {:.4} counts/gate",
                fit.lambda, fit.lambda_error
            ),
            ACCENT_LILAC,
        ),
        (
            format!("sample variance = {:.4}", fit.sample_variance),
            TEXT_MAIN,
        ),
        (format!("Fano factor = {fano}"), ACCENT_AMBER),
        (goodness_text(fit.goodness), TEXT_MUTED),
    ]
}

fn render_plot(app: &CurieApp, ui: &mut Ui) {
    card_frame().show(ui, |ui| {
        let (x_label, y_label) = match app.fit_model {
            FitModel::Exponential if app.decay_log_scale => ("t (s)", "ln(rate − B)"),
            FitModel::Exponential => ("t (s)", "rate (s⁻¹)"),
            FitModel::Gaussian | FitModel::Poisson => ("counts per gate", "number of gates"),
        };
        Plot::new("analysis_plot")
            .height((ui.available_height() - 10.0).max(200.0))
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
            .color(ACCENT_PERIWINKLE),
    );
    plot_ui.points(
        Points::new("outside window", skipped)
            .shape(MarkerShape::Cross)
            .radius(3.0)
            .color(TEXT_MUTED),
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
                .color(ACCENT_AMBER)
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
                .fill(TAB_ACTIVE_BG)
                .stroke(Stroke::new(1.0, ACCENT_LILAC))
        })
        .collect();
    plot_ui.bar_chart(BarChart::new("observed", bars).color(ACCENT_LILAC));
    let expected = if app.fit_model == FitModel::Gaussian {
        cache
            .and_then(|cached| cached.gaussian.as_ref())
            .map(|fit| ("Gaussian", &fit.expected_per_bin))
    } else {
        cache
            .and_then(|cached| cached.poisson.as_ref())
            .map(|fit| ("Poisson", &fit.expected_per_bin))
    };
    if let Some((name, values)) = expected {
        let curve: Vec<[f64; 2]> = bins
            .iter()
            .zip(values)
            .map(|(bin, &value)| [bin.center, value])
            .collect();
        plot_ui.points(
            Points::new(name, curve.clone())
                .shape(MarkerShape::Diamond)
                .radius(4.0)
                .color(ACCENT_AMBER),
        );
        plot_ui.line(Line::new(name, curve).color(ACCENT_AMBER).width(1.5));
    }
}
