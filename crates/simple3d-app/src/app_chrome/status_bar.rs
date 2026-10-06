//! The footer: selection, its measurements, and what the application is doing.

use super::*;
use crate::app::{App, Status};
use crate::theme;
use crate::ui;
use simple3d_core::config::RenderEngine;
use simple3d_core::unit::Unit;

impl App {
    pub(crate) fn status_bar(&mut self, ctx: &egui::Context) {
        let frame = egui::Frame::NONE.fill(theme::token::SURFACE_2).inner_margin(egui::Margin {
            left: 8,
            right: 8,
            top: 0,
            bottom: 0,
        });
        egui::TopBottomPanel::bottom("status").frame(frame).exact_height(theme::metric::STATUS_BAR).show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(6.0, 0.0);

                ui.add(egui::Label::new(theme::value(self.selection_summary())).selectable(false));
                dot(ui);
                ui.add(egui::Label::new(theme::numeric(self.selection_size_text())).selectable(false));
                dot(ui);
                // Step and grid are different numbers, both shown, so a jump of 10 can be explained.
                let unit = self.unit();
                let step = simple3d_core::unit::format_length(self.move_snap(), unit);
                let grid = simple3d_core::unit::format_length(self.scene.settings.grid_spacing, unit);
                ui.add(egui::Label::new(theme::numeric(format!("Step {step} {}", unit.suffix()))).selectable(false))
                    .on_hover_text("How far one nudge, and one snapped step of a drag, goes. Set it in Transform.");
                dot(ui);
                ui.add(egui::Label::new(theme::numeric(format!("Grid {grid} {}", unit.suffix()))).selectable(false))
                    .on_hover_text("Ground grid spacing; set it in the Document panel");
                dot(ui);

                // The unit is a click away, since every field reads it.
                let unit = self.unit();
                egui::ComboBox::from_id_salt("status-unit")
                    .selected_text(theme::numeric(unit.suffix()))
                    .width(52.0)
                    .show_ui(ui, |ui| {
                        for option in Unit::ALL {
                            // Switching never rescales the model, only what the fields read (spec section 4).
                            if ui.selectable_label(unit == option, option.suffix()).clicked() {
                                self.scene.settings.unit = option;
                                self.fields.clear();
                            }
                        }
                    });

                dot(ui);

                // Renderer choice as a dropup beside the frame time it affects.
                let engine = self.settings.render_engine;
                let mut chosen = engine;
                egui::ComboBox::from_id_salt("status-engine")
                    .selected_text(theme::value(engine.label()))
                    .width(60.0)
                    .show_ui(ui, |ui| {
                        for option in RenderEngine::ALL {
                            let entry = ui.selectable_label(engine == option, option.label());
                            if entry.on_hover_text(option.description()).clicked() {
                                chosen = option;
                            }
                        }
                    });
                if chosen != engine {
                    self.settings.render_engine = chosen;
                    // Clear the last refusal so a failed driver can be retried.
                    self.gpu_error = None;
                    if chosen == RenderEngine::Cpu {
                        self.gpu = None;
                        self.gpu_texture = None;
                    }
                    // The engine is not part of the viewport cache key, so invalidate it.
                    self.image_key = u64::MAX;
                    self.persist();
                }
                if let Some(why) = &self.gpu_error {
                    if self.settings.render_engine == RenderEngine::Gpu {
                        ui.add(egui::Label::new(theme::value("\u{26a0} CPU")).selectable(false)).on_hover_text(
                            format!(
                            "The GPU renderer is not available, so the viewport is being drawn in software.\n\n{why}"
                        ),
                        );
                    }
                }

                dot(ui);

                if let Some(job) = &self.split_job {
                    // Real progress: cells are counted before cutting starts.
                    ui.add(egui::ProgressBar::new(job.fraction()).desired_width(110.0).show_percentage());
                    ui.add(
                        egui::Label::new(theme::value(format!(
                            "Splitting into {} cells ({}s)",
                            job.cells,
                            job.elapsed().as_secs()
                        )))
                        .selectable(false),
                    );
                    if ui
                        .small_button("Stop")
                        .on_hover_text("Abandon the split. Nothing in the document is changed.")
                        .clicked()
                    {
                        job.cancel();
                    }
                } else if let Some(job) = &self.export_job {
                    let fraction = job.fraction();
                    ui.add(egui::ProgressBar::new(fraction).desired_width(110.0).show_percentage());
                    ui.add(
                        egui::Label::new(theme::value(format!(
                            "Exporting {} ({}s of {}s allowed)",
                            job.format_label,
                            job.elapsed().as_secs(),
                            job.limit().as_secs()
                        )))
                        .selectable(false),
                    );
                    if ui.small_button("Cancel").clicked() {
                        job.cancel();
                    }
                } else if let Some(job) = &self.import_job {
                    // Real progress: every format but OBJ declares its triangle count up front.
                    ui.add(egui::ProgressBar::new(job.fraction()).desired_width(110.0).show_percentage());
                    ui.add(
                        egui::Label::new(theme::value(format!(
                            "Importing {} ({}s of {}s allowed)",
                            job.path.file_name().map(|name| name.to_string_lossy().to_string()).unwrap_or_default(),
                            job.elapsed().as_secs(),
                            job.limit().as_secs()
                        )))
                        .selectable(false),
                    );
                    if ui
                        .small_button("Cancel")
                        .on_hover_text("Stop reading. Nothing is added to the document.")
                        .clicked()
                    {
                        job.cancel();
                    }
                } else if let Some(prompt) = &self.file_prompt {
                    // The desktop's file dialog (a portal on Linux) may be slow or never answer; it waits on its
                    // own thread, and this says what the app is waiting for.
                    ui.add(egui::Spinner::new().size(12.0));
                    ui.add(
                        egui::Label::new(theme::value(format!(
                            "{}: choosing a file\u{2026} ({}s)",
                            prompt.what(),
                            prompt.waiting_for().as_secs()
                        )))
                        .selectable(false),
                    );
                    if ui
                        .small_button("Stop waiting")
                        .on_hover_text("Give up on the file dialog. If it does answer later, the answer is ignored.")
                        .clicked()
                    {
                        self.stop_waiting_for_file();
                    }
                } else if self.worker.is_busy() {
                    // A boolean has no real progress, but elapsed time and Stop are always available.
                    ui.add(egui::Spinner::new().size(12.0));
                    let waited = self.worker.waiting_for().unwrap_or_default();
                    let text = if waited.as_secs() >= 1 {
                        format!("Evaluating\u{2026} ({}s)", waited.as_secs())
                    } else {
                        "Evaluating\u{2026}".to_string()
                    };
                    ui.add(egui::Label::new(theme::value(text)).selectable(false));
                    if ui
                        .small_button("Stop")
                        .on_hover_text(
                            "Abandon this evaluation. The viewport keeps the last shape it managed to \
                             build, so what is on screen will be out of date until the next edit.",
                        )
                        .clicked()
                    {
                        self.worker.abandon();
                        self.status = Status::Warning("Evaluation stopped -- the viewport is out of date".into());
                    }
                } else {
                    let colour = match &self.status {
                        Status::Warning(_) => theme::token::ACCENT,
                        _ => theme::token::TEXT_LO,
                    };
                    // Messages fade once read, so old results do not look current.
                    let opacity = crate::app::status_opacity(&self.status, self.status_at.elapsed());
                    if opacity > 0.0 {
                        // The message (possibly a full path) gets what is left after the right readout and is elided,
                        // rather than running underneath it.
                        let text = self.status_text();
                        let room = (ui.available_width() - theme::metric::STATUS_READOUT).max(0.0);
                        ui.scope(|ui| {
                            ui.set_max_width(room);
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(&text)
                                        .size(theme::font::LABEL)
                                        .color(colour.gamma_multiply(opacity)),
                                )
                                .truncate()
                                .selectable(false),
                            )
                            .on_hover_text(&text);
                        });
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // The root is not a document node; counting it made an empty document report one.
                    let nodes = self.scene.len().saturating_sub(1);
                    ui.add(
                        egui::Label::new(theme::numeric(ui::describe_counts(
                            nodes,
                            self.committed_triangle_count(),
                        )))
                        .selectable(false),
                    )
                    .on_hover_text("Shapes and groups in the document, and the triangles the model came out as");
                    if let Some(elapsed) = self.worker.last_elapsed {
                        dot(ui);
                        ui.add(egui::Label::new(theme::numeric(ui::describe_elapsed(elapsed))).selectable(false))
                            .on_hover_text("How long the last rebuild of the model took");
                    }
                    if !self.evaluated.errors.is_empty() {
                        dot(ui);
                        let names: Vec<&str> = self.evaluated.errors.iter().map(|e| e.name.as_str()).collect();
                        ui.colored_label(theme::token::DANGER, format!("Failed: {}", names.join(", "))).on_hover_text(
                            self.evaluated
                                .errors
                                .iter()
                                .map(|e| format!("{}: {}", e.name, e.message))
                                .collect::<Vec<_>>()
                                .join("\n"),
                        );
                    }
                });
            });
        });
    }
}
