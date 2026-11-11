//! Page 4:CONFIG: data source, serial port, detector, autosave folder and saved runs.
#![expect(
    unused_results,
    clippy::shadow_reuse,
    clippy::non_ascii_literal,
    reason = "egui widgets return a Response that layout-only calls ignore, and egui closures rebind `ui` to the child Ui; labels use physics symbols (tau, micro)"
)]

use alloc::collections::BTreeSet;

use curie_core::{DaqState, DatasetType, DetectorProfile};
use eframe::egui::{
    Align, Button, DragValue, Grid, Layout, RichText, ScrollArea, Stroke, TextEdit, Ui, Vec2,
};

use crate::app::{CurieApp, GateOwner};
use crate::device::DeviceKind;
use crate::modals::ExportTarget;
use crate::serial::{SUPPORTED_BAUD_RATES, SerialProtocol, candidate_ports};
use crate::theme::Theme;

pub fn render(app: &mut CurieApp, ui: &mut Ui) {
    let available_w = ui.available_width();
    let spacing = ui.spacing().item_spacing.x;
    let col_w = ((available_w - spacing) * 0.5).floor();
    let right_w = available_w - col_w - spacing;
    let content_h = ui.available_height();

    ui.with_layout(Layout::left_to_right(Align::Min), |ui| {
        // Container 1 (Left): Hardware & System Profile (Dynamic 50% width)
        ui.allocate_ui_with_layout(
            Vec2::new(col_w, content_h),
            Layout::top_down(Align::Min),
            |ui| {
                Theme::card_frame().show(ui, |ui| {
                    ScrollArea::vertical()
                        .id_salt("settings_hw_scroll")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            render_system_and_hardware(app, ui);
                        });
                    let remaining = ui.available_height();
                    if remaining > 0.0 {
                        ui.allocate_space(Vec2::new(ui.available_width(), remaining));
                    }
                });
            },
        );

        // Container 2 (Right): Dataset Archive & Storage Manager (Dynamic 50% width)
        ui.allocate_ui_with_layout(
            Vec2::new(right_w, content_h),
            Layout::top_down(Align::Min),
            |ui| {
                Theme::card_frame().show(ui, |ui| {
                    render_storage_manager(app, ui);
                    let remaining = ui.available_height();
                    if remaining > 0.0 {
                        ui.allocate_space(Vec2::new(ui.available_width(), remaining));
                    }
                });
            },
        );
    });
}

fn render_system_and_hardware(app: &mut CurieApp, ui: &mut Ui) {
    let busy = app.gate_owner != GateOwner::None
        || app.session.state == DaqState::Acquiring
        || app.sweep.is_measuring();

    let heading = RichText::new("HARDWARE").monospace().size(13.5).strong();
    ui.heading(heading.color(Theme::TEXT_MAIN));
    ui.add_space(8.0);

    ui.label(Theme::subheading("DEVICE", Theme::ACCENT_LILAC));
    ui.add_space(2.0);
    ui.horizontal_wrapped(|ui| {
        ui.add_enabled_ui(!busy, |ui| {
            for kind in DeviceKind::ALL {
                let btn = Theme::segmented_button(
                    kind.label(),
                    app.device_kind == kind,
                    Vec2::new(0.0, 26.0),
                );
                if ui.add(btn).clicked() && app.device_kind != kind {
                    app.select_device(kind);
                }
            }
        });
    });
    let timing = if app.device_kind.times_gates() {
        "Gates are timed by the device."
    } else {
        "The device streams pulses; the PC sorts them into gates."
    };
    ui.label(Theme::note(timing));
    ui.add_space(8.0);

    match app.device_kind {
        DeviceKind::Simulator => render_simulator_controls(app, ui),
        DeviceKind::Serial(protocol) => render_serial(app, ui, protocol),
    }

    ui.add_space(10.0);
    ui.separator();
    ui.add_space(8.0);

    render_detector(app, ui, busy);

    ui.add_space(10.0);
    ui.separator();
    ui.add_space(8.0);

    ui.label(Theme::subheading("AUTOSAVE FOLDER", Theme::ACCENT_LILAC));
    ui.add_space(2.0);
    let editable = app.run_log.is_none() && app.sweep_log.is_none();
    let folder = TextEdit::singleline(&mut app.autosave_dir);
    ui.add_enabled_ui(editable, |ui| {
        ui.add_sized([ui.available_width(), 24.0], folder);
    });
    ui.add_space(2.0);
    ui.label(Theme::note(
        "Every gate is appended and flushed to a CSV here as soon as it is counted.",
    ));
}

