//! `rescue-asset-viewer`: browse the user's own dump (egui), or build and search the
//! resource index from the command line.
//!
//! ```text
//! rescue-asset-viewer [ROM]                         open the viewer
//! rescue-asset-viewer index build [ROM] [-o FILE]   write the index (default rescue-index.tsv)
//! rescue-asset-viewer index search TERM [--index FILE] [--rom ROM]
//! ```
//!
//! ROM defaults to `RESCUE_ROM`, then `rom_path` in `./rescue.toml`. Unsupported dumps are
//! refused. Exports go to `extracted/png/` (git-ignored) and are for local use only.

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use eframe::egui;
use rescue_asset_viewer::game::{self, Game};
use rescue_asset_viewer::index::{self, Resource};
use rescue_asset_viewer::png;
use rescue_asset_viewer::view::{self, Options, View};

const EXPORT_DIR: &str = "extracted/png";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("index") => index_cli(&args[1..]),
        Some("-h" | "--help") => {
            println!(
                "usage: rescue-asset-viewer [ROM] | index build [ROM] [-o FILE] | index search TERM [--index FILE] [--rom ROM]"
            );
            Ok(())
        }
        rom => gui(rom),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Value of `--flag VALUE` in `args`.
fn flag<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}

fn load(rom: Option<&str>) -> Result<Game, String> {
    Game::load(&game::rom_path(rom)?)
}

fn index_cli(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("build") => {
            let rom = args
                .get(1)
                .filter(|a| !a.starts_with('-'))
                .map(String::as_str);
            let out = flag(args, "-o").unwrap_or(index::DEFAULT_PATH);
            let idx = index::build(&load(rom)?);
            std::fs::write(out, index::to_tsv(&idx))
                .map_err(|e| format!("cannot write {out}: {e}"))?;
            println!(
                "wrote {} resources to {out} (local only: it holds game data, never commit it)",
                idx.len()
            );
            Ok(())
        }
        Some("search") => {
            let term = args.get(1).ok_or("index search needs a term")?;
            let path = flag(args, "--index").unwrap_or(index::DEFAULT_PATH);
            let idx = match std::fs::read_to_string(path) {
                Ok(s) => index::from_tsv(&s),
                Err(_) => index::build(&load(flag(args, "--rom"))?),
            };
            for r in index::search(&idx, term) {
                println!(
                    "{}\t{}\t{}\t{}\t{}",
                    r.file, r.sub, r.format, r.name, r.info
                );
            }
            Ok(())
        }
        _ => Err(
            "usage: index build [ROM] [-o FILE] | index search TERM [--index FILE] [--rom ROM]"
                .into(),
        ),
    }
}

fn gui(rom: Option<&str>) -> Result<(), String> {
    let g = load(rom)?;
    let index = index::build(&g);
    let app = Viewer {
        g,
        index,
        search: String::new(),
        sel: None,
        opt: Options::default(),
        view: None,
        tex: None,
        zoom: 3,
        playing: true,
        status: String::new(),
    };
    eframe::run_native(
        "rescue asset viewer",
        eframe::NativeOptions::default(),
        Box::new(|_| Ok(Box::new(app))),
    )
    .map_err(|e| e.to_string())
}

/// Selected resource: ROM file path and optional archive sub-file.
type Sel = (String, Option<String>);

struct Viewer {
    g: Game,
    index: Vec<Resource>,
    search: String,
    sel: Option<Sel>,
    opt: Options,
    view: Option<(Sel, Options, View)>,
    tex: Option<egui::TextureHandle>,
    zoom: usize,
    playing: bool,
    status: String,
}

impl Viewer {
    fn select(&mut self, file: &str, sub: Option<&str>) {
        self.sel = Some((file.to_owned(), sub.map(str::to_owned)));
        self.opt = Options::default();
        self.status.clear();
    }

    /// Re-renders when the selection or options changed.
    fn refresh(&mut self, ctx: &egui::Context) {
        let Some(sel) = self.sel.clone() else { return };
        if self
            .view
            .as_ref()
            .is_some_and(|(s, o, _)| *s == sel && *o == self.opt)
        {
            return;
        }
        let v = view::view(&self.g, &sel.0, sel.1.as_deref(), self.opt);
        self.tex = v
            .image
            .as_ref()
            .filter(|i| i.width > 0 && i.height > 0)
            .map(|i| {
                let img = egui::ColorImage::from_rgba_unmultiplied([i.width, i.height], &i.pixels);
                ctx.load_texture("view", img, egui::TextureOptions::NEAREST)
            });
        self.view = Some((sel, self.opt, v));
    }

    fn export(&mut self) {
        let Some((sel, _, v)) = &self.view else {
            return;
        };
        let Some(img) = &v.image else { return };
        let name = format!(
            "{}_{}.png",
            sel.0.replace(['/', '.'], "_"),
            sel.1.as_deref().unwrap_or("file")
        );
        let path: PathBuf = Path::new(EXPORT_DIR).join(name);
        let r = std::fs::create_dir_all(EXPORT_DIR).and_then(|()| {
            std::fs::write(
                &path,
                png::encode(img.width as u32, img.height as u32, &img.pixels),
            )
        });
        self.status = match r {
            Ok(()) => format!("exported {} (local use only)", path.display()),
            Err(e) => format!("export failed: {e}"),
        };
    }

