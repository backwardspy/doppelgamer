use std::{process, time::Duration};

use eframe::{
    Frame, NativeOptions,
    egui::{self, Context, IconData, ViewportBuilder},
};
use jiff::{Span, Unit, Zoned};

struct App {
    game_name: String,
    duration: Span,
    quit_time: Zoned,
}

impl eframe::App for App {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        let remaining = Zoned::now()
            .until(&self.quit_time)
            .expect("Time went backwards")
            .round(Unit::Second)
            .expect("Rounding failed");

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Doppelgamer");
            ui.label(format!("Playing: {}", self.game_name));
            ui.label(format!("Duration: {:#}", self.duration));
            ui.label(format!(
                "Will exit at: {:#}",
                self.quit_time.strftime("%H:%M:%S")
            ));
            ui.label(format!("Time remaining: {remaining:#}"));
        });

        if Zoned::now() > self.quit_time {
            process::exit(0);
        } else {
            ctx.request_repaint_after(Duration::from_secs(1));
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: spoofer <game_name> <duration_minutes>");
        process::exit(1);
    }
    let game_name = args[1].clone();
    let duration: i64 = args[2].parse().unwrap_or_else(|_| {
        eprintln!("Invalid duration: {}", args[2]);
        process::exit(1);
    });
    let duration = Span::new().minutes(duration);
    let quit_time = Zoned::now()
        .round(Unit::Second)
        .expect("Rounding failed")
        .saturating_add(duration)
        .saturating_add(Span::new().seconds(15));

    let app = App {
        game_name: game_name.clone(),
        duration,
        quit_time,
    };

    let native_options = NativeOptions {
        viewport: ViewportBuilder::default()
            .with_inner_size([320.0, 160.0])
            .with_resizable(false)
            .with_icon(load_icon()),
        ..Default::default()
    };
    let _ = eframe::run_native(
        &game_name,
        native_options,
        Box::new(|_cc| Ok(Box::new(app))),
    );
}

fn load_icon() -> IconData {
    let img = image::load_from_memory(include_bytes!("../../assets/snek.ico"))
        .expect("ico is valid")
        .into_rgba8();
    let (width, height) = img.dimensions();
    let rgba = img.into_raw();
    IconData {
        rgba,
        width,
        height,
    }
}
