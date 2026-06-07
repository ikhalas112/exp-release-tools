use eframe::egui::{self, Color32, Frame, Margin, RichText, ScrollArea, Stroke, TextEdit};

use crate::model::{AssetSelectionKind, MaxionGuiApp};
use crate::theme::{
    icon, ICON_APP, ICON_DONE, ICON_ERROR, ICON_FILE, ICON_FOLDER, ICON_LOG, ICON_OUTPUT,
};

pub fn render(app: &mut MaxionGuiApp, ctx: &egui::Context) {
    render_header(ctx);
    render_body(app, ctx);
}

fn render_header(ctx: &egui::Context) {
    egui::TopBottomPanel::top("header_panel")
        .frame(
            Frame::default()
                .fill(Color32::from_rgb(54, 44, 34))
                .inner_margin(Margin::symmetric(18, 16)),
        )
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(icon(ICON_APP, Color32::from_rgb(244, 216, 162), 28.0));
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new("Maxion Protector GUI")
                            .size(26.0)
                            .color(Color32::from_rgb(251, 245, 233))
                            .strong(),
                    );
                    ui.label(
                        RichText::new(
                            "Pack selected files and folders into your executable while preserving their original paths.",
                        )
                        .size(13.0)
                        .color(Color32::from_rgb(216, 204, 186)),
                    );
                });
            });
        });
}

fn render_body(app: &mut MaxionGuiApp, ctx: &egui::Context) {
    egui::CentralPanel::default()
        .frame(
            Frame::default()
                .fill(Color32::from_rgb(245, 240, 231))
                .inner_margin(Margin::same(18)),
        )
        .show(ctx, |ui| {
            ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    render_status(app, ui);
                    ui.add_space(12.0);
                    render_executable_card(app, ui);
                    ui.add_space(12.0);
                    render_options_card(app, ui);
                    ui.add_space(14.0);
                    render_asset_list(app, ui);
                    ui.add_space(14.0);
                    render_protect_cta(app, ui);
                    ui.add_space(14.0);
                    render_log_panel(app, ui);
                });
        });
}

fn render_status(app: &MaxionGuiApp, ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(&app.status_text)
                .color(app.status_color)
                .strong()
                .size(16.0),
        );
        if app.running {
            ui.spinner();
        } else if app.status_color == Color32::from_rgb(67, 119, 74) {
            ui.label(icon(ICON_DONE, Color32::from_rgb(67, 119, 74), 18.0));
        } else if app.status_color == Color32::from_rgb(176, 54, 54) {
            ui.label(icon(ICON_ERROR, Color32::from_rgb(176, 54, 54), 18.0));
        }
    });
}

fn render_executable_card(app: &mut MaxionGuiApp, ui: &mut egui::Ui) {
    Frame::group(ui.style())
        .fill(Color32::from_rgb(252, 249, 243))
        .stroke(Stroke::new(1.0, Color32::from_rgb(219, 205, 184)))
        .inner_margin(Margin::same(12))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(icon(ICON_FILE, Color32::from_rgb(121, 91, 55), 20.0));
                ui.heading("Executable");
            });
            ui.add_space(6.0);

            ui.label("Input .exe");
            if ui.available_width() > 720.0 {
                if render_browse_row(ui, &mut app.config.input_exe, "Browse") {
                    app.pick_input_exe();
                }
            } else {
                ui.add_sized(
                    [ui.available_width(), 30.0],
                    TextEdit::singleline(&mut app.config.input_exe),
                );
                ui.add_space(6.0);
                if browse_button(ui, "Browse Input .exe", ui.available_width()).clicked() {
                    app.pick_input_exe();
                }
            }

            ui.add_space(10.0);
            ui.label("Output Folder");
            if ui.available_width() > 720.0 {
                if render_browse_row(ui, &mut app.config.output_dir, "Browse") {
                    app.pick_output_dir();
                }
            } else {
                ui.add_sized(
                    [ui.available_width(), 30.0],
                    TextEdit::singleline(&mut app.config.output_dir),
                );
                ui.add_space(6.0);
                if browse_button(ui, "Browse Output Folder", ui.available_width()).clicked() {
                    app.pick_output_dir();
                }
            }

            ui.add_space(10.0);
            ui.label("Protected Output");
            let mut output_preview = app.expected_output_exe();
            ui.add_sized(
                [ui.available_width(), 30.0],
                TextEdit::singleline(&mut output_preview).interactive(false),
            );
        });
}

