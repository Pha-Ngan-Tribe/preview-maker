#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;
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

fn main() -> eframe::Result {
    eframe::run_native(
        "Preview Maker",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([850.0, 700.0])
                .with_min_inner_size([480.0, 400.0]),
            ..Default::default()
        },
        Box::new(|_| Ok(Box::<PreviewApp>::default())),
    )
}

#[derive(Default)]
struct PreviewApp {
    folder: String,
    status: String,
    running: bool,
    models: Vec<Model>,
    textures: Vec<Option<egui::TextureHandle>>,
    events: Option<Receiver<Event>>,
}

#[derive(Clone, Default)]
struct Model {
    id: String,
    title: String,
    image_path: Option<PathBuf>,
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

        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            ui.vertical_centered(|ui| ui.heading("PREVIEW MAKER"));
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Choose main folder…").clicked() {
                    if let Some(path) = choose_folder() {
                        self.folder = path.to_string_lossy().into_owned();
                        self.status.clear();
                    }
                }
                ui.label(&self.folder);
            });

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(!self.running, egui::Button::new("Find previews"))
                    .clicked()
                {
                    self.start_scan(ctx.clone());
                }
                if ui
                    .add_enabled(!self.running, egui::Button::new("Scan folder"))
                    .clicked()
                {
                    self.start_folder_scan(ctx.clone());
                }
            });
            if !self.status.is_empty() {
                ui.label(&self.status);
            }

            if self.running || !self.models.is_empty() {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let gap = 12.0;
                        let min_card_width = 260.0;
                        let available_width = ui.available_width();
                        let columns = ((available_width + gap) / (min_card_width + gap))
                            .floor()
                            .max(1.0) as usize;
                        let card_width = ((available_width - gap * (columns - 1) as f32)
                            / columns as f32)
                            .max(min_card_width);
                        let image_size = (card_width - 32.0).min(240.0);

                        egui::Grid::new("models")
                            .num_columns(columns)
                            .spacing(egui::vec2(gap, gap))
                            .show(ui, |ui| {
                                for (index, model) in self.models.iter().enumerate() {
                                    egui::Frame::group(ui.style()).show(ui, |ui| {
                                        ui.set_min_width(card_width);
                                        ui.set_max_width(card_width);
                                        ui.set_min_height(240.0);
                                        ui.vertical_centered(|ui| {
                                            if let Some(texture) =
                                                self.textures.get(index).and_then(Option::as_ref)
                                            {
                                                ui.add(
                                                    egui::Image::new(texture).fit_to_exact_size(
                                                        egui::vec2(image_size, image_size),
                                                    ),
                                                );
                                            } else {
                                                ui.allocate_exact_size(
                                                    egui::vec2(image_size, image_size),
                                                    egui::Sense::hover(),
                                                );
                                                ui.label(if model.title.is_empty() {
                                                    "Searching…"
                                                } else {
                                                    &model.title
                                                });
                                            }
                                            ui.label(&model.id);
                                            if !model.title.is_empty()
                                                && self
                                                    .textures
                                                    .get(index)
                                                    .and_then(Option::as_ref)
                                                    .is_some()
                                            {
                                                ui.colored_label(
                                                    egui::Color32::from_rgb(190, 92, 130),
                                                    &model.title,
                                                );
                                            }
                                        });
                                    });
                                    if (index + 1) % columns == 0 {
                                        ui.end_row();
                                    }
                                }
                            });
                    });
            }
        });

        if self.running {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }
}

impl PreviewApp {
    fn start_folder_scan(&mut self, ctx: egui::Context) {
        let root = PathBuf::from(self.folder.trim());
        if !root.is_dir() {
            self.status = "Choose an existing main folder first.".into();
            return;
        }

        self.running = true;
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
            }
            Err(error) => model.title = format!("No preview: {error}"),
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
