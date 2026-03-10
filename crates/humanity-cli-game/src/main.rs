use core_offline_loop::{apply_command, parse_command, Command, WorldSnapshot};
use persistence_sqlite::SqliteStore;
use std::env;
use std::io::{self, Write};

fn try_meta_command(world: &mut WorldSnapshot, line: &str) -> Result<Option<(bool, String)>, String> {
    let mut parts = line.split_whitespace();
    let Some(cmd) = parts.next() else {
        return Ok(None);
    };

    match cmd {
        "save_db" => {
            let db_path = parts.next().ok_or("usage: save_db <db_path> <slot>")?;
            let slot = parts.next().ok_or("usage: save_db <db_path> <slot>")?;
            let store = SqliteStore::open(db_path).map_err(|e| e.to_string())?;
            let id = store
                .save_snapshot(slot, world)
                .map_err(|e| e.to_string())?;
            let _ = store.append_event(slot, world.tick, "snapshot_save", "{}");
            Ok(Some((false, format!("saved snapshot id={id} slot={slot} db={db_path}"))))
        }
        "load_db" => {
            let db_path = parts.next().ok_or("usage: load_db <db_path> <slot>")?;
            let slot = parts.next().ok_or("usage: load_db <db_path> <slot>")?;
            let store = SqliteStore::open(db_path).map_err(|e| e.to_string())?;
            match store
                .load_latest_snapshot(slot)
                .map_err(|e| e.to_string())?
            {
                Some(snapshot) => {
                    *world = snapshot;
                    Ok(Some((false, format!("loaded latest snapshot for slot={slot} from {db_path}"))))
                }
                None => Ok(Some((false, format!("no snapshots for slot={slot}")))),
            }
        }
        "events" => {
            let db_path = parts.next().ok_or("usage: events <db_path> <slot> [limit]")?;
            let slot = parts.next().ok_or("usage: events <db_path> <slot> [limit]")?;
            let limit = parts
                .next()
                .and_then(|x| x.parse::<u32>().ok())
                .unwrap_or(10);
            let store = SqliteStore::open(db_path).map_err(|e| e.to_string())?;
            let rows = store
                .list_recent_events(slot, limit)
                .map_err(|e| e.to_string())?;
            let mut out = String::new();
            out.push_str(&format!("recent events ({})\n", rows.len()));
            for (id, tick, kind, payload) in rows {
                out.push_str(&format!("- id={id} tick={tick} kind={kind} payload={payload}\n"));
            }
            Ok(Some((false, out.trim_end().to_string())))
        }
        "help_db" => Ok(Some((
            false,
            "db commands: save_db <db_path> <slot> | load_db <db_path> <slot> | events <db_path> <slot> [limit]".to_string(),
        ))),
        _ => Ok(None),
    }
}

fn process_line(world: &mut WorldSnapshot, line: &str) -> (bool, String) {
    match try_meta_command(world, line) {
        Ok(Some((quit, out))) => return (quit, out),
        Ok(None) => {}
        Err(e) => return (false, format!("error: {e}")),
    }

    match parse_command(line).and_then(|cmd| apply_command(world, cmd.clone()).map(|out| (cmd, out))) {
        Ok((cmd, out)) => (cmd == Command::Quit, out),
        Err(e) => (false, format!("error: {e}")),
    }
}

fn run_script(world: &mut WorldSnapshot, script: &str) {
    for raw in script.split(';') {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let (quit, out) = process_line(world, line);
        println!("> {line}");
        println!("{out}");
        if quit {
            break;
        }
    }
}

fn run_repl(world: &mut WorldSnapshot) {
    println!("Humanity CLI Game (scaffold) — type 'help' for game commands, 'help_db' for sqlite commands.");
    let stdin = io::stdin();

    loop {
        print!("> ");
        let _ = io::stdout().flush();

        let mut line = String::new();
        if stdin.read_line(&mut line).is_err() {
            println!("input error");
            continue;
        }

        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let (quit, out) = process_line(world, line);
        println!("{out}");
        if quit {
            break;
        }
    }
}

fn main() {
    let mut world = WorldSnapshot::new_default();
    let args: Vec<String> = env::args().collect();

    if args.len() >= 3 && args[1] == "--script" {
        run_script(&mut world, &args[2]);
        return;
    }

    run_repl(&mut world);
}
