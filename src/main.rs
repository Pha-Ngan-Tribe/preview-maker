#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui::{
    self, Align, Color32, CornerRadius, FontId, Layout, Margin, RichText, Stroke, StrokeKind,
    TextStyle, vec2,
};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::Duration,
};

const API_URL: &str = "https://3ddd.ru/api/models";
const IMAGE_HOSTS: [&str; 3] = [
    "https://b5.3ddd.ru/media/cache/tuk_model_custom_filter_ang_ru/",
    "https://b6.3ddd.ru/media/cache/tuk_model_custom_filter_ang_ru/",
    "https://b7.3ddd.ru/media/cache/tuk_model_custom_filter_ang_ru/",
];

// ---------- palette ----------
const BG: Color32 = Color32::from_rgb(15, 20, 25);
const PANEL: Color32 = Color32::from_rgb(19, 26, 34);
const CARD: Color32 = Color32::from_rgb(20, 27, 35);
const WELL: Color32 = Color32::from_rgb(11, 16, 21);
const BORDER: Color32 = Color32::from_rgb(33, 43, 55);
const BORDER_STRONG: Color32 = Color32::from_rgb(48, 60, 76);
const TEXT: Color32 = Color32::from_rgb(230, 237, 243);
const MUTED: Color32 = Color32::from_rgb(135, 149, 163);
const DIM: Color32 = Color32::from_rgb(109, 123, 137);
const ACCENT: Color32 = Color32::from_rgb(31, 143, 130);
const ACCENT_BG: Color32 = Color32::from_rgb(22, 52, 47);
const ACCENT_TEXT: Color32 = Color32::from_rgb(111, 224, 208);
const DANGER: Color32 = Color32::from_rgb(217, 83, 79);
const DANGER_TEXT: Color32 = Color32::from_rgb(229, 138, 134);
const DANGER_BORDER: Color32 = Color32::from_rgb(90, 47, 46);

fn main() -> eframe::Result {
    eframe::run_native(
        "Preview maker",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([1100.0, 820.0])
                .with_min_inner_size([640.0, 500.0]),
            ..Default::default()
        },
        Box::new(|cc| {
            apply_theme(&cc.egui_ctx);
            Ok(Box::<PreviewApp>::default())
        }),
    )
}

fn apply_theme(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();

    style.spacing.item_spacing = vec2(10.0, 10.0);
    style.spacing.button_padding = vec2(14.0, 6.0);

    style
        .text_styles
        .insert(TextStyle::Heading, FontId::proportional(18.0));
    style
        .text_styles
        .insert(TextStyle::Body, FontId::proportional(13.5));
    style
        .text_styles
        .insert(TextStyle::Button, FontId::proportional(13.5));
    style
        .text_styles
        .insert(TextStyle::Small, FontId::proportional(11.5));
    style
        .text_styles
        .insert(TextStyle::Monospace, FontId::monospace(12.0));

    let mut v = egui::Visuals::dark();
    v.panel_fill = BG;
    v.window_fill = PANEL;
    v.extreme_bg_color = WELL;
    v.selection.bg_fill = ACCENT;
    v.override_text_color = Some(TEXT);

    let radius = CornerRadius::same(6);
    v.widgets.inactive.corner_radius = radius;
    v.widgets.hovered.corner_radius = radius;
    v.widgets.active.corner_radius = radius;
    v.widgets.noninteractive.corner_radius = radius;

    v.widgets.inactive.weak_bg_fill = Color32::from_rgb(24, 33, 43);
    v.widgets.inactive.bg_fill = Color32::from_rgb(24, 33, 43);
    v.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, BORDER_STRONG);
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(32, 43, 56);
    v.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, Color32::from_rgb(70, 86, 106));
    v.widgets.active.weak_bg_fill = Color32::from_rgb(38, 51, 67);

    style.visuals = v;
    ctx.set_style(style);
}

// ---------- widgets ----------
fn primary_button(text: &str) -> egui::Button<'static> {
    egui::Button::new(
        RichText::new(text.to_owned())
            .color(Color32::WHITE)
            .strong(),
    )
    .fill(ACCENT)
    .stroke(Stroke::new(1.0_f32, ACCENT))
    .min_size(vec2(0.0, 32.0))
}

