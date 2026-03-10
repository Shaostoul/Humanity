use core_offline_loop::{apply_command, parse_command, Command, WorldSnapshot};
use std::env;
use std::io::{self, Write};

fn run_script(world: &mut WorldSnapshot, script: &str) {
    for raw in script.split(';') {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        match parse_command(line).and_then(|cmd| apply_command(world, cmd.clone()).map(|out| (cmd, out))) {
            Ok((cmd, out)) => {
                println!("> {line}");
                println!("{out}");
                if cmd == Command::Quit {
                    break;
                }
            }
            Err(e) => {
                println!("> {line}");
                println!("error: {e}");
            }
        }
    }
}

fn run_repl(world: &mut WorldSnapshot) {
    println!("Humanity CLI Game (scaffold) — type 'help' for commands.");
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

        match parse_command(line).and_then(|cmd| apply_command(world, cmd.clone()).map(|out| (cmd, out))) {
            Ok((cmd, out)) => {
                println!("{out}");
                if cmd == Command::Quit {
                    break;
                }
            }
            Err(e) => println!("error: {e}"),
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
