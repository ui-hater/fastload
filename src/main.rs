#![windows_subsystem = "windows"]
mod torrent;

use std::cell::Cell;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use wxdragon::prelude::*;
use wxdragon::translations::translate;
use wxdragon::translations::{Locale, Translations, add_catalog_lookup_path_prefix};

const CHUNKS: u64 = 8;
const MIN_CHUNKED_SIZE: u64 = 4 * 1024 * 1024;

#[derive(Clone, Copy)]
struct Ui {
    url_input: TextCtrl,
    download_btn: Button,
    gauge: Gauge,
    status: StaticText,
}

fn setup_translations() {
    add_catalog_lookup_path_prefix(".\\locale");
    let tr = Translations::new();
    tr.set_language(Locale::get_system_language());
    tr.add_catalog("fastload");
    tr.add_std_catalog();
    Translations::set_global(tr);
}
fn main() {
    let _ = wxdragon::main(|_| {
        setup_translations();
        let (frame, ui) = build_ui();
        bind_events(frame, ui);
        frame.show(true);
        frame.centre();
    });
}

fn build_ui() -> (Frame, Ui) {
    let frame = Frame::builder()
        .with_title("fastload")
        .with_size(Size::new(500, 180))
        .build();
    let panel = Panel::builder(&frame).build();

    let url_input = TextCtrl::builder(&panel).build();
    url_input.set_accessibility_label(&translate("Link to file or torrent"));
    let download_btn = Button::builder(&panel)
        .with_label(&translate("Downloa&d"))
        .build();
    let gauge = Gauge::builder(&panel).with_range(100).build();
    let status = StaticText::builder(&panel)
        .with_label(&translate("paste link"))
        .build();

    let row = BoxSizer::builder(Orientation::Horizontal).build();
    row.add(&url_input, 1, SizerFlag::Expand | SizerFlag::Right, 5);
    row.add(&download_btn, 0, SizerFlag::Expand, 0);

    let col = BoxSizer::builder(Orientation::Vertical).build();
    col.add_sizer(&row, 0, SizerFlag::Expand | SizerFlag::All, 10);
    col.add(
        &gauge,
        0,
        SizerFlag::Expand | SizerFlag::Left | SizerFlag::Right,
        10,
    );
    col.add(&status, 0, SizerFlag::Expand | SizerFlag::All, 10);
    panel.set_sizer(col, true);

    (
        frame,
        Ui {
            url_input,
            download_btn,
            gauge,
            status,
        },
    )
}

fn bind_events(frame: Frame, ui: Ui) {
    ui.download_btn
        .on_click(move |_| on_download_clicked(&frame, ui));
}

fn on_download_clicked(frame: &Frame, ui: Ui) {
    let url = ui.url_input.get_value().trim().to_string();
    if url.is_empty() {
        ui.status.set_label(&translate("please paste a link"));
        return;
    }
    if torrent::is_torrent(&url) {
        start_torrent_download(frame, ui, url);
    } else {
        start_http_download(frame, ui, url);
    }
}

fn start_http_download(frame: &Frame, ui: Ui, url: String) {
    let Some(path) = ask_save_path(frame, &file_name_from_url(&url)) else {
        return;
    };
    start_download(ui);
    thread::spawn(move || {
        let result = download(&url, &path, progress_reporter(ui));
        report_finished(ui, result);
    });
}

fn start_torrent_download(frame: &Frame, ui: Ui, url: String) {
    let Some(dir) = ask_save_dir(frame) else {
        return;
    };
    start_download(ui);
    thread::spawn(move || {
        let result = torrent::download(&url, &dir, progress_reporter(ui));
        report_finished(ui, result);
    });
}

fn start_download(ui: Ui) {
    ui.download_btn.enable(false);
    ui.gauge.set_value(0);
    ui.status.set_label(&translate("downloading..."));
}

fn ask_save_path(parent: &Frame, default_name: &str) -> Option<String> {
    let dialog = FileDialog::builder(parent)
        .with_message(&translate("save as"))
        .with_default_file(default_name)
        .with_wildcard(&translate("All files (*.*)|*.*"))
        .with_style(FileDialogStyle::Save | FileDialogStyle::OverwritePrompt)
        .build();
    if dialog.show_modal() != ID_OK {
        return None;
    }
    dialog.get_path()
}

fn ask_save_dir(parent: &Frame) -> Option<String> {
    let dialog =
        DirDialog::builder(parent, &translate("Please select destination folder"), "").build();
    if dialog.show_modal() != ID_OK {
        return None;
    }
    dialog.get_path()
}

fn file_name_from_url(url: &str) -> String {
    let path = url.split(['?', '#']).next().unwrap_or("");
    match path.rsplit('/').next() {
        Some(name) if !name.is_empty() && !name.contains(':') => name.to_string(),
        _ => "download".to_string(),
    }
}

fn download(url: &str, path: &str, on_progress: impl Fn(u64, Option<u64>)) -> Result<u64, String> {
    match probe_range_support(url) {
        Some(total) if total >= MIN_CHUNKED_SIZE => download_chunked(url, path, total, on_progress),
        _ => download_single(url, path, on_progress),
    }
}

/// Returns the file size if the server answers a range request with 206.
fn probe_range_support(url: &str) -> Option<u64> {
    let response = ureq::get(url)
        .header("Range", "bytes=0-0")
        .header("Accept-Encoding", "identity")
        .call()
        .ok()?;
    if response.status() != 206 {
        return None;
    }
    let range = response.headers().get("content-range")?.to_str().ok()?;
    range.rsplit('/').next()?.parse().ok()
}