fn secondary_button(text: &str) -> egui::Button<'static> {
    egui::Button::new(RichText::new(text.to_owned())).min_size(vec2(0.0, 32.0))
}

fn chip(ui: &mut egui::Ui, label: &str, selected: bool) -> bool {
    let button = egui::Button::new(
        RichText::new(label.to_owned())
            .size(12.0)
            .color(if selected { ACCENT_TEXT } else { MUTED }),
    )
    .fill(if selected {
        ACCENT_BG
    } else {
        Color32::TRANSPARENT
    })
    .stroke(Stroke::new(
        1.0_f32,
        if selected { ACCENT } else { BORDER_STRONG },
    ))
    .corner_radius(CornerRadius::same(12))
    .min_size(vec2(0.0, 24.0));
    ui.add(button).clicked()
}

fn model_card(ui: &mut egui::Ui, model: &Model, texture: Option<&egui::TextureHandle>, width: f32) {
    const PAD: f32 = 10.0;
    let failed = matches!(model.state, State::Failed(_));
    let inner = (width - PAD * 2.0 - 2.0).max(60.0);

    let out = egui::Frame::new()
        .fill(CARD)
        .stroke(Stroke::new(
            1.0_f32,
            if failed { DANGER_BORDER } else { BORDER },
        ))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::same(PAD as i8))
        .show(ui, |ui| {
            ui.set_width(inner);
            ui.vertical(|ui| {
                ui.set_width(inner);
                ui.spacing_mut().item_spacing.y = 3.0;

                let image_size = inner.min(280.0);
                let (slot, _) =
                    ui.allocate_exact_size(vec2(inner, image_size), egui::Sense::hover());
                let rect =
                    egui::Rect::from_center_size(slot.center(), vec2(image_size, image_size));
                ui.painter().rect_filled(
                    rect,
                    6.0,
                    if failed {
                        Color32::from_rgb(24, 22, 26)
                    } else {
                        Color32::from_rgb(26, 34, 44)
                    },
                );

                if let Some(texture) = texture {
                    let source_size = texture.size_vec2();
                    let scale = (rect.width() / source_size.x).min(rect.height() / source_size.y);
                    let image_rect =
                        egui::Rect::from_center_size(rect.center(), source_size * scale);
                    ui.put(
                        image_rect,
                        egui::Image::new(texture)
                            .fit_to_exact_size(image_rect.size())
                            .corner_radius(6.0),
                    );
                } else if matches!(model.state, State::Pending) {
                    let spinner_rect =
                        egui::Rect::from_center_size(rect.center(), vec2(26.0, 26.0));
                    egui::Spinner::new()
                        .size(26.0)
                        .color(ACCENT)
                        .paint_at(ui, spinner_rect);
                } else {
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "No image",
                        FontId::proportional(13.0),
                        DIM,
                    );
                }

                ui.add_space(5.0);

                // primary line
                let (title, title_color) = match (&model.state, model.title.is_empty()) {
                    (State::Pending, _) => ("Loading…".to_owned(), MUTED),
                    (_, true) => ("Untitled model".to_owned(), MUTED),
                    (_, false) => (model.title.clone(), TEXT),
                };
                ui.add(
                    egui::Label::new(RichText::new(title).strong().color(title_color)).truncate(),
                );

                // error line
                if let State::Failed(error) = &model.state {
                    ui.add(
                        egui::Label::new(RichText::new(error).small().color(DANGER_TEXT))
                            .truncate(),
                    )
                    .on_hover_text(error);
                }

                // id line
                ui.add(
                    egui::Label::new(RichText::new(&model.id).monospace().small().color(DIM))
                        .truncate(),
                )
                .on_hover_text(&model.id);
            });
        });

    if ui.rect_contains_pointer(out.response.rect) {
        ui.painter().rect_stroke(
            out.response.rect,
            8.0,
            Stroke::new(1.0_f32, if failed { DANGER } else { ACCENT }),
            StrokeKind::Inside,
        );
    }
}

// ---------- app ----------
#[derive(Clone, Copy, PartialEq, Eq, Default)]
enum Filter {
    #[default]
    All,
    Saved,
    Failed,
}