    fn tree(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("search");
            ui.text_edit_singleline(&mut self.search);
        });
        ui.separator();
        let mut pick: Option<(String, Option<String>)> = None;
        egui::ScrollArea::vertical().show(ui, |ui| {
            if !self.search.is_empty() {
                for r in index::search(&self.index, &self.search)
                    .into_iter()
                    .take(500)
                {
                    let label = format!("{} {} [{}] {}", r.file, r.sub, r.format, r.name);
                    if ui.selectable_label(false, label).clicked() {
                        let sub = (!r.sub.is_empty()).then(|| r.sub.clone());
                        pick = Some((r.file.clone(), sub));
                    }
                }
                return;
            }
            for f in self.g.fs.files() {
                match self.g.pack(&f.path) {
                    Some(p) => {
                        egui::CollapsingHeader::new(format!(
                            "{} ({})",
                            f.path,
                            p.archive.entries().len()
                        ))
                        .id_salt(&f.path)
                        .show(ui, |ui| {
                            for e in p.archive.entries() {
                                let on = self.sel.as_ref().is_some_and(|(a, b)| {
                                    *a == f.path && b.as_deref() == Some(e.name.as_str())
                                });
                                if ui.selectable_label(on, &e.name).clicked() {
                                    pick = Some((f.path.clone(), Some(e.name.clone())));
                                }
                            }
                        });
                    }
                    None => {
                        let on = self
                            .sel
                            .as_ref()
                            .is_some_and(|(a, b)| *a == f.path && b.is_none());
                        if ui.selectable_label(on, &f.path).clicked() {
                            pick = Some((f.path.clone(), None));
                        }
                    }
                }
            }
        });
        if let Some((f, s)) = pick {
            self.select(&f, s.as_deref());
        }
    }

    fn body(&mut self, ui: &mut egui::Ui) {
        let Some((sel, _, v)) = &self.view else {
            ui.label(format!("{} — pick a file on the left.", self.g.id));
            return;
        };
        let is_sprite = v.kind.starts_with("sprite");
        ui.horizontal(|ui| {
            ui.heading(format!("{} {}", sel.0, sel.1.as_deref().unwrap_or("")));
            ui.label(&v.kind);
        });
        let mut export = false;
        ui.horizontal(|ui| {
            ui.add(egui::Slider::new(&mut self.zoom, 1..=8).text("zoom"));
            if v.image.is_some() && ui.button("export PNG").clicked() {
                export = true;
            }
            if is_sprite {
                ui.add(egui::Slider::new(&mut self.opt.group, 0..=40).text("animation"));
                ui.add(egui::Slider::new(&mut self.opt.direction, 0..=7).text("direction"));
                ui.checkbox(&mut self.playing, "play");
            }
            ui.label(&self.status);
        });
        if export {
            self.export();
        }
        let Some((_, _, v)) = &self.view else { return };
        if let Some(t) = &self.tex {
            let size = t.size_vec2() * self.zoom as f32;
            egui::ScrollArea::both()
                .id_salt("img")
                .max_height(ui.available_height() * 0.6)
                .show(ui, |ui| {
                    ui.add(egui::Image::new(egui::load::SizedTexture::new(
                        t.id(),
                        size,
                    )));
                });
        }
        ui.separator();
        let lines = &v.lines;
        let hex = v.bytes.len().div_ceil(16);
        let rows = if lines.is_empty() { hex } else { lines.len() };
        egui::ScrollArea::vertical()
            .id_salt("text")
            .show_rows(ui, 14.0, rows, |ui, range| {
                for i in range {
                    let s = if lines.is_empty() {
                        let chunk = &v.bytes[i * 16..(i * 16 + 16).min(v.bytes.len())];
                        let hx: Vec<String> = chunk.iter().map(|b| format!("{b:02x}")).collect();
                        let asc: String = chunk
                            .iter()
                            .map(|&b| {
                                if b.is_ascii_graphic() {
                                    char::from(b)
                                } else {
                                    '.'
                                }
                            })
                            .collect();
                        format!("{:08x}  {:<48}  {asc}", i * 16, hx.join(" "))
                    } else {
                        lines[i].clone()
                    };
                    ui.monospace(s);
                }
            });
    }
}

impl eframe::App for Viewer {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        if self.playing
            && self
                .view
                .as_ref()
                .is_some_and(|(_, _, v)| v.kind.starts_with("sprite"))
        {
            // DS runs at ~60 game frames per second.
            self.opt.tick = (ctx.input(|i| i.time) * 60.0) as u32;
            ctx.request_repaint();
        }
        self.refresh(&ctx);
        egui::Panel::left("tree")
            .resizable(true)
            .default_size(280.0)
            .show(ui, |ui| self.tree(ui));
        egui::CentralPanel::default().show(ui, |ui| self.body(ui));
    }
}
