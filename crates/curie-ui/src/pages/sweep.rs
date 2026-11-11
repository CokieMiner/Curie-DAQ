//! Page 2:SWEEP: manual distance or angle sweeps with several bench states.
#![expect(
    unused_results,
    clippy::shadow_reuse,
    clippy::non_ascii_literal,
    reason = "egui widgets return a Response that layout-only calls ignore, and egui closures rebind `ui` to the child Ui; labels use physics symbols (plus-minus, sigma, lambda, chi-squared, degree)"
)]

use core::time::Duration;

use curie_core::{F64Math, MAX_SWEEP_STATES, SweepAxis};
use eframe::egui::{
    Align, Align2, Button, DragValue, Grid, Layout, ProgressBar, RichText, ScrollArea, Stroke,
    TextEdit, Ui, Vec2, Window,
};

use crate::app::CurieApp;
use crate::modals::ExportTarget;
use crate::theme::{
    ACCENT_AMBER, ACCENT_LILAC, BG, PANEL_BORDER, STATE_RUNNING, TEXT_MAIN, TEXT_MUTED, card_frame,
};

/// Height of every control row; widgets are centred vertically inside it.
const ROW_HEIGHT: f32 = 30.0;

pub fn render(app: &mut CurieApp, ui: &mut Ui) {
    card_frame().show(ui, |ui| {
        render_controls(app, ui);
        render_bench(app, ui);
    });
    render_state_names_window(app, ui);

    ui.add_space(8.0);
    card_frame().show(ui, |ui| {
        let row = Layout::left_to_right(Align::Center);
        ui.allocate_ui_with_layout(Vec2::new(ui.available_width(), ROW_HEIGHT), row, |ui| {
            let title = RichText::new("POSITION × STATE  (rate ± σ in counts/s)")
                .monospace()
                .size(13.0)
                .strong();
            ui.label(title.color(TEXT_MAIN));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let total = format!("{} GATES", app.sweep.points().len());
                ui.label(
                    RichText::new(total)
                        .monospace()
                        .size(11.0)
                        .color(TEXT_MUTED),
                );
            });
        });
        ui.add_space(6.0);
        render_matrix(app, ui);
    });
}

/// Title, action buttons and the status line.
fn render_controls(app: &mut CurieApp, ui: &mut Ui) {
    let is_measuring = app.sweep.is_measuring();
    let row = Layout::left_to_right(Align::Center);
    ui.allocate_ui_with_layout(Vec2::new(ui.available_width(), ROW_HEIGHT), row, |ui| {
        let title = RichText::new("MANUAL SWEEP")
            .monospace()
            .size(13.5)
            .strong();
        ui.label(title.color(TEXT_MAIN));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let hint = "One gate per (position, state): set the bench, then START STEP";
            ui.label(RichText::new(hint).monospace().size(11.0).color(TEXT_MUTED));
        });
    });
    ui.add_space(4.0);

    ui.allocate_ui_with_layout(Vec2::new(ui.available_width(), ROW_HEIGHT), row, |ui| {
        let start_label = RichText::new("START STEP").monospace().size(12.0).strong();
        let start = Button::new(start_label.color(BG))
            .fill(STATE_RUNNING)
            .stroke(Stroke::new(1.0, PANEL_BORDER));
        if ui.add_enabled(!is_measuring, start).clicked() {
            app.start_sweep_step();
        }
        let has_data = !app.sweep.points().is_empty();
        let button = |text: &str| Button::new(RichText::new(text).monospace());
        if ui
            .add_enabled(is_measuring || has_data, button("REPEAT"))
            .clicked()
        {
            app.repeat_sweep_step();
        }
        if ui
            .add_enabled(is_measuring || has_data, button("UNDO"))
            .clicked()
        {
            app.undo_sweep_point();
        }
        if ui.add_enabled(has_data, button("RESET")).clicked() {
            app.reset_sweep();
        }
        ui.separator();
        if ui.add_enabled(has_data, button("SAVE DATASET")).clicked() {
            app.open_save_modal(true);
        }
        if ui.add_enabled(has_data, button("EXPORT CSV")).clicked() {
            app.export_modal.is_open = true;
            app.export_modal.target = ExportTarget::Sweep;
            "sweep.csv".clone_into(&mut app.export_modal.file_name);
        }
    });
    if let Some(notice) = &app.export_notice {
        ui.label(
            RichText::new(notice)
                .monospace()
                .size(10.5)
                .color(ACCENT_AMBER),
        );
    }
    ui.add_space(6.0);
    ui.separator();
    ui.add_space(6.0);
}