#[derive(Default)]
struct PreviewApp {
    folder: String,
    status: String,
    running: bool,
    fetching: bool,
    filter: Filter,
    models: Vec<Model>,
    textures: Vec<Option<egui::TextureHandle>>,
    events: Option<Receiver<Event>>,
}

#[derive(Clone, Default, PartialEq)]
enum State {
    #[default]
    Pending,
    Saved,
    Failed(String),
}

#[derive(Clone, Default)]
struct Model {
    id: String,
    title: String,
    image_path: Option<PathBuf>,
    state: State,
}

enum Event {
    Found(Vec<Model>),
    Updated(Model),
    Status(String),
    Finished(String),
}

impl eframe::App for PreviewApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.receive_events(ctx);

        // drag & drop a folder onto the window
        if !self.running {
            let dropped = ctx.input(|i| i.raw.dropped_files.clone());
            if let Some(path) = dropped
                .into_iter()
                .find_map(|file| file.path)
                .filter(|path| path.is_dir())
            {
                self.set_folder(path);
            }
        }

        // ----- header -----
        egui::TopBottomPanel::top("header")
            .frame(
                egui::Frame::new()
                    .fill(PANEL)
                    .stroke(Stroke::new(1.0_f32, BORDER))
                    .inner_margin(Margin::symmetric(20, 12)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let enabled = !self.running;
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui
                            .add_enabled(enabled, primary_button("Find and save previews"))
                            .clicked()
                        {
                            self.start_scan(ctx.clone());
                        }
                        if ui
                            .add_enabled(enabled, secondary_button("Scan existing"))
                            .on_hover_text("Checks local JPG and PNG files only")
                            .clicked()
                        {
                            self.start_folder_scan(ctx.clone());
                        }
                        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                            ui.label(RichText::new("Preview maker").heading().strong());
                            ui.add_space(12.0);
                            if ui
                                .add_enabled(enabled, secondary_button("Choose folder…"))
                                .clicked()
                            {
                                if let Some(path) = choose_folder() {
                                    self.set_folder(path);
                                }
                            }
                            ui.add_enabled(
                                enabled,
                                egui::TextEdit::singleline(&mut self.folder)
                                    .hint_text("Folder containing your model folders")
                                    .font(TextStyle::Monospace)
                                    .margin(Margin::symmetric(10, 7))
                                    .desired_width(ui.available_width()),
                            );
                        });
                    });
                });
            });

        // ----- footer / status -----
        if self.running || !self.status.is_empty() {
            egui::TopBottomPanel::bottom("status")
                .frame(
                    egui::Frame::new()
                        .fill(PANEL)
                        .stroke(Stroke::new(1.0_f32, BORDER))
                        .inner_margin(Margin::symmetric(20, 8)),
                )
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        if self.running {
                            ui.add(egui::Spinner::new().size(14.0).color(ACCENT));
                        }
                        ui.label(RichText::new(&self.status).small().color(MUTED));
                    });
                });
        }

        // ----- content -----
        let mut pick_folder = false;
        let mut retry_failed_clicked = false;
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(BG)
                    .inner_margin(Margin::symmetric(20, 14)),
            )
            .show(ctx, |ui| {
                let total = self.models.len();
                let saved = self.models.iter().filter(|m| m.state == State::Saved).count();
                let failed = self
                    .models
                    .iter()
                    .filter(|m| matches!(m.state, State::Failed(_)))
                    .count();
                let pending = self.models.iter().filter(|m| m.state == State::Pending).count();

                ui.allocate_ui_with_layout(
                    vec2(ui.available_width(), 40.0),
                    Layout::left_to_right(Align::Center),
                    |ui| {
                    ui.heading("Previews");
                    if total > 0 {
                        ui.label(RichText::new(format!("{saved} of {total} saved")).color(MUTED));
                        if failed > 0
                            && ui
                                .add_enabled(
                                    !self.running,
                                    secondary_button("Retry failed"),
                                )
                                .clicked()
                        {
                            retry_failed_clicked = true;
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.spacing_mut().item_spacing.x = 6.0;
                            if chip(ui, &format!("Errors {failed}"), self.filter == Filter::Failed) {
                                self.filter = Filter::Failed;
                            }
                            if chip(ui, &format!("Saved {saved}"), self.filter == Filter::Saved) {
                                self.filter = Filter::Saved;
                            }
                            if chip(ui, &format!("All {total}"), self.filter == Filter::All) {
                                self.filter = Filter::All;
                            }
                        });
                    }
                    },
                );

                if self.running && self.fetching && total > 0 {
                    let fraction = (total - pending) as f32 / total as f32;
                    ui.add(
                        egui::ProgressBar::new(fraction)
                            .desired_height(4.0)
                            .fill(ACCENT)
                            .corner_radius(CornerRadius::same(2)),
                    );
                }
                ui.add_space(4.0);

                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if self.models.is_empty() {
                            ui.add_space(70.0);
                            ui.vertical_centered(|ui| {
                                ui.label(RichText::new("▧").size(44.0).color(BORDER_STRONG));
                                ui.add_space(6.0);
                                ui.label(RichText::new("Pick a model folder").heading());
                                ui.label(
                                    RichText::new(
                                        "Choose it above or drop it onto this window. Then find previews online or scan images already saved there.",
                                    )
                                    .color(MUTED),
                                );
                                ui.add_space(8.0);
                                if ui
                                    .add_enabled(!self.running, primary_button("Choose folder…"))
                                    .clicked()
                                {
                                    pick_folder = true;
                                }
                            });
                            return;
                        }

                        let visible: Vec<usize> = self
                            .models
                            .iter()
                            .enumerate()
                            .filter(|(_, m)| match self.filter {
                                Filter::All => true,
                                Filter::Saved => m.state == State::Saved,
                                Filter::Failed => matches!(m.state, State::Failed(_)),
                            })
                            .map(|(i, _)| i)
                            .collect();

                        if visible.is_empty() {
                            ui.add_space(40.0);
                            ui.vertical_centered(|ui| {
                                ui.label(RichText::new("Nothing matches this filter").color(MUTED));
                            });
                            return;
                        }

                        let gap = 14.0;
                        let min_card = 260.0;
                        // Leave the same visual gutter at the right as the grid gap.
                        let available = (ui.available_width() - gap).max(1.0);
                        let columns = (((available + gap) / (min_card + gap)).floor() as usize).max(1);
                        let card_width =
                            ((available - gap * (columns - 1) as f32) / columns as f32).max(1.0);

                        ui.spacing_mut().item_spacing = vec2(gap, gap);
                        for row in visible.chunks(columns) {
                            ui.horizontal_top(|ui| {
                                for &index in row {
                                    model_card(
                                        ui,
                                        &self.models[index],
                                        self.textures.get(index).and_then(Option::as_ref),
                                        card_width,
                                    );
                                }
                            });
                        }
                        ui.add_space(8.0);
                    });
            });

        if pick_folder {
            if let Some(path) = choose_folder() {
                self.set_folder(path);
            }
        }
        if retry_failed_clicked {
            self.retry_failed(ctx.clone());
        }

        if self.running {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }
}

