#![windows_subsystem = "windows"]
mod torrent;

use std::fs::File;
use std::io::{Read, Write};
use std::thread;

use wxdragon::prelude::*;
use wxdragon::translations::translate;

#[derive(Clone, Copy)]
struct Ui {
    url_input: TextCtrl,
    download_btn: Button,
    gauge: Gauge,
    status: StaticText,
}

fn main() {
    let _ = wxdragon::main(|_| {
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
    let download_btn = Button::builder(&panel).with_label(&translate("Downloa&d")).build();
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
        let result = download(&url, &path, |done, total| report_progress(ui, done, total));
        report_finished(ui, result);
    });
}

fn start_torrent_download(frame: &Frame, ui: Ui, url: String) {
    let Some(dir) = ask_save_dir(frame) else {
        return;
    };
    start_download(ui);
    thread::spawn(move || {
        let result = torrent::download(&url, &dir, |done, total| report_progress(ui, done, total));
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
    let dialog = DirDialog::builder(parent, &translate("Please select destination folder"), "").build();
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

fn report_progress(ui: Ui, done: u64, total: Option<u64>) {
    wxdragon::call_after(Box::new(move || match total {
        Some(t) if t > 0 => {
            ui.gauge.set_value((done * 100 / t) as i32);
            ui.status
                .set_label(&format!("{} / {}", format_size(done), format_size(t)));
        }
        _ => ui.status.set_label(&format_size(done)),
    }));
}

fn report_finished(ui: Ui, result: Result<u64, String>) {
    wxdragon::call_after(Box::new(move || {
        ui.download_btn.enable(true);
        match result {
            Ok(bytes) => {
                ui.gauge.set_value(100);
                ui.status
                    .set_label(&format!(&translate("Done: {}", format_size(bytes)));
            }
            Err(e) => {
                ui.gauge.set_value(0);
                ui.status.set_label(&format!(&translate("Error: {e}")));
            }
        }
    }));
}

fn format_size(bytes: u64) -> String {
    let mb = bytes as f64 / 1024.0 / 1024.0;
    format!("{mb:.2} MB")
}
