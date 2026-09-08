//! The footer: what is selected, what it measures, and what the
//! application is doing.

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

                // Left to right: what is selected, how big it is, what the
                // numbers snap to, and what unit they are in.
                ui.add(egui::Label::new(theme::value(self.selection_summary())).selectable(false));
                dot(ui);
                ui.add(egui::Label::new(theme::numeric(self.selection_size_text())).selectable(false));
                dot(ui);
                // The step and the grid are two different numbers, and both are
                // on the bar: "why did it jump 10" is answered here.
                let unit = self.unit();
                let step = simple3d_core::unit::format_length(self.move_snap(), unit);
                let grid = simple3d_core::unit::format_length(self.scene.settings.grid_spacing, unit);
                ui.add(egui::Label::new(theme::numeric(format!("Step {step} {}", unit.suffix()))).selectable(false))
                    .on_hover_text("How far one nudge, and one snapped step of a drag, goes. Set it in Transform.");
                dot(ui);
                ui.add(egui::Label::new(theme::numeric(format!("Grid {grid} {}", unit.suffix()))).selectable(false))
                    .on_hover_text("Ground grid spacing; set it in the Document panel");
                dot(ui);

                // The unit is a click, not a trip to a settings window: it is
                // the one piece of document state read on every single field.
                let unit = self.unit();
                egui::ComboBox::from_id_salt("status-unit")
                    .selected_text(theme::numeric(unit.suffix()))
                    .width(52.0)
                    .show_ui(ui, |ui| {
                        for option in Unit::ALL {
                            // Switching never rescales the model: the unit only
                            // changes what the fields read (spec section 4).
                            if ui.selectable_label(unit == option, option.suffix()).clicked() {
                                self.scene.settings.unit = option;
                                self.fields.clear();
                            }
                        }
                    });

                dot(ui);

                // Which renderer draws the viewport. A dropup rather than a
                // trip to a settings window, and here beside the frame time it
                // changes: the two are read together or not at all. egui opens
                // the list upwards on its own, this near the bottom of the
                // screen.
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
                    // Asking again clears the last refusal, so a driver that
                    // failed once can be tried again after the user has done
                    // something about it.
                    self.gpu_error = None;
                    if chosen == RenderEngine::Cpu {
                        self.gpu = None;
                        self.gpu_texture = None;
                    }
                    // The viewport is cached on this key; the engine is not part
                    // of it, so the switch has to say the picture is stale.
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

                // The message area, and progress for whatever is in flight.
                if let Some(job) = &self.split_job {
                    // Honest progress, unlike an evaluation's: the cells are
                    // counted before any of them is cut, so the bar knows how
                    // much of the job is left.
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
                } else if let Some(prompt) = &self.file_prompt {
                    // The dialog is a window of the desktop's, not ours, and on
                    // Linux it is the portal's -- which can be slow, or absent,
                    // or simply never answer. It waits on its own thread now, so
                    // this line is here to say what the application is waiting
                    // for rather than to apologise for being frozen.
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
                    // A spinner and the word "Evaluating..." was the whole of
                    // what this said, with no way out of a run that had decided
                    // to take minutes -- while an export in the same bar gets
                    // its elapsed seconds and a Cancel button. There is no
                    // honest progress to show for a boolean, which does not know
                    // how much of itself is left, but how long the user has been
                    // waiting is always knowable and Stop always available.
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
                    // A message fades out once it has had time to be read, so
                    // the bar stops reporting something that finished minutes
                    // ago as though it had just happened.
                    let opacity = crate::app::status_opacity(&self.status, self.status_at.elapsed());
                    if opacity > 0.0 {
                        // The message is the one thing on this bar whose length
                        // is not ours to choose: a message that names a file
                        // names its whole path. It gets what is left once the
                        // readout at the right end has had its room, and is
                        // elided into that -- running underneath the readout,
                        // which is what an unbounded label does, leaves both
                        // unreadable.
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
                    // The scene itself is not a node the document holds: it is
                    // the tree's root, it is always there, and counting it made
                    // an empty document report one node.
                    let nodes = self.scene.len().saturating_sub(1);
                    ui.add(
                        egui::Label::new(theme::numeric(ui::describe_counts(
                            nodes,
                            self.evaluated.mesh.triangle_count(),
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