impl PreviewApp {
    fn set_folder(&mut self, path: PathBuf) {
        self.folder = path.to_string_lossy().into_owned();
        self.status.clear();
        self.models.clear();
        self.textures.clear();
        self.filter = Filter::All;
    }

    fn start_folder_scan(&mut self, ctx: egui::Context) {
        let root = PathBuf::from(self.folder.trim());
        if !root.is_dir() {
            self.status = "Choose an existing main folder first.".into();
            return;
        }

        self.running = true;
        self.fetching = false;
        self.filter = Filter::All;
        self.status = "Looking for preview images…".into();
        self.models.clear();
        self.textures.clear();
        let (sender, receiver) = mpsc::channel();
        self.events = Some(receiver);
        thread::spawn(move || {
            let result = scan_folder_images(&root, &sender);
            let _ = sender.send(Event::Finished(result));
            ctx.request_repaint();
        });
    }

    fn start_scan(&mut self, ctx: egui::Context) {
        let root = PathBuf::from(self.folder.trim());
        if !root.is_dir() {
            self.status = "Choose an existing main folder first.".into();
            return;
        }

        self.running = true;
        self.fetching = true;
        self.filter = Filter::All;
        self.status = "Reading folders…".into();
        self.models.clear();
        self.textures.clear();
        let (sender, receiver) = mpsc::channel();
        self.events = Some(receiver);
        thread::spawn(move || {
            let result = scan_and_fetch(&root, &sender);
            let _ = sender.send(Event::Finished(result));
            ctx.request_repaint();
        });
    }

