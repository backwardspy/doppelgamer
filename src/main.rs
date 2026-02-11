#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use std::fs::{self, File};
use std::io::Write as _;
use std::os::windows::process::CommandExt as _;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

use etcetera::AppStrategy as _;
use iced::futures::SinkExt as _;
use iced::futures::StreamExt as _;
use iced::futures::channel::mpsc;
use tracing::{error, info, warn};
use tracing_subscriber::fmt::writer::MakeWriterExt as _;

const ICON: &[u8] = include_bytes!("../resources/snek.ico");
const BUNDLED_GAMES: &str = include_str!("../games.v2.json");
const GAMES_JSON_URL: &str =
    "https://raw.githubusercontent.com/backwardspy/doppelgamer/refs/heads/main/games.v2.json";
const MAX_SHOWN_RESULTS: usize = 100;

#[cfg(debug_assertions)]
const SPOOFER_BIN: &[u8] = include_bytes!("../target/debug/spoofer.exe");
#[cfg(not(debug_assertions))]
const SPOOFER_BIN: &[u8] = include_bytes!("../target/release/spoofer.exe");

#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
struct Game {
    name: String,
    exe: PathBuf,
}

#[derive(Clone, Debug)]
enum MatcherCommand {
    Search(String),
    ReloadGames(Vec<Game>),
}

struct App {
    query: String,
    games: Vec<Game>,
    selected_game: Option<Game>,
    duration: u32,
    matcher_tx: Option<mpsc::Sender<MatcherCommand>>,
    initial_games: Option<Vec<Game>>,
    mode: iced::theme::Mode,
}

#[derive(Clone, Debug)]
enum Message {
    MatcherReady(mpsc::Sender<MatcherCommand>),
    SystemThemeChanged(iced::theme::Mode),
    GamesUpdated(Option<Vec<Game>>),
    Suggest(Vec<Game>),
    Search(String),
    Select(Game),
    SetDuration(u32),
    Launch(Game, u32),
}

fn games_json_path() -> anyhow::Result<PathBuf> {
    let mut path = etc_strategy()?.config_dir();
    fs::create_dir_all(&path)?;
    path.push("games.v2.json");
    Ok(path)
}

fn ensure_local_games_json() {
    info!("Ensuring local games.json exists");
    let Ok(path) = games_json_path() else {
        warn!("Failed to determine games.json path, skipping local cache");
        return;
    };

    if !path.exists()
        && let Ok(mut file) = File::create(&path)
    {
        let _ = file.write_all(BUNDLED_GAMES.as_bytes());
        info!("Wrote bundled games.json to local cache");
    }
}

fn load_games() -> Vec<Game> {
    info!("Loading games from local cache");
    ensure_local_games_json();
    if let Some(path) = games_json_path().ok()
        && let Ok(data) = fs::read_to_string(&path)
        && let Ok(games) = serde_json::from_str(&data)
    {
        return games;
    }
    serde_json::from_str(BUNDLED_GAMES).expect("bundled games json should be valid")
}

async fn fetch_remote_games() -> Option<Vec<Game>> {
    info!("Fetching games from remote");
    let response = match reqwest::get(GAMES_JSON_URL).await {
        Ok(r) => r,
        Err(e) => {
            warn!("Failed to fetch remote games.json: {e}");
            return None;
        }
    };
    if !response.status().is_success() {
        warn!(
            "Failed to fetch remote games.json, status: {}",
            response.status()
        );
        return None;
    }

    info!("Remote games.json fetched, parsing response");
    let games: Vec<Game> = match response.json().await {
        Ok(g) => g,
        Err(e) => {
            warn!("Failed to parse remote games.json: {e}");
            return None;
        }
    };
    info!("Parsed {} games from remote", games.len());

    if let Ok(path) = games_json_path()
        && let Ok(mut file) = File::create(&path)
        && let Err(e) = serde_json::to_writer(&mut file, &games)
    {
        error!("Failed to write fetched games to local cache: {e}");
    }

    Some(games)
}

fn etc_strategy() -> anyhow::Result<impl etcetera::AppStrategy> {
    let strategy = etcetera::choose_app_strategy(etcetera::AppStrategyArgs {
        top_level_domain: "com".to_string(),
        author: "backwardspy".to_string(),
        app_name: "doppelgamer".to_string(),
    })?;
    Ok(strategy)
}

fn game_exe_path(game: &Game) -> anyhow::Result<PathBuf> {
    let mut path = etc_strategy()?.data_dir();
    path.push(&game.exe);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    info!("Using spoofer path: {}", path.display());
    Ok(path)
}