fn render_simulator_controls(app: &mut CurieApp, ui: &mut Ui) {
    let simulator = &mut app.simulator;
    ui.horizontal(|ui| {
        ui.label(Theme::caption("BACKGROUND"));
        let background = DragValue::new(&mut simulator.bg_rate_cps)
            .speed(0.05)
            .range(0.0..=1_000.0)
            .suffix(" /s");
        ui.add(background);
        ui.label(Theme::caption("SOURCE"));
        let source = DragValue::new(&mut simulator.base_rate_cps)
            .speed(0.5)
            .range(0.0..=100_000.0)
            .suffix(" /s");
        ui.add(source);
    });
    ui.horizontal(|ui| {
        ui.label(Theme::caption("HALF-LIFE (0 = constant)"));
        let half_life = DragValue::new(&mut simulator.half_life_s)
            .speed(1.0)
            .range(0.0..=1e6)
            .suffix(" s");
        ui.add(half_life);
        if ui
            .button(RichText::new("Pa-234m").monospace().size(10.0))
            .clicked()
        {
            simulator.half_life_s = 70.2;
            simulator.base_rate_cps = 80.0;
        }
    });
}

/// Port, baud rate, CONNECT / DISCONNECT and the link status.
fn render_serial(app: &mut CurieApp, ui: &mut Ui, protocol: SerialProtocol) {
    let connected = app.serial.is_some();
    let config = &mut app.serial_config;
    let mut port_selected = None;
    ui.add_enabled_ui(!connected, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.label(Theme::caption("PORT:"));
            ui.add_sized([120.0, 22.0], TextEdit::singleline(&mut config.port));
            for port in candidate_ports() {
                if ui
                    .small_button(RichText::new(&port).monospace().size(10.0))
                    .clicked()
                {
                    port_selected = Some(port);
                }
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.label(Theme::caption("BAUD:"));
            for baud in SUPPORTED_BAUD_RATES {
                let btn = Theme::segmented_button(
                    baud.to_string(),
                    config.baud == baud,
                    Vec2::new(0.0, 26.0),
                );
                if ui.add(btn).clicked() {
                    config.baud = baud;
                }
            }
            ui.label(Theme::caption("8N1"));
        });
        if protocol == SerialProtocol::PhyweCobra {
            let help = "Cobra: ASCII commands (X 20 handshake, Z gate, z 2 poll). \
                        COM1/COM2 at 9600 or 19200 baud as in the RADIO1 menu.";
            ui.label(
                RichText::new(help)
                    .monospace()
                    .size(9.5)
                    .color(Theme::TEXT_MUTED),
            );
        } else {
            ui.horizontal(|ui| {
                ui.label(Theme::caption("INIT STRING:"));
                let init = TextEdit::singleline(&mut config.init_cmd).hint_text(r"e.g. START\r\n");
                ui.add_sized([200.0, 22.0], init);
            });
        }
    });
    if let Some(port) = port_selected {
        app.serial_config.port = port;
        if app.auto_connect {
            app.connect_serial();
        }
    }
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        if connected {
            let label = RichText::new("DISCONNECT").monospace().strong();
            let btn = Button::new(label.color(Theme::ACCENT_AMBER))
                .stroke(Stroke::new(1.0, Theme::PANEL_BORDER))
                .min_size(Vec2::new(120.0, 26.0));
            if ui.add(btn).clicked() {
                app.disconnect_serial();
            }
        } else {
            let btn = Theme::primary_button("CONNECT", Vec2::new(120.0, 26.0));
            if ui.add(btn).clicked() {
                app.connect_serial();
            }
            ui.add_space(8.0);
            ui.checkbox(&mut app.auto_connect, "AUTO-RETRY");
        }
        if app.device_ready && !app.device_status.is_empty() {
            ui.colored_label(Theme::STATE_RUNNING, &app.device_status);
        }
    });
    if !app.device_ready && !app.device_status.is_empty() {
        ui.add_space(4.0);
        ui.label(
            RichText::new(&app.device_status)
                .monospace()
                .size(10.0)
                .color(Theme::ACCENT_AMBER),
        );
    }
}