fn download_chunked(
    url: &str,
    path: &str,
    total: u64,
    on_progress: impl Fn(u64, Option<u64>),
) -> Result<u64, String> {
    let file = File::create(path).map_err(|e| e.to_string())?;
    file.set_len(total).map_err(|e| e.to_string())?;
    drop(file);

    let done = AtomicU64::new(0);
    let error = OnceLock::new();
    thread::scope(|s| {
        let workers: Vec<_> = chunk_ranges(total, CHUNKS)
            .into_iter()
            .map(|(start, end)| {
                let (done, error) = (&done, &error);
                s.spawn(move || {
                    if let Err(e) = download_range(url, path, start, end, done, error) {
                        let _ = error.set(e);
                    }
                })
            })
            .collect();
        while !workers.iter().all(|w| w.is_finished()) {
            on_progress(done.load(Ordering::Relaxed), Some(total));
            thread::sleep(Duration::from_millis(200));
        }
    });
    match error.into_inner() {
        Some(e) => Err(e),
        None => Ok(total),
    }
}

/// Splits `0..total` into `count` inclusive byte ranges.
fn chunk_ranges(total: u64, count: u64) -> Vec<(u64, u64)> {
    let size = total.div_ceil(count);
    (0..count)
        .map(|i| (i * size, ((i + 1) * size).min(total) - 1))
        .filter(|(start, end)| start <= end)
        .collect()
}

/// Downloads bytes `start..=end` into the file at the same offset.
/// Stops early once another chunk has set `error`.
fn download_range(
    url: &str,
    path: &str,
    start: u64,
    end: u64,
    done: &AtomicU64,
    error: &OnceLock<String>,
) -> Result<(), String> {
    let response = ureq::get(url)
        .header("Range", &format!("bytes={start}-{end}"))
        .header("Accept-Encoding", "identity")
        .call()
        .map_err(|e| e.to_string())?;
    if response.status() != 206 {
        return Err(format!(
            "server ignored range request ({})",
            response.status()
        ));
    }
    let mut reader = response.into_body().into_reader();
    let mut file = OpenOptions::new()
        .write(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    file.seek(SeekFrom::Start(start))
        .map_err(|e| e.to_string())?;

    let mut buf = vec![0u8; 64 * 1024];
    let mut written = 0u64;
    while error.get().is_none() {
        let n = reader.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        written += n as u64;
        done.fetch_add(n as u64, Ordering::Relaxed);
    }
    if error.get().is_none() && written != end - start + 1 {
        return Err("connection closed before chunk was complete".to_string());
    }
    Ok(())
}

fn download_single(
    url: &str,
    path: &str,
    on_progress: impl Fn(u64, Option<u64>),
) -> Result<u64, String> {
    let response = ureq::get(url).call().map_err(|e| e.to_string())?;
    let total = response.body().content_length();
    let mut reader = response.into_body().into_reader();
    let mut file = File::create(path).map_err(|e| e.to_string())?;
    copy_with_progress(&mut reader, &mut file, total, on_progress)
}

fn copy_with_progress(
    reader: &mut impl Read,
    writer: &mut impl Write,
    total: Option<u64>,
    on_progress: impl Fn(u64, Option<u64>),
) -> Result<u64, String> {
    let mut buf = vec![0u8; 64 * 1024];
    let mut done = 0u64;
    loop {
        let n = reader.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            return Ok(done);
        }
        writer.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        done += n as u64;
        on_progress(done, total);
    }
}

/// Returns an `on_progress` callback that measures the speed over the
/// last second and updates the UI once per measurement.
fn progress_reporter(ui: Ui) -> impl Fn(u64, Option<u64>) {
    let last = Cell::new((Instant::now(), 0u64));
    move |done, total| {
        let (since, done_then) = last.get();
        let elapsed = since.elapsed().as_secs_f64();
        if elapsed < 1.0 {
            return;
        }
        let speed = done.saturating_sub(done_then) as f64 / elapsed;
        last.set((Instant::now(), done));
        report_progress(ui, done, total, speed);
    }
}

fn report_progress(ui: Ui, done: u64, total: Option<u64>, speed: f64) {
    let speed = format_speed(speed);
    wxdragon::call_after(Box::new(move || match total {
        Some(t) if t > 0 => {
            ui.gauge.set_value((done * 100 / t) as i32);
            ui.status.set_label(&format!(
                "{} / {} ({speed})",
                format_size(done),
                format_size(t)
            ));
        }
        _ => ui
            .status
            .set_label(&format!("{} ({speed})", format_size(done))),
    }));
}

fn report_finished(ui: Ui, result: Result<u64, String>) {
    wxdragon::call_after(Box::new(move || {
        ui.download_btn.enable(true);
        match result {
            Ok(bytes) => {
                ui.gauge.set_value(100);
                ui.status
                    .set_label(&translate("Done: {}").replace("{}", &format_size(bytes)));
            }
            Err(e) => {
                ui.gauge.set_value(0);
                ui.status
                    .set_label(&translate("Error: {}").replace("{}", &e));
            }
        }
    }));
}

fn format_size(bytes: u64) -> String {
    let mb = bytes as f64 / 1024.0 / 1024.0;
    format!("{mb:.2} MB")
}

fn format_speed(bytes_per_sec: f64) -> String {
    let mb = bytes_per_sec / 1024.0 / 1024.0;
    format!("{mb:.2} MB/s")
}