    fn retry_failed(&mut self, ctx: egui::Context) {
        let root = PathBuf::from(self.folder.trim());
        let folders: Vec<(String, PathBuf)> = self
            .models
            .iter()
            .filter(|model| matches!(model.state, State::Failed(_)))
            .map(|model| (model.id.clone(), root.join(&model.id)))
            .collect();
        if folders.is_empty() {
            return;
        }

        for (index, model) in self.models.iter_mut().enumerate() {
            if matches!(model.state, State::Failed(_)) {
                model.state = State::Pending;
                model.title.clear();
                model.image_path = None;
                self.textures[index] = None;
            }
        }

        self.running = true;
        self.fetching = false;
        self.status = format!("Retrying {} failed previews…", folders.len());
        let (sender, receiver) = mpsc::channel();
        self.events = Some(receiver);
        thread::spawn(move || {
            let result = fetch_folders(&folders, &sender);
            let _ = sender.send(Event::Finished(result));
            ctx.request_repaint();
        });
    }

    fn receive_events(&mut self, ctx: &egui::Context) {
        let Some(receiver) = &self.events else { return };
        while let Ok(event) = receiver.try_recv() {
            match event {
                Event::Found(models) => {
                    self.textures = models
                        .iter()
                        .map(|model| {
                            model.image_path.as_ref().and_then(|path| {
                                load_texture(ctx, path, &format!("{}:{}", model.id, model.title))
                            })
                        })
                        .collect();
                    self.models = models;
                }
                Event::Updated(model) => {
                    if let Some(index) = self
                        .models
                        .iter()
                        .position(|current| current.id == model.id)
                    {
                        self.textures[index] = model
                            .image_path
                            .as_ref()
                            .and_then(|path| load_texture(ctx, path, &model.id));
                        self.models[index] = model;
                    }
                }
                Event::Status(status) => self.status = status,
                Event::Finished(status) => {
                    self.status = status;
                    self.running = false;
                }
            }
        }
    }
}

fn choose_folder() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    let output = Command::new("osascript")
        .args([
            "-e",
            "POSIX path of (choose folder with prompt \"Choose main folder\")",
        ])
        .output()
        .ok()?;

    #[cfg(target_os = "windows")]
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-Command",
            "Add-Type -AssemblyName System.Windows.Forms; $d=New-Object System.Windows.Forms.FolderBrowserDialog; if($d.ShowDialog() -eq 'OK'){Write-Output $d.SelectedPath}",
        ])
        .output()
        .ok()?;

    #[cfg(target_os = "linux")]
    let output = Command::new("zenity")
        .args([
            "--file-selection",
            "--directory",
            "--title=Choose main folder",
        ])
        .output()
        .or_else(|_| {
            Command::new("kdialog")
                .args(["--getexistingdirectory", ".", "Choose main folder"])
                .output()
        })
        .ok()?;

    if !output.status.success() {
        return None;
    }
    let path = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    (!path.is_empty()).then(|| PathBuf::from(path))
}

fn scan_and_fetch(root: &Path, events: &Sender<Event>) -> String {
    let folders = match fs::read_dir(root) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let path = entry.path();
                let name = entry.file_name().to_string_lossy().into_owned();
                (path.is_dir() && is_model_id(&name)).then_some((name, path))
            })
            .collect::<Vec<_>>(),
        Err(error) => return format!("Could not read folder: {error}"),
    };

    if folders.is_empty() {
        return "No ID-named folders found in the selected folder.".into();
    }

    let models: Vec<Model> = folders
        .iter()
        .map(|(id, _)| Model {
            id: id.clone(),
            ..Default::default()
        })
        .collect();
    let _ = events.send(Event::Found(models));

    fetch_folders(&folders, events)
}

