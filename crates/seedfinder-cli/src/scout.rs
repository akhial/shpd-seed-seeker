//! Human-readable scouting for seed codes and UTC daily runs.

use std::fmt::Write as _;
use std::io::Write as _;
use std::path::Path;

use shpd_seedfinder_core::catalog::item;
use shpd_seedfinder_core::challenges::Challenges;
use shpd_seedfinder_core::main_world::generate_main_world_with_trinket;
use shpd_seedfinder_core::seed::DungeonSeed;

pub fn parse_seed(value: Option<&str>, daily: bool) -> Result<DungeonSeed, String> {
    if let Some(value) = value {
        if daily {
            DungeonSeed::from_daily_date(value)
        } else {
            DungeonSeed::from_code(value)
        }
        .map_err(|error| error.to_string())
    } else if daily {
        let seconds = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|error| error.to_string())?
            .as_secs();
        DungeonSeed::daily_at_unix_seconds(seconds).map_err(|error| error.to_string())
    } else {
        Err("--scout requires a XXX-XXX-XXX seed code".to_owned())
    }
}

pub fn run(seed: DungeonSeed, items: Option<&Path>, output: Option<&Path>) -> Result<(), String> {
    let query = items.map(super::load_query).transpose()?;
    if let (Some(input), Some(output)) = (items, output) {
        if std::fs::canonicalize(input).ok() == std::fs::canonicalize(output).ok() {
            return Err("--output must be different from the --items file".to_owned());
        }
    }
    let challenges = query.as_ref().map_or(Challenges::NONE, |q| q.challenges);
    let selected = query
        .as_ref()
        .and_then(|q| shpd_seedfinder_core::trinkets::selected_for_query(seed, q));
    let world = generate_main_world_with_trinket(seed, 24, challenges, selected)
        .map_err(|error| error.to_string())?;
    let mut text = format!(
        "{} {seed}\nShattered Pixel Dungeon {}\n",
        if seed.is_daily() {
            "Daily run (UTC)"
        } else {
            "Seed"
        },
        shpd_seedfinder_core::SHPD_VERSION
    );
    if seed.is_daily() {
        text.push_str("Daily dates use this game version, including past and future dates.\n");
    }
    writeln!(
        text,
        "Trinket: {}",
        selected.map_or("None", |id| item(id).name)
    )
    .unwrap();
    if let Some(query) = &query {
        let matches = shpd_seedfinder_core::query::scout_matches(&world, query);
        writeln!(
            text,
            "{} of {} requirements matched",
            matches.matched_requirements, matches.total_requirements
        )
        .unwrap();
    }
    for depth in 1..=24 {
        let floor: Vec<_> = world
            .items
            .iter()
            .filter(|entry| entry.depth == depth)
            .collect();
        if floor.is_empty() {
            continue;
        }
        writeln!(text, "\nFloor {depth}").unwrap();
        for entry in floor {
            write!(
                text,
                "  {} +{}",
                item(entry.item).name,
                entry.displayed_upgrade()
            )
            .unwrap();
            if let Some(effect) = entry.effect {
                write!(text, " {}", effect.wire_name()).unwrap();
            }
            if entry.cursed {
                text.push_str(" [cursed]");
            }
            if entry.secret {
                text.push_str(" [secret]");
            }
            write!(text, " — {:?}", entry.source).unwrap();
            if entry.accessibility != shpd_seedfinder_core::model::Accessibility::Independent {
                write!(text, " ({:?})", entry.accessibility).unwrap();
            }
            text.push('\n');
        }
    }
    if let Some(output) = output {
        std::fs::write(output, text).map_err(|error| error.to_string())
    } else {
        std::io::stdout()
            .lock()
            .write_all(text.as_bytes())
            .map_err(|error| error.to_string())
    }
}
