//! Modal windows for naming a saved run and exporting CSV files.
#![expect(
    unused_results,
    clippy::shadow_reuse,
    reason = "egui widgets return a Response that layout-only calls ignore, and egui closures rebind `ui` to the child Ui"
)]

use eframe::egui::{Align2, Button, Context, Margin, RichText, Stroke, TextEdit, Window};

use crate::theme::{
    ACCENT_AMBER, ACCENT_LILAC, PANEL_BG, PANEL_BORDER, STATE_RUNNING, TAB_INACTIVE_BG, TEXT_MAIN,
    TEXT_MUTED,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportTarget {
    ActiveDataset,
    Sweep,
    RegistryDataset(usize),
    MassAll,
    Selected,
}

#[derive(Debug, Clone)]
pub struct SaveModal {
    pub is_open: bool,
    pub dataset_name: String,
    pub is_sweep: bool,
}

impl Default for SaveModal {
    fn default() -> Self {
        Self {
            is_open: false,
            dataset_name: "live_run".to_owned(),
            is_sweep: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ExportModal {
    pub is_open: bool,
    pub file_name: String,
    pub directory: String,
    pub target: ExportTarget,
}

impl Default for ExportModal {
    fn default() -> Self {
        Self {
            is_open: false,
            file_name: "dataset_export.csv".to_owned(),
            directory: String::new(),
            target: ExportTarget::ActiveDataset,
        }
    }
}

pub fn render_save_modal(
    ctx: &Context,
    modal: &mut SaveModal,
    on_confirm: impl FnOnce(String, bool),
) {
    if !modal.is_open {
        return;
    }

    let mut close = false;
    let mut confirmed = false;

    let title = if modal.is_sweep {
        "SAVE SWEEP"
    } else {
        "SAVE GATE SERIES"
    };
    Window::new(RichText::new(title).monospace().strong().color(TEXT_MAIN))
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.set_min_width(340.0);
            ui.add_space(6.0);

            ui.label(
                RichText::new("NAME:")
                    .monospace()
                    .size(11.0)
                    .strong()
                    .color(ACCENT_LILAC),
            );
            ui.add_sized(
                [ui.available_width(), 24.0],
                TextEdit::singleline(&mut modal.dataset_name)
                    .hint_text("e.g. Cs137_plateau_run_1")
                    .margin(Margin::symmetric(6, 4)),
            );

            ui.add_space(14.0);
            ui.separator();
            ui.add_space(10.0);

            ui.horizontal(|ui| {
                let save_btn =
                    Button::new(RichText::new("SAVE").monospace().strong().color(PANEL_BG))
                        .fill(STATE_RUNNING)
                        .stroke(Stroke::new(1.0, PANEL_BORDER));
                if ui.add_sized([100.0, 28.0], save_btn).clicked() {
                    confirmed = true;
                    close = true;
                }

                ui.add_space(8.0);

                let cancel_btn = Button::new(RichText::new("CANCEL").monospace().color(TEXT_MUTED))
                    .fill(TAB_INACTIVE_BG)
                    .stroke(Stroke::new(1.0, PANEL_BORDER));
                if ui.add_sized([80.0, 28.0], cancel_btn).clicked() {
                    close = true;
                }
            });
            ui.add_space(4.0);
        });

    if confirmed {
        let name = modal.dataset_name.trim().to_owned();
        let is_sweep = modal.is_sweep;
        on_confirm(name, is_sweep);
    }
    if close {
        modal.is_open = false;
    }
}

pub fn render_export_modal(
    ctx: &Context,
    modal: &mut ExportModal,
    on_confirm: impl FnOnce(String, String, ExportTarget),
) {
    if !modal.is_open {
        return;
    }

    let mut close = false;
    let mut confirmed = false;

    let title = match modal.target {
        ExportTarget::MassAll => "MASS EXPORT ALL DATASETS TO CSV",
        ExportTarget::Selected => "EXPORT SELECTED DATASETS TO CSV",
        ExportTarget::ActiveDataset | ExportTarget::Sweep | ExportTarget::RegistryDataset(_) => {
            "EXPORT DATASET TO CSV"
        }
    };

    let label = |text: &str, color| {
        RichText::new(text)
            .monospace()
            .size(11.0)
            .strong()
            .color(color)
    };
    let hint = |text: &str| RichText::new(text).monospace().size(11.5).color(TEXT_MUTED);
    let bulk = matches!(modal.target, ExportTarget::MassAll | ExportTarget::Selected);
    Window::new(RichText::new(title).monospace().strong().color(TEXT_MAIN))
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.set_min_width(380.0);
            ui.add_space(6.0);
            if bulk {
                ui.label(hint("One CSV per dataset is written into this folder:"));
            } else {
                ui.label(label("FILE NAME (.csv):", ACCENT_AMBER));
                let file = TextEdit::singleline(&mut modal.file_name)
                    .hint_text("dataset.csv")
                    .margin(Margin::symmetric(6, 4));
                ui.add_sized([ui.available_width(), 24.0], file);
            }

            ui.add_space(8.0);
            ui.label(label("OUTPUT DIRECTORY PATH:", ACCENT_LILAC));
            let directory =
                TextEdit::singleline(&mut modal.directory).margin(Margin::symmetric(6, 4));
            ui.add_sized([ui.available_width(), 24.0], directory);
            ui.add_space(4.0);
            let note = "The folder is created if it does not exist; relative paths start in \
                        the folder the program was started from.";
            ui.label(RichText::new(note).monospace().size(10.0).color(TEXT_MUTED));

            ui.add_space(14.0);
            ui.separator();
            ui.add_space(10.0);

            ui.horizontal(|ui| {
                let export_label = RichText::new("EXPORT NOW").monospace().strong();
                let export_btn = Button::new(export_label.color(PANEL_BG))
                    .fill(ACCENT_AMBER)
                    .stroke(Stroke::new(1.0, PANEL_BORDER));
                if ui.add_sized([110.0, 28.0], export_btn).clicked() {
                    confirmed = true;
                    close = true;
                }
                ui.add_space(8.0);
                let cancel_label = RichText::new("CANCEL").monospace().color(TEXT_MUTED);
                let cancel_btn = Button::new(cancel_label)
                    .fill(TAB_INACTIVE_BG)
                    .stroke(Stroke::new(1.0, PANEL_BORDER));
                if ui.add_sized([80.0, 28.0], cancel_btn).clicked() {
                    close = true;
                }
            });
            ui.add_space(4.0);
        });

    if confirmed {
        let dir = modal.directory.trim().to_owned();
        let file = modal.file_name.trim().to_owned();
        let target = modal.target;
        on_confirm(dir, file, target);
    }
    if close {
        modal.is_open = false;
    }
}