/// Detector preset or a custom tube, its dead time, and the tube voltage noted in the CSV.
fn render_detector(app: &mut CurieApp, ui: &mut Ui, busy: bool) {
    ui.label(Theme::subheading("DETECTOR", Theme::ACCENT_LILAC));
    let profile = app.hardware.profile();
    let custom = profile.name == DetectorProfile::CUSTOM_NAME;
    ui.add_enabled_ui(!busy, |ui| {
        ui.horizontal_wrapped(|ui| {
            for preset in DetectorProfile::PRESETS {
                let btn =
                    Theme::segmented_button(preset.name, profile == preset, Vec2::new(0.0, 26.0));
                if ui.add(btn).clicked() {
                    app.hardware.set_profile(preset);
                }
            }
            let custom_btn = Theme::segmented_button("CUSTOM", custom, Vec2::new(0.0, 26.0));
            if ui.add(custom_btn).clicked() && !custom {
                app.hardware
                    .set_profile(DetectorProfile::custom(profile.dead_time_sec));
            }
        });
        ui.horizontal(|ui| {
            ui.label(Theme::caption("DEAD TIME τ"));
            if custom {
                let mut microseconds = f64::from(profile.dead_time_sec) * 1.0e6;
                let field = DragValue::new(&mut microseconds)
                    .range(0.0..=10_000.0)
                    .speed(1.0)
                    .max_decimals(1)
                    .suffix(" µs");
                if ui.add(field).changed() {
                    #[expect(
                        clippy::as_conversions,
                        clippy::cast_possible_truncation,
                        reason = "a dead time of at most 10 ms fits f32"
                    )]
                    let dead_time_s = (microseconds * 1.0e-6) as f32;
                    let bias = app.hardware.bias_voltage_v();
                    app.hardware
                        .set_profile(DetectorProfile::custom(dead_time_s));
                    app.hardware.set_bias_voltage(bias);
                }
            } else {
                let value = format!(
                    "{:.1} µs ({})",
                    profile.dead_time_sec * 1.0e6,
                    profile.dead_time_basis.label()
                );
                ui.label(
                    RichText::new(value)
                        .monospace()
                        .size(11.0)
                        .color(Theme::TEXT_MAIN),
                );
            }
        });
    });
    ui.horizontal(|ui| {
        ui.label(Theme::caption("TUBE VOLTAGE (note only):"));
        let mut volts = app.hardware.bias_voltage_v();
        let field = DragValue::new(&mut volts)
            .range(profile.min_voltage_v..=profile.max_voltage_v)
            .suffix(" V");
        if ui.add(field).changed() {
            app.hardware.set_bias_voltage(volts);
        }
    });
    let note = "The high voltage is set on the counter hardware; this value is only written \
                to the CSV header.";
    ui.label(
        RichText::new(note)
            .monospace()
            .size(9.5)
            .color(Theme::TEXT_MUTED),
    );
}