fn launch_spoofer(game: Game, duration: u32) {
    info!(
        "Launching spoofer for {} with duration {} minutes",
        game.name, duration
    );

    tokio::task::spawn_blocking(move || {
        let Ok(exe_path) = game_exe_path(&game) else {
            error!("Failed to determine spoofer path, aborting launch");
            return;
        };
        let Ok(()) = fs::write(&exe_path, SPOOFER_BIN) else {
            error!("Failed to write spoofer binary to disk, aborting launch");
            return;
        };

        #[allow(clippy::zombie_processes)]
        let Ok(_) = Command::new(&exe_path)
            .arg(&game.name)
            .arg(duration.to_string())
            .creation_flags(0x0000_0008) // CREATE_NEW_CONSOLE
            .spawn()
        else {
            error!("Failed to launch spoofer process, aborting launch");
            return;
        };
    });
}

impl App {
    fn boot() -> (Self, iced::Task<Message>) {
        let use_local = std::env::args().any(|arg| arg == "--local");
        let games = load_games();
        let app = Self {
            query: String::new(),
            games: games.iter().take(MAX_SHOWN_RESULTS).cloned().collect(),
            selected_game: None,
            duration: 15,
            matcher_tx: None,
            initial_games: Some(games),
            mode: iced::theme::Mode::None,
        };
        let mut tasks = vec![iced::system::theme().map(Message::SystemThemeChanged)];
        if use_local {
            info!("--local flag set, skipping remote fetch");
        } else {
            tasks.push(iced::Task::perform(
                fetch_remote_games(),
                Message::GamesUpdated,
            ));
        }
        (app, iced::Task::batch(tasks))
    }

    fn update(&mut self, msg: Message) {
        match msg {
            Message::MatcherReady(tx) => {
                self.matcher_tx = Some(tx);
                if let Some(games) = self.initial_games.take()
                    && let Some(tx) = &mut self.matcher_tx
                {
                    let _ = tx.try_send(MatcherCommand::ReloadGames(games));
                }
            }
            Message::SystemThemeChanged(mode) => {
                self.mode = mode;
            }
            Message::GamesUpdated(Some(games)) => {
                self.games = games.iter().take(MAX_SHOWN_RESULTS).cloned().collect();
                if let Some(tx) = &mut self.matcher_tx {
                    let _ = tx.try_send(MatcherCommand::ReloadGames(games));
                }
            }
            Message::GamesUpdated(None) => {}
            Message::Search(query) => {
                self.query = query;
                if let Some(tx) = &mut self.matcher_tx {
                    let _ = tx.try_send(MatcherCommand::Search(self.query.clone()));
                }
            }
            Message::Suggest(results) => {
                self.games = results.into_iter().take(MAX_SHOWN_RESULTS).collect();
            }
            Message::Select(game) => {
                self.selected_game = Some(game);
            }
            Message::SetDuration(duration) => {
                self.duration = duration;
            }
            Message::Launch(game, duration) => {
                launch_spoofer(game, duration);
                self.selected_game = None;
                self.query.clear();
                if let Some(tx) = &mut self.matcher_tx {
                    let _ = tx.try_send(MatcherCommand::Search(String::new()));
                }
            }
        }
    }

