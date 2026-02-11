use std::{process, time::Duration};

use jiff::{Span, Unit, Zoned};

const ICON: &[u8] = include_bytes!("../../resources/snek.ico");

struct App {
    game_name: String,
    duration: Span,
    quit_time: Zoned,
    remaining: String,
    mode: iced::theme::Mode,
}

#[derive(Copy, Clone, Debug)]
enum Message {
    Tick,
    SystemThemeChanged(iced::theme::Mode),
}

impl App {
    fn boot() -> (Self, iced::Task<Message>) {
        let args: Vec<String> = std::env::args().collect();
        if args.len() < 3 {
            eprintln!("Usage: spoofer <game_name> <duration_minutes>");
            process::exit(1);
        }
        let game_name = args[1].clone();
        let minutes: i64 = args[2].parse().unwrap_or_else(|_| {
            eprintln!("Invalid duration: {}", args[2]);
            process::exit(1);
        });
        let duration = Span::new().minutes(minutes);
        let quit_time = Zoned::now()
            .round(Unit::Second)
            .expect("Rounding failed")
            .saturating_add(duration)
            .saturating_add(Span::new().seconds(15));

        let remaining = format_remaining(&quit_time);

        (
            Self {
                game_name,
                duration,
                quit_time,
                remaining,
                mode: iced::theme::Mode::None,
            },
            iced::system::theme().map(Message::SystemThemeChanged),
        )
    }

    fn title(&self) -> String {
        format!("Doppelgamer - {} ({})", self.game_name, self.remaining)
    }

    fn update(&mut self, msg: Message) {
        match msg {
            Message::Tick => {
                if Zoned::now() > self.quit_time {
                    process::exit(0);
                }
                self.remaining = format_remaining(&self.quit_time);
            }
            Message::SystemThemeChanged(mode) => {
                self.mode = mode;
            }
        }
    }

    const fn theme(&self) -> iced::Theme {
        match self.mode {
            iced::theme::Mode::Dark => iced::Theme::CatppuccinMocha,
            iced::theme::Mode::Light | iced::theme::Mode::None => iced::Theme::CatppuccinLatte,
        }
    }

    fn view(&self) -> iced::Element<'_, Message> {
        iced::widget::column![
            iced::widget::text("Doppelgamer").size(24),
            iced::widget::text(format!(
                "Playing {} for {:?}",
                self.game_name, self.duration
            )),
            iced::widget::text(format!(
                "Will exit at: {:#}",
                self.quit_time.strftime("%H:%M:%S")
            )),
            iced::widget::text(format!("Time remaining: {}", self.remaining)),
        ]
        .spacing(5)
        .padding(10)
        .into()
    }

    #[allow(clippy::unused_self)]
    fn subscription(&self) -> iced::Subscription<Message> {
        iced::Subscription::batch([
            iced::time::every(Duration::from_secs(1)).map(|_| Message::Tick),
            iced::system::theme_changes().map(Message::SystemThemeChanged),
        ])
    }
}

fn format_remaining(quit_time: &Zoned) -> String {
    let remaining = Zoned::now()
        .until(quit_time)
        .expect("Time went backwards")
        .round(Unit::Second)
        .expect("Rounding failed");
    format!("{remaining:#}")
}

fn main() -> anyhow::Result<()> {
    iced::application(App::boot, App::update, App::view)
        .title(App::title)
        .theme(App::theme)
        .window(iced::window::Settings {
            size: iced::Size::new(320.0, 130.0),
            resizable: false,
            icon: Some(
                iced::window::icon::from_file_data(ICON, None)
                    .expect("embedded icon should be valid"),
            ),
            ..Default::default()
        })
        .subscription(App::subscription)
        .run()?;

    Ok(())
}