fn render_storage_manager(app: &mut CurieApp, ui: &mut Ui) {
    let runs = app.registry.datasets().len();
    let heading = RichText::new(format!("SAVED RUNS ({runs})"))
        .monospace()
        .size(13.5)
        .strong();
    ui.heading(heading.color(Theme::TEXT_MAIN));
    ui.add_space(4.0);
    let folder = format!(
        "Kept between sessions in {}/saved_runs. Deleting a run here removes that copy only; \
         the autosave logs stay.",
        app.autosave_dir
    );
    ui.label(
        RichText::new(folder)
            .monospace()
            .size(11.0)
            .color(Theme::TEXT_MUTED),
    );
    ui.add_space(8.0);

    // Drop selections of runs that no longer exist.
    app.storage_selection.retain(|id| {
        app.registry
            .datasets()
            .iter()
            .any(|dataset| dataset.id == *id)
    });
    render_storage_toolbar(app, ui);
    ui.add_space(10.0);
    ui.separator();
    ui.add_space(6.0);
    render_stored_runs(app, ui);
}

/// SELECT ALL / DESELECT ALL and the export and delete actions for the selection (or all).
fn render_storage_toolbar(app: &mut CurieApp, ui: &mut Ui) {
    let selected_count = app.storage_selection.len();
    let has_selection = selected_count > 0;
    let has_runs = !app.registry.datasets().is_empty();
    ui.horizontal_wrapped(|ui| {
        let select_all = Theme::secondary_button("SELECT ALL", Vec2::new(90.0, 26.0));
        if ui
            .add_enabled_ui(has_runs, |ui| ui.add(select_all))
            .inner
            .clicked()
        {
            app.storage_selection = app
                .registry
                .datasets()
                .iter()
                .map(|dataset| dataset.id)
                .collect();
        }
        ui.add_space(4.0);
        let deselect_all = Theme::secondary_button("DESELECT ALL", Vec2::new(100.0, 26.0));
        if ui
            .add_enabled_ui(has_selection, |ui| ui.add(deselect_all))
            .inner
            .clicked()
        {
            app.storage_selection.clear();
        }
        ui.add_space(6.0);
        ui.separator();
        ui.add_space(6.0);

        let (export_label, export_target, can_export, default_name) = if has_selection {
            let label = format!("EXPORT SELECTED ({selected_count})");
            (label, ExportTarget::Selected, true, "curie_selected_runs")
        } else {
            let label = "EXPORT ALL TO CSV".to_owned();
            (label, ExportTarget::MassAll, has_runs, "curie_runs_bulk")
        };
        let (export_text, export_fill) = if can_export {
            (Theme::PANEL_BG, Theme::ACCENT_AMBER)
        } else {
            (Theme::TEXT_MUTED, Theme::TAB_INACTIVE_BG)
        };
        let export_label_text = RichText::new(export_label).monospace().size(11.0).strong();
        let export = Button::new(export_label_text.color(export_text))
            .fill(export_fill)
            .stroke(Stroke::new(1.0, Theme::PANEL_BORDER));
        if ui
            .add_enabled_ui(can_export, |ui| ui.add_sized([160.0, 26.0], export))
            .inner
            .clicked()
        {
            app.export_modal.is_open = true;
            app.export_modal.target = export_target;
            default_name.clone_into(&mut app.export_modal.file_name);
        }
        ui.add_space(4.0);

        let delete_label = if has_selection {
            format!("DELETE SELECTED ({selected_count})")
        } else {
            "DELETE ALL".to_owned()
        };
        let can_delete = has_selection || has_runs;
        let delete = Theme::secondary_button(delete_label, Vec2::new(140.0, 26.0));
        if ui
            .add_enabled_ui(can_delete, |ui| ui.add(delete))
            .inner
            .clicked()
        {
            let ids = if has_selection {
                app.storage_selection.clone()
            } else {
                app.registry
                    .datasets()
                    .iter()
                    .map(|dataset| dataset.id)
                    .collect()
            };
            app.delete_runs(&ids);
        }
    });
}