fn render_options_card(app: &mut MaxionGuiApp, ui: &mut egui::Ui) {
    Frame::group(ui.style())
        .fill(Color32::from_rgb(252, 249, 243))
        .stroke(Stroke::new(1.0, Color32::from_rgb(219, 205, 184)))
        .inner_margin(Margin::same(12))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(icon(ICON_OUTPUT, Color32::from_rgb(121, 91, 55), 20.0));
                ui.heading("Protection Options");
            });
            ui.add_space(6.0);

            ui.label("Protect File Types");
            ui.add_sized(
                [ui.available_width(), 30.0],
                TextEdit::singleline(&mut app.config.protect_types),
            );
            ui.label(
                RichText::new(
                    "Leave empty to automatically protect all detected file types from the selected files and folders.",
                )
                .size(11.0)
                .color(Color32::from_rgb(109, 98, 87)),
            );

            ui.add_space(8.0);
            ui.label("Skip File Types");
            ui.add_sized(
                [ui.available_width(), 30.0],
                TextEdit::singleline(&mut app.config.skip_types),
            );
            ui.label(
                RichText::new("Leave empty to skip nothing.")
                    .size(11.0)
                    .color(Color32::from_rgb(109, 98, 87)),
            );

            ui.add_space(10.0);
            if ui.available_width() > 520.0 {
                ui.horizontal(|ui| {
                    ui.label("Compression Level");
                    ui.add_sized(
                        [220.0, 18.0],
                        egui::Slider::new(&mut app.config.compression_level, 0..=11),
                    );
                    ui.label(app.config.compression_level.to_string());
                });
            } else {
                ui.label("Compression Level");
                ui.add_sized(
                    [ui.available_width(), 18.0],
                    egui::Slider::new(&mut app.config.compression_level, 0..=11),
                );
                ui.label(app.config.compression_level.to_string());
            }

            ui.add_space(6.0);
            let is_x64 = app.detected_is_x64().unwrap_or(true);
            ui.horizontal_wrapped(|ui| {
                ui.checkbox(&mut app.config.enable_compress, "Enable compression");
                ui.add_enabled_ui(is_x64, |ui| {
                    ui.checkbox(&mut app.config.use_phase2, "Use Phase 2 embedding");
                });
            });
            if !is_x64 {
                app.config.use_phase2 = false;
                ui.label(
                    RichText::new(
                        "x86 executables use Phase 1 automatically to match the working Zone4 v9 flow.",
                    )
                    .size(11.0)
                    .color(Color32::from_rgb(145, 82, 48)),
                );
            }
        });
}

fn render_asset_list(app: &mut MaxionGuiApp, ui: &mut egui::Ui) {
    let card_frame = Frame::group(ui.style())
        .fill(Color32::from_rgb(248, 244, 237))
        .stroke(Stroke::new(1.0, Color32::from_rgb(219, 205, 184)))
        .inner_margin(Margin::same(12));

    card_frame.show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(icon(ICON_FOLDER, Color32::from_rgb(121, 91, 55), 20.0));
            ui.heading("Protected Assets");
            ui.add_space(8.0);
            ui.label(format!("{} item(s)", app.config.asset_entries.len()));
        });

        ui.add_space(6.0);
        ui.horizontal_wrapped(|ui| {
            if dark_button(ui, "Browse Files").clicked() {
                app.browse_files();
            }
            if dark_button(ui, "Browse Folders").clicked() {
                app.browse_folders();
            }
        });

        ui.add_space(8.0);
        ScrollArea::vertical().max_height(160.0).show(ui, |ui| {
            if app.config.asset_entries.is_empty() {
                ui.label("No files or folders selected yet.");
                return;
            }

            let mut remove_index = None;
            for (index, entry) in app.config.asset_entries.iter().enumerate() {
                ui.horizontal(|ui| {
                    let icon_code = match entry.kind {
                        AssetSelectionKind::File => ICON_FILE,
                        AssetSelectionKind::Folder => ICON_FOLDER,
                    };
                    ui.label(icon(icon_code, Color32::from_rgb(116, 89, 59), 18.0));
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new(&entry.path)
                                .strong()
                                .color(Color32::from_rgb(53, 46, 38)),
                        );
                        ui.label(
                            RichText::new(entry.kind.label())
                                .size(12.0)
                                .color(Color32::from_rgb(110, 104, 97)),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new("Remove").color(Color32::WHITE).strong(),
                                )
                                .fill(Color32::from_rgb(122, 74, 56)),
                            )
                            .clicked()
                        {
                            remove_index = Some(index);
                        }
                    });
                });
                ui.separator();
            }

            if let Some(index) = remove_index {
                app.config.asset_entries.remove(index);
            }
        });
    });
}