    fn view(&self) -> iced::Element<'_, Message> {
        let top: iced::Element<'_, Message> = self.selected_game.as_ref().map_or_else(
            || iced::widget::text("Select a game below to get started").into(),
            |game| {
                iced::widget::row![
                    iced::widget::text(format!("Launch {} for", &game.name)),
                    iced_aw::widget::number_input(&self.duration, 1..=60, Message::SetDuration),
                    iced::widget::text("minutes?"),
                    iced::widget::space::horizontal(),
                    iced::widget::button("Make it so!")
                        .style(iced::widget::button::primary)
                        .on_press(Message::Launch(game.clone(), self.duration)),
                ]
                .spacing(5)
                .align_y(iced::Center)
                .into()
            },
        );

        iced::widget::column![
            top,
            iced::widget::text_input("Search for a game...", &self.query).on_input(Message::Search),
            iced::widget::scrollable(
                iced::widget::column![
                    iced::widget::column(self.games.iter().map(|game| {
                        iced::widget::button(
                            iced::widget::row![
                                iced::widget::text(&game.name),
                                iced::widget::text(format!("({})", game.exe.to_string_lossy()))
                                    .wrapping(iced::widget::text::Wrapping::None)
                                    .size(12)
                            ]
                            .spacing(5)
                            .align_y(iced::Alignment::Center),
                        )
                        .width(iced::Fill)
                        .padding(0)
                        .style(iced::widget::button::text)
                        .on_press(Message::Select(game.clone()))
                        .into()
                    }))
                    .spacing(5),
                    iced::widget::rule::horizontal(1),
                    iced::widget::text(format!(
                        "Showing up to {MAX_SHOWN_RESULTS} results, use the search bar to filter!"
                    ))
                ]
                .spacing(10)
            )
            .width(iced::Fill),
        ]
        .spacing(10)
        .padding(10)
        .into()
    }

    const fn theme(&self) -> iced::Theme {
        match self.mode {
            iced::theme::Mode::Dark => iced::Theme::CatppuccinMocha,
            iced::theme::Mode::Light | iced::theme::Mode::None => iced::Theme::CatppuccinLatte,
        }
    }

    #[allow(clippy::unused_self)]
    fn subscription(&self) -> iced::Subscription<Message> {
        iced::Subscription::batch([
            iced::Subscription::run(Self::run_matcher),
            iced::system::theme_changes().map(Message::SystemThemeChanged),
        ])
    }

    fn run_matcher() -> impl iced::futures::Stream<Item = Message> {
        iced::stream::channel(100, async move |mut output| {
            let (tx, mut rx) = mpsc::channel(100);

            output
                .send(Message::MatcherReady(tx))
                .await
                .expect("main loop should be alive");

            let mut matcher =
                nucleo::Nucleo::new(nucleo::Config::DEFAULT, Arc::new(|| {}), None, 2);

            let inject_games = |matcher: &nucleo::Nucleo<Game>, games: &[Game]| {
                let injector = matcher.injector();
                for game in games {
                    injector.push(game.clone(), |game, cols| {
                        cols[0] = game.name.as_str().into();
                        cols[1] = game.exe.to_string_lossy().into();
                    });
                }
            };

            let mut last_query = String::new();

            loop {
                let Some(cmd) = rx.next().await else {
                    break;
                };

                // drain pending commands, keeping the latest search and latest reload
                let mut reload_games = match &cmd {
                    MatcherCommand::ReloadGames(games) => Some(games.clone()),
                    MatcherCommand::Search(_) => None,
                };
                let mut latest_search = match cmd {
                    MatcherCommand::Search(query) => Some(query),
                    MatcherCommand::ReloadGames(_) => None,
                };
                while let Ok(Some(newer)) = rx.try_next() {
                    match newer {
                        MatcherCommand::ReloadGames(games) => reload_games = Some(games),
                        MatcherCommand::Search(query) => latest_search = Some(query),
                    }
                }

                if let Some(games) = reload_games {
                    matcher =
                        nucleo::Nucleo::new(nucleo::Config::DEFAULT, Arc::new(|| {}), None, 2);
                    inject_games(&matcher, &games);
                    matcher.tick(10);
                }

                if let Some(query) = latest_search {
                    last_query = query;
                }

                matcher.pattern.reparse(
                    0,
                    &last_query,
                    nucleo::pattern::CaseMatching::Smart,
                    nucleo::pattern::Normalization::Smart,
                    false,
                );

                loop {
                    let status = matcher.tick(10);

                    if status.changed {
                        let snapshot = matcher.snapshot();
                        let count = snapshot.matched_item_count();
                        let results: Vec<Game> = snapshot
                            .matched_items(0..count)
                            .map(|item| item.data.clone())
                            .collect();

                        let _ = output.send(Message::Suggest(results)).await;
                    }

                    if !status.running {
                        break;
                    }
                }
            }
        })
    }
}

fn cleanup_old_logs(log_dir: &std::path::Path) {
    let Ok(entries) = fs::read_dir(log_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !name.starts_with("fuzz.log.") {
            continue;
        }
        if let Ok(meta) = path.metadata()
            && let Ok(modified) = meta.modified()
            && let Ok(age) = modified.elapsed()
            && age > std::time::Duration::from_secs(7 * 24 * 60 * 60)
        {
            let _ = fs::remove_file(&path);
        }
    }
}

fn main() -> anyhow::Result<()> {
    let log_dir = etc_strategy()?.data_dir();
    fs::create_dir_all(&log_dir)?;
    cleanup_old_logs(&log_dir);
    let file_appender = tracing_appender::rolling::daily(&log_dir, "fuzz.log");
    tracing_subscriber::fmt()
        .with_ansi(false)
        .with_writer(std::io::stderr.and(file_appender))
        .init();

    iced::application(App::boot, App::update, App::view)
        .window(iced::window::Settings {
            min_size: Some(iced::Size::new(640.0, 640.0).ratio(1.6)),
            icon: Some(
                iced::window::icon::from_file_data(ICON, None)
                    .expect("embedded icon should be valid"),
            ),
            ..Default::default()
        })
        .title("Doppelgamer - Launcher")
        .font(iced_aw::ICED_AW_FONT_BYTES)
        .theme(App::theme)
        .subscription(App::subscription)
        .run()?;

    Ok(())
}