/// Axis, position, step, gate and state selection, plus the progress of the running step.
fn render_bench(app: &mut CurieApp, ui: &mut Ui) {
    let is_measuring = app.sweep.is_measuring();
    let angle_axis = app.sweep.axis() == SweepAxis::Angle;
    let row = Layout::left_to_right(Align::Center);
    let caption = |text: &str| RichText::new(text).monospace().size(10.5).color(TEXT_MUTED);
    ui.add_enabled_ui(!is_measuring, |ui| {
        ui.allocate_ui_with_layout(Vec2::new(ui.available_width(), ROW_HEIGHT), row, |ui| {
            // Axis and gate are fixed once points exist, so a sweep never mixes mm and
            // degrees, and a saved sweep (one gate for all steps) stays exact.
            let axis_free = app.sweep.points().is_empty();
            ui.label(caption("AXIS"));
            ui.add_enabled_ui(axis_free, |ui| {
                let distance = RichText::new("DISTANCE").monospace();
                if ui.selectable_label(!angle_axis, distance).clicked() {
                    app.sweep.set_axis(SweepAxis::Distance);
                }
                if ui
                    .selectable_label(angle_axis, RichText::new("ANGLE").monospace())
                    .clicked()
                {
                    app.sweep.set_axis(SweepAxis::Angle);
                }
            });
            ui.add_space(16.0);
            ui.label(caption("POSITION"));
            if angle_axis {
                let mut angle = app.sweep.target_angle_deg();
                let field = DragValue::new(&mut angle).range(-180..=180).suffix(" °");
                if ui.add(field).changed() {
                    app.sweep.set_target_angle_deg(angle);
                }
            } else {
                let mut distance = app.sweep.target_distance_mm();
                let field = DragValue::new(&mut distance)
                    .range(0..=u16::MAX)
                    .suffix(" mm");
                if ui.add(field).changed() {
                    app.sweep.set_target_distance_mm(distance);
                }
            }
            ui.add_space(16.0);
            ui.label(caption("STEP"));
            let unit = if angle_axis { " °" } else { " mm" };
            let mut step = app.sweep.step_size_mm();
            if ui
                .add(DragValue::new(&mut step).range(1..=1_000).suffix(unit))
                .changed()
            {
                app.sweep.set_step_size_mm(step);
            }
            ui.add_space(16.0);
            ui.label(caption("GATE Δt"));
            let mut gate_s = f64::from(app.sweep.step_duration_ms()) / 1000.0;
            let field = DragValue::new(&mut gate_s)
                .range(0.1..=3_600.0)
                .speed(0.5)
                .suffix(" s");
            if ui.add_enabled(axis_free, field).changed() {
                #[expect(
                    clippy::as_conversions,
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "the field is limited to 0.1 ... 3600 s"
                )]
                let milliseconds = (gate_s * 1000.0).round() as u32;
                app.sweep.set_step_duration_ms(milliseconds);
            }
        });

        render_state_selector(app, ui);
    });
    let advance_hint = "After the last state the position advances by STEP; for uneven spacing \
        (1 mm near the source, ~1 cm far) type each position before START STEP.";
    ui.label(
        RichText::new(advance_hint)
            .monospace()
            .size(9.5)
            .color(TEXT_MUTED),
    );

    ui.add_space(6.0);
    let now_ms = u64::try_from(app.clock.elapsed().as_millis()).unwrap_or(u64::MAX);
    let elapsed = Duration::from_millis(app.sweep.step_elapsed_ms(now_ms)).as_secs_f32();
    let duration = Duration::from_millis(u64::from(app.sweep.step_duration_ms())).as_secs_f32();
    let progress = if is_measuring && duration > 0.0 {
        (elapsed / duration).min(1.0)
    } else {
        0.0
    };
    ui.add(
        ProgressBar::new(progress)
            .show_percentage()
            .animate(is_measuring),
    );
}

/// Number of states, and PREV / NEXT arrows that pick the state measured next (a row of
/// buttons would not fit dozens of states).
fn render_state_selector(app: &mut CurieApp, ui: &mut Ui) {
    let row = Layout::left_to_right(Align::Center);
    let caption = |text: &str| RichText::new(text).monospace().size(10.5).color(TEXT_MUTED);
    ui.allocate_ui_with_layout(Vec2::new(ui.available_width(), ROW_HEIGHT), row, |ui| {
        ui.label(caption("STATES"));
        let mut count = app.sweep.state_count();
        if ui
            .add(DragValue::new(&mut count).range(1..=MAX_SWEEP_STATES))
            .changed()
        {
            app.set_sweep_state_count(count);
        }
        ui.add_space(16.0);
        ui.label(caption("NEXT GATE"));
        let state_count = app.sweep.state_count();
        let current = app.sweep.state_index();
        if ui.button(RichText::new("< PREV").monospace()).clicked() {
            let previous = current
                .checked_sub(1)
                .unwrap_or_else(|| state_count.saturating_sub(1));
            app.sweep.set_state_index(previous);
        }
        let number = current.saturating_add(1);
        let name = app.sweep_labels.get(current).map_or("", String::as_str);
        let text = if name.is_empty() {
            format!("S{number}/{state_count}")
        } else {
            format!("S{number}/{state_count} · {name}")
        };
        ui.label(RichText::new(text).monospace().strong().color(ACCENT_LILAC));
        if ui.button(RichText::new("NEXT >").monospace()).clicked() {
            let next = if number < state_count { number } else { 0 };
            app.sweep.set_state_index(next);
        }
        ui.add_space(10.0);
        if ui
            .button(RichText::new("STATE NAMES…").monospace())
            .clicked()
        {
            app.state_names_open = true;
        }
    });
}