/// Table of saved runs with a selection box, EXPORT CSV and DELETE per run.
fn render_stored_runs(app: &mut CurieApp, ui: &mut Ui) {
    let mut toggled = None;
    let mut delete_id = None;
    let mut export = None;
    ScrollArea::both()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            Grid::new("storage_runs_grid")
                .striped(true)
                .min_col_width(28.0)
                .spacing([8.0, 6.0])
                .show(ui, |ui| {
                    for (text, color) in [
                        ("SEL", Theme::ACCENT_LILAC),
                        ("ID", Theme::ACCENT_AMBER),
                        ("NAME", Theme::TEXT_MAIN),
                        ("TYPE", Theme::TEXT_MUTED),
                        ("POINTS", Theme::ACCENT_LILAC),
                        ("ACTIONS", Theme::TEXT_MAIN),
                    ] {
                        ui.label(RichText::new(text).monospace().strong().color(color));
                    }
                    ui.end_row();

                    if app.registry.datasets().is_empty() {
                        ui.label(RichText::new(" - ").monospace().color(Theme::TEXT_MUTED));
                        ui.label(RichText::new(" - ").monospace().color(Theme::TEXT_MUTED));
                        ui.label(
                            RichText::new("No saved runs yet")
                                .monospace()
                                .color(Theme::TEXT_MUTED),
                        );
                        ui.label(RichText::new(" - ").monospace().color(Theme::TEXT_MUTED));
                        ui.label(RichText::new("0 pts").monospace().color(Theme::TEXT_MUTED));
                        ui.label(RichText::new(" - ").monospace().color(Theme::TEXT_MUTED));
                        ui.end_row();
                    } else {
                        for (index, dataset) in app.registry.datasets().iter().enumerate() {
                            let is_selected = app.storage_selection.contains(&dataset.id);
                            let mut check = is_selected;
                            if ui.checkbox(&mut check, "").changed() {
                                toggled = Some((dataset.id, check));
                            }
                            let id = RichText::new(format!("#{}", dataset.id)).monospace();
                            ui.label(id.color(Theme::TEXT_MUTED));
                            let name_color = if is_selected {
                                Theme::ACCENT_LILAC
                            } else {
                                Theme::TEXT_MAIN
                            };
                            ui.label(
                                RichText::new(&dataset.name)
                                    .monospace()
                                    .strong()
                                    .color(name_color),
                            );
                            let kind = match dataset.kind {
                                DatasetType::AcquireTemporal => "GATE SERIES",
                                DatasetType::SweepMatrix => "SWEEP",
                                _ => "OTHER",
                            };
                            ui.label(RichText::new(kind).monospace().size(10.5).color(Theme::TEXT_MUTED));
                            let points = dataset.points.len().to_string();
                            ui.label(RichText::new(points).monospace().color(Theme::ACCENT_LILAC));
                            ui.horizontal(|ui| {
                                let export_label = RichText::new("EXPORT CSV").monospace().size(10.5);
                                if ui.button(export_label).clicked() {
                                    export = Some((index, format!("{}.csv", dataset.name)));
                                }
                                let delete_label = RichText::new("DELETE").monospace().size(10.5);
                                if ui.button(delete_label.color(Theme::ACCENT_AMBER)).clicked() {
                                    delete_id = Some(dataset.id);
                                }
                            });
                            ui.end_row();
                        }
                    }
                });

            if app.registry.datasets().is_empty() {
                ui.add_space(14.0);
                ui.label(Theme::note(
                    "Runs saved via 'SAVE RUN' in Acquire or 'SAVE DATASET' in Sweep are archived here. \
                     Saved runs persist across sessions and can be re-analyzed in Tab 3 (ANALYSIS).",
                ));
            }
        });

    if let Some((id, checked)) = toggled {
        let _changed = if checked {
            app.storage_selection.insert(id)
        } else {
            app.storage_selection.remove(&id)
        };
    }
    if let Some(id) = delete_id {
        app.delete_runs(&BTreeSet::from([id]));
    }
    if let Some((index, file_name)) = export {
        app.export_modal.is_open = true;
        app.export_modal.target = ExportTarget::RegistryDataset(index);
        app.export_modal.file_name = file_name;
    }
}