fn render_protect_cta(app: &mut MaxionGuiApp, ui: &mut egui::Ui) {
    let button = egui::Button::new(
        RichText::new("Protect Now")
            .size(18.0)
            .color(Color32::WHITE)
            .strong(),
    )
    .fill(Color32::from_rgb(186, 102, 48))
    .min_size(egui::vec2(ui.available_width(), 40.0));

    if ui.add_enabled(!app.running, button).clicked() {
        app.begin_protect();
    }

    ui.add_space(8.0);
    ui.label(
        RichText::new(
            "The GUI will stage your selected files and folders, run pnp protect, and copy the required DLLs to the output folder.",
        )
        .size(12.0)
        .color(Color32::from_rgb(86, 78, 69)),
    );
}

fn render_log_panel(app: &mut MaxionGuiApp, ui: &mut egui::Ui) {
    let frame = Frame::group(ui.style())
        .fill(Color32::from_rgb(38, 36, 32))
        .stroke(Stroke::new(1.0, Color32::from_rgb(84, 80, 72)))
        .inner_margin(Margin::same(14));

    frame.show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label(icon(ICON_LOG, Color32::from_rgb(241, 197, 104), 20.0));
            ui.label(
                RichText::new("Logs and Errors")
                    .size(18.0)
                    .color(Color32::from_rgb(247, 242, 230)),
            );
        });

        ui.add_space(8.0);
        Frame::group(ui.style())
            .fill(Color32::from_rgb(24, 24, 24))
            .stroke(Stroke::new(1.0, Color32::from_rgb(66, 66, 66)))
            .inner_margin(Margin::same(8))
            .show(ui, |ui| {
                let old_override = ui.visuals().override_text_color;
                let old_bg = ui.visuals().widgets.noninteractive.bg_fill;
                let old_fg = ui.visuals().widgets.noninteractive.fg_stroke.color;
                let old_inactive_bg = ui.visuals().widgets.inactive.bg_fill;
                let old_inactive_fg = ui.visuals().widgets.inactive.fg_stroke.color;
                let old_active_bg = ui.visuals().widgets.active.bg_fill;
                let old_active_fg = ui.visuals().widgets.active.fg_stroke.color;
                ui.visuals_mut().override_text_color = Some(Color32::from_rgb(236, 236, 236));
                ui.visuals_mut().widgets.noninteractive.bg_fill = Color32::from_rgb(24, 24, 24);
                ui.visuals_mut().widgets.noninteractive.fg_stroke.color =
                    Color32::from_rgb(236, 236, 236);
                ui.visuals_mut().widgets.inactive.bg_fill = Color32::from_rgb(24, 24, 24);
                ui.visuals_mut().widgets.inactive.fg_stroke.color =
                    Color32::from_rgb(236, 236, 236);
                ui.visuals_mut().widgets.active.bg_fill = Color32::from_rgb(32, 32, 32);
                ui.visuals_mut().widgets.active.fg_stroke.color =
                    Color32::from_rgb(236, 236, 236);

                ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        ui.add(
                            TextEdit::multiline(&mut app.log_text)
                                .desired_rows(14)
                                .font(egui::TextStyle::Monospace)
                                .interactive(false)
                                .desired_width(f32::INFINITY),
                        );
                    });

                ui.visuals_mut().override_text_color = old_override;
                ui.visuals_mut().widgets.noninteractive.bg_fill = old_bg;
                ui.visuals_mut().widgets.noninteractive.fg_stroke.color = old_fg;
                ui.visuals_mut().widgets.inactive.bg_fill = old_inactive_bg;
                ui.visuals_mut().widgets.inactive.fg_stroke.color = old_inactive_fg;
                ui.visuals_mut().widgets.active.bg_fill = old_active_bg;
                ui.visuals_mut().widgets.active.fg_stroke.color = old_active_fg;
            });
    });
}

fn render_browse_row(ui: &mut egui::Ui, value: &mut String, button_label: &str) -> bool {
    let mut clicked = false;
    ui.horizontal(|ui| {
        let browse_width = 108.0;
        let field_width = (ui.available_width() - browse_width - 8.0).max(120.0);
        ui.add_sized([field_width, 30.0], TextEdit::singleline(value));
        if browse_button(ui, button_label, browse_width).clicked() {
            clicked = true;
        }
    });
    clicked
}

fn browse_button(ui: &mut egui::Ui, label: &str, width: f32) -> egui::Response {
    ui.add_sized(
        [width, 30.0],
        egui::Button::new(RichText::new(label).color(Color32::WHITE).strong())
            .fill(Color32::from_rgb(94, 78, 61)),
    )
}

fn dark_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(label).color(Color32::WHITE).strong())
            .fill(Color32::from_rgb(94, 78, 61)),
    )
}
