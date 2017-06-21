extern crate shared;

use std::io::{self, Read, Write};

use shared::{HEAD, State, TAIL};

fn main() {
    run().unwrap();
}

fn run() -> io::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let stderr = io::stderr();

    let mut stdin = stdin.lock();
    let mut stdout = stdout.lock();
    let mut stderr = stderr.lock();

    let mut byte = [0];
    let mut input = [0; 11];
    let mut previous_snapshot = None;
    loop {
        // Synchronize frame
        loop {
            stdin.read_exact(&mut byte)?;
            write!(stderr, "H? {:?} - ", byte)?;
            if byte == [HEAD] {
                writeln!(stderr, "OK")?;
                break;
            }
            writeln!(stderr, "NOPE")?;
        }

        stdin.read_exact(&mut input)?;
        writeln!(stderr, "B: {:?}", input)?;

        stdin.read_exact(&mut byte)?;
        write!(stderr, "T? {:?} - ", byte)?;

        if byte != [TAIL] {
            writeln!(stderr, "NOPE")?;
            continue;
        }
        writeln!(stderr, "OK")?;

        let state = State::deserialize(&input);
        let sleep = state.sleep_cycles as f64;
        if let Some(previous) = previous_snapshot {
            let elapsed = state.snapshot.wrapping_sub(previous);
            let cpu = 100. * (1. - sleep / elapsed as f64);

            write!(
                stdout,
                "CPU: {:.2}% - ",
                cpu,
            )?;
        }
        previous_snapshot = Some(state.snapshot);

        writeln!(
            stdout,
            "CS: {}, F: {}",
            state.context_switches,
            state.frames,
        )?;
    }
}
