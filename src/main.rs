use std::{
    env,
    fs::{self, File},
    io::{self, BufReader, Write as _},
    os::windows::process::CommandExt as _,
    path::PathBuf,
    process::Command,
};

use anyhow::Context as _;
use etcetera::AppStrategy as _;

#[cfg(debug_assertions)]
const SPOOFER_BIN: &[u8] = include_bytes!("../target/debug/spoofer.exe");
#[cfg(not(debug_assertions))]
const SPOOFER_BIN: &[u8] = include_bytes!("../target/release/spoofer.exe");

const GAMES_JSON_URL: &str =
    "https://raw.githubusercontent.com/backwardspy/doppelgamer/refs/heads/main/games.json";
const BUNDLED_GAMES: &str = include_str!("../games.json");

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct Game {
    display_name: String,
    path: PathBuf,
    exe_name: String,
}

fn strategy() -> anyhow::Result<impl etcetera::AppStrategy> {
    let strategy = etcetera::choose_app_strategy(etcetera::AppStrategyArgs {
        top_level_domain: "com".to_string(),
        author: "backwardspy".to_string(),
        app_name: "doppelgamer".to_string(),
    })?;
    Ok(strategy)
}

fn games_json_path() -> anyhow::Result<PathBuf> {
    let mut path = strategy()?.config_dir();
    fs::create_dir_all(&path)?;
    path.push("games.json");
    Ok(path)
}

fn game_exe_path(game: &Game) -> anyhow::Result<PathBuf> {
    let mut path = strategy()?.data_dir();
    path.push(&game.path);
    fs::create_dir_all(&path)?;
    path.push(&game.exe_name);
    Ok(path)
}

fn get_last_known_games(use_local: bool) -> anyhow::Result<Vec<Game>> {
    let path = games_json_path()?;
    path.parent().map(fs::create_dir_all).transpose()?;
    if !use_local && path.exists() {
        println!("Using existing games.json at {}", path.display());
    } else {
        let mut file = File::create(&path)?;
        file.write_all(BUNDLED_GAMES.as_bytes())?;
        println!("Created initial games.json at {}", path.display());
    }
    let file = File::open(&path)?;
    let reader = BufReader::new(file);
    let games: Vec<Game> = serde_json::from_reader(reader)?;
    Ok(games)
}

fn get_games(use_local: bool) -> anyhow::Result<Vec<Game>> {
    let path = games_json_path()?;

    println!("Fetching games list...");
    if !use_local
        && let Ok(response) = reqwest::blocking::get(GAMES_JSON_URL)
        && response.status().is_success()
    {
        println!(
            "Games list fetched successfully, updating local copy in {}",
            path.display()
        );
        let games: Vec<Game> = response.json()?;
        let mut file =
            File::create(&path).context(format!("Failed to create {}", path.display()))?;
        serde_json::to_writer_pretty(&mut file, &games)?;
        println!("Games list saved to {}", path.display());
        return Ok(games);
    }

    println!("Failed to fetch games list, using last known games.");
    get_last_known_games(use_local)
}

fn main() -> anyhow::Result<()> {
    ctrlc::set_handler(|| {
        println!("\nBye!");
        std::process::exit(0);
    })?;

    let use_local = env::args().any(|arg| arg == "--local");
    let mut games = get_games(use_local)?;
    games.sort_by(|a, b| a.display_name.cmp(&b.display_name));

    println!("Available games:\n-----");
    for (i, game) in games.iter().enumerate() {
        println!("{}: {}", i + 1, game.display_name);
    }
    println!("\nCtrl+C to exit without selecting.\n");

    let selected_game = loop {
        print!("Enter the number of the game: ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        match input.trim().parse::<usize>() {
            Ok(num) if num > 0 && num <= games.len() => {
                break games[num - 1].clone();
            }
            _ => println!("Invalid selection, try again."),
        }
    };

    let duration = loop {
        print!("Enter duration in minutes: ");
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        match input.trim().parse::<u64>() {
            Ok(mins) if mins > 0 => break mins,
            _ => println!("Invalid duration, try again."),
        }
    };

    println!(
        "Selected game: {}\nDuration: {} minutes",
        selected_game.display_name, duration
    );

    let exe_path = game_exe_path(&selected_game)?;
    fs::write(&exe_path, SPOOFER_BIN).expect("Failed to write spoofer binary");

    let quit_time = jiff::Zoned::now()
        .saturating_add(jiff::Span::new().minutes(i64::try_from(duration).unwrap_or(i64::MAX)));
    println!(
        "Spoofer will quit at: {}",
        quit_time.strftime("%Y-%m-%d %H:%M:%S")
    );

    Command::new(&exe_path)
        .arg(&selected_game.display_name)
        .arg(duration.to_string())
        .creation_flags(0x0000_0008) // CREATE_NEW_CONSOLE
        .spawn()?;

    println!("Spoofer launched as a detached process. Exiting main program.");

    Ok(())
}