fn fetch_folders(folders: &[(String, PathBuf)], events: &Sender<Event>) -> String {
    let total = folders.len();
    let mut downloaded = 0;
    for (index, (id, folder)) in folders.iter().enumerate() {
        let _ = events.send(Event::Status(format!(
            "Requesting {id} ({}/{total})…",
            index + 1
        )));
        let mut model = Model {
            id: id.clone(),
            ..Default::default()
        };
        match fetch_and_save(id, folder) {
            Ok((title, path)) => {
                downloaded += 1;
                model.title = title;
                model.image_path = Some(path);
                model.state = State::Saved;
            }
            Err(error) => {
                let first_line = error.lines().next().unwrap_or("Unknown error").to_owned();
                model.state = State::Failed(first_line);
            }
        }
        let _ = events.send(Event::Updated(model));
    }
    format!("Done: {downloaded} of {total} preview images saved in their model folders.")
}

fn scan_folder_images(root: &Path, events: &Sender<Event>) -> String {
    let model_folder_count = fs::read_dir(root)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .filter(|entry| {
                    entry.path().is_dir() && is_model_id(&entry.file_name().to_string_lossy())
                })
                .count()
        })
        .unwrap_or(0);
    let mut pending = vec![root.to_path_buf()];
    let mut models = Vec::new();
    while let Some(folder) = pending.pop() {
        let entries = match fs::read_dir(&folder) {
            Ok(entries) => entries,
            Err(error) => return format!("Could not read {}: {error}", folder.display()),
        };
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            let file_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(_) => continue,
            };
            if file_type.is_dir() {
                if !name.starts_with('.') {
                    pending.push(path);
                }
            } else if file_type.is_file() && is_preview_image(&path) {
                models.push(Model {
                    id: path
                        .parent()
                        .and_then(Path::file_name)
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "root".into()),
                    title: name,
                    image_path: Some(path),
                    state: State::Saved,
                });
            }
        }
    }
    models.sort_by(|a, b| a.id.cmp(&b.id).then(a.title.cmp(&b.title)));
    let count = models.len();
    let _ = events.send(Event::Found(models));
    format!("Found {model_folder_count} ID folders and {count} preview images (JPG and PNG).")
}

fn is_preview_image(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "jpg" | "jpeg" | "png"
            )
        })
}

fn is_model_id(name: &str) -> bool {
    let Some((number, uuid)) = name.split_once('.') else {
        return false;
    };
    !number.is_empty()
        && number.bytes().all(|byte| byte.is_ascii_digit())
        && !uuid.is_empty()
        && uuid.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn fetch_and_save(id: &str, folder: &Path) -> Result<(String, PathBuf), String> {
    let body = serde_json::to_string(&serde_json::json!({ "query": id }))
        .map_err(|error| error.to_string())?;
    let response = Command::new("curl")
        .args([
            "-sS",
            "-f",
            "--max-time",
            "60",
            "-H",
            "Content-Type: application/json",
            "-d",
            &body,
            API_URL,
        ])
        .output()
        .map_err(|error| format!("Could not start request: {error}"))?;
    if !response.status.success() {
        return Err(String::from_utf8_lossy(&response.stderr).trim().to_owned());
    }
    let json: serde_json::Value = serde_json::from_slice(&response.stdout)
        .map_err(|error| format!("Invalid API response: {error}"))?;
    let model = &json["data"]["models"][0];
    let title = model["title_en"]
        .as_str()
        .unwrap_or("Model preview")
        .to_owned();
    let web_path = model["images"][0]["web_path"]
        .as_str()
        .ok_or("API returned no image")?;
    let filename = web_path
        .rsplit('/')
        .next()
        .filter(|name| !name.is_empty())
        .ok_or("Invalid image path")?;

    for host in IMAGE_HOSTS {
        let image = Command::new("curl")
            .args([
                "-sS",
                "-f",
                "--max-time",
                "60",
                &format!("{host}{web_path}"),
            ])
            .output();
        let Ok(image) = image else { continue };
        if image.status.success() {
            let path = folder.join(filename);
            fs::write(&path, image.stdout)
                .map_err(|error| format!("Could not save image: {error}"))?;
            return Ok((title, path));
        }
    }
    Err("image download failed".into())
}

fn load_texture(ctx: &egui::Context, path: &Path, id: &str) -> Option<egui::TextureHandle> {
    let bytes = fs::read(path).ok()?;
    let image = image::load_from_memory(&bytes).ok()?.to_rgba8();
    let size = [image.width() as usize, image.height() as usize];
    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw());
    Some(ctx.load_texture(id, color_image, egui::TextureOptions::LINEAR))
}