/// Small window where each bench state gets a name (written to the CSV).
fn render_state_names_window(app: &mut CurieApp, ui: &Ui) {
    let mut open = app.state_names_open;
    let mut done = false;
    let title = RichText::new("STATE NAMES").monospace().strong();
    Window::new(title.color(TEXT_MAIN))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ui.ctx(), |ui| {
            let hint = "Name each bench configuration (written to the CSV):";
            ui.label(RichText::new(hint).monospace().size(11.0).color(TEXT_MUTED));
            ui.add_space(6.0);
            Grid::new("state_names_grid")
                .spacing([10.0, 6.0])
                .show(ui, |ui| {
                    for (index, name) in app.sweep_labels.iter_mut().enumerate() {
                        let tag = format!("S{}", index.saturating_add(1));
                        ui.label(RichText::new(tag).monospace().strong().color(ACCENT_LILAC));
                        ui.add_sized([280.0, 24.0], TextEdit::singleline(name));
                        ui.end_row();
                    }
                });
            ui.add_space(8.0);
            if ui
                .button(RichText::new("DONE").monospace().strong())
                .clicked()
            {
                done = true;
            }
        });
    app.state_names_open = open && !done;
}

/// One row per position: the rate of every state, and S1 - S2 and S2 / S1 for two states.
fn render_matrix(app: &CurieApp, ui: &mut Ui) {
    let state_count = app.sweep.state_count();
    let angle_axis = app.sweep.axis() == SweepAxis::Angle;
    let cell = |target: &mut Ui, text: String, color, strong: bool| {
        let rich = RichText::new(text).monospace().color(color);
        target.label(if strong { rich.strong() } else { rich });
    };
    ScrollArea::both()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            Grid::new("sweep_matrix_grid")
                .striped(true)
                .min_col_width(90.0)
                .spacing([14.0, 6.0])
                .show(ui, |ui| {
                    let axis = if angle_axis { "ANGLE" } else { "DISTANCE" };
                    cell(ui, axis.to_owned(), ACCENT_AMBER, true);
                    for index in 0..state_count {
                        let label = app.sweep_labels.get(index).map_or("", String::as_str);
                        let header = format!("S{} {label}", index.saturating_add(1))
                            .trim_end()
                            .to_owned();
                        cell(ui, header, ACCENT_LILAC, true);
                    }
                    if state_count == 2 {
                        cell(ui, "S1 − S2".to_owned(), ACCENT_AMBER, true);
                        cell(ui, "S2 / S1".to_owned(), TEXT_MAIN, true);
                    }
                    ui.end_row();

                    let groups = app.sweep.groups();
                    if groups.is_empty() {
                        cell(ui, "No points yet.".to_owned(), TEXT_MUTED, false);
                        ui.end_row();
                    }
                    for group in groups {
                        let position = if angle_axis {
                            format!("{} °", group.angle_deg)
                        } else {
                            format!("{} mm", group.distance_mm)
                        };
                        cell(ui, position, TEXT_MAIN, true);
                        let rates: Vec<Option<(f64, f64)>> = group
                            .records
                            .iter()
                            .take(state_count)
                            .map(|slot| {
                                slot.map(|record| {
                                    let gate_s = f64::from(record.duration_ms) / 1000.0;
                                    // sqrt(1) for an empty gate: zero counts are not exact.
                                    let sigma = F64Math::sqrt(f64::from(record.pulses.max(1)));
                                    (f64::from(record.pulses) / gate_s, sigma / gate_s)
                                })
                            })
                            .collect();
                        for rate in &rates {
                            let text = rate.map_or_else(
                                || "—".to_owned(),
                                |(value, sigma)| format!("{value:.2} ± {sigma:.2}"),
                            );
                            cell(ui, text, TEXT_MAIN, false);
                        }
                        if let [Some((first, first_sigma)), Some((second, second_sigma))] =
                            rates.as_slice()
                        {
                            let sigma = F64Math::sqrt(
                                first_sigma.mul_add(*first_sigma, second_sigma * second_sigma),
                            );
                            cell(
                                ui,
                                format!("{:+.2} ± {sigma:.2}", first - second),
                                ACCENT_AMBER,
                                false,
                            );
                            let ratio = if *first > 0.0 {
                                // Propagated sigma of S2/S1, finite when S2 = 0.
                                let fraction = second / first;
                                let scaled = fraction * first_sigma;
                                let ratio_sigma = F64Math::sqrt(
                                    second_sigma.mul_add(*second_sigma, scaled * scaled),
                                ) / first;
                                format!("{:.1} ± {:.1} %", fraction * 100.0, ratio_sigma * 100.0)
                            } else {
                                "—".to_owned()
                            };
                            cell(ui, ratio, TEXT_MAIN, false);
                        } else if state_count == 2 {
                            cell(ui, "—".to_owned(), TEXT_MUTED, false);
                            cell(ui, "—".to_owned(), TEXT_MUTED, false);
                        }
                        ui.end_row();
                    }
                });
        });
}
