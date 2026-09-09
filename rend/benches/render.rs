/*!
Benchmarks of drawing fabrics on a terminal.

Each scenario is a handful of fabrics which are drawn one after another, over and over, so that
what is measured is what it costs to draw a screen given the one which came before it rather than
what it costs to draw the first one. The throughput is set to how many bytes a frame of the
scenario is sent as, so that the size of a frame is recorded alongside how long it takes.

Every scenario is drawn twice over: once onto a pseudoterminal, which is what says how much the
bytes cost, and once nowhere at all, which is what says how much working out what to send costs.
*/
use std::fs::File;
use std::io::{self, LineWriter, Read, Sink};
use std::thread::{self, JoinHandle};

use ansi::Color;
use rend::{Engine, Fabric, Renderer, Size, Yarn};

use criterion::{criterion_group, criterion_main, BatchSize, Criterion, Throughput};
use nix::pty::openpty;
use nix::sys::termios::{cfmakeraw, tcgetattr, tcsetattr, SetArg};

/// The engines which each scenario is drawn with, and what each one is called.
const ENGINES: [(&str, Engine); 2] = [("full", Engine::Full), ("incremental", Engine::Incremental)];

/// A pseudoterminal to draw on, with something on the other end of it reading everything which is
/// written the way a terminal emulator does.
///
/// Drawing on one of these is what makes the bytes a frame is sent as cost anything, which drawing
/// nowhere at all does not.
struct Pty {
    /// The end of it which is drawn on.
    terminal: Option<File>,
    /// What is reading the other end of it.
    reader: Option<JoinHandle<()>>,
}

impl Pty {
    /// Return a pseudoterminal which is being read.
    fn new() -> Self {
        let pty = openpty(None, None).unwrap();

        // NOTE: The line discipline is put in raw mode so that what is written is what comes out
        // the other end rather than what it has made of the control characters in it.
        let terminal = File::from(pty.slave);
        let mut attributes = tcgetattr(&terminal).unwrap();
        cfmakeraw(&mut attributes);
        tcsetattr(&terminal, SetArg::TCSANOW, &attributes).unwrap();

        let mut read_from = File::from(pty.master);
        let reader = thread::spawn(move || {
            let mut buffer: [u8; 4096] = [0; 4096];
            while matches!(read_from.read(&mut buffer), Ok(read) if read > 0) {}
        });

        Pty {
            terminal: Some(terminal),
            reader: Some(reader),
        }
    }

    /// Return the end of it which is drawn on.
    fn terminal(&self) -> File {
        self.terminal.as_ref().unwrap().try_clone().unwrap()
    }
}

impl Drop for Pty {
    fn drop(&mut self) {
        // Whatever is reading it stops once there is nothing left which could be written to it.
        self.terminal.take();
        self.reader.take().unwrap().join().unwrap();
    }
}

/// The sizes of terminal which each scenario is drawn at.
fn sizes() -> [(&'static str, Size); 2] {
    [("24x80", Size::new(24, 80)), ("60x200", Size::new(60, 200))]
}

/// Return the rows of a directory listing, numbered from the given one so that two listings can be
/// made to differ.
fn rows(size: Size, first: usize) -> Vec<String> {
    (0..size.rows)
        .map(|row| format!("{:>6}  file_{:04}.rs", first + row, first + row))
        .collect()
}

/// Return the fabric which the rows are drawn as, each of them as wide as the screen.
fn fabric(rows: Vec<String>, size: Size) -> Fabric {
    let yarns: Vec<Yarn> = rows
        .iter()
        .map(|row| {
            let mut yarn = Yarn::from(row.as_str());
            yarn.resize(size.columns);
            yarn
        })
        .collect();
    Fabric::from(yarns)
}

/// Return the fabric which the rows are drawn as with the file names colored and the given row
/// selected, the way the browser draws a directory.
fn colored_fabric(rows: Vec<String>, size: Size, selected: usize) -> Fabric {
    let yarns: Vec<Yarn> = rows
        .iter()
        .enumerate()
        .map(|(number, row)| {
            let mut yarn = Yarn::from(row.as_str());
            yarn.resize(size.columns);
            yarn.color_after(Color::BrightBlue, 8);
            if number == selected {
                yarn.background(Color::BrightBlack);
            }
            yarn
        })
        .collect();
    Fabric::from(yarns)
}

/// Return what is drawn at the given size, and what each of them is called.
fn scenarios(size: Size) -> Vec<(&'static str, Vec<Fabric>)> {
    let screen: Fabric = fabric(rows(size, 0), size);

    let mut one_cell: Vec<String> = rows(size, 0);
    one_cell[size.rows / 2].replace_range(2..3, "X");

    let mut one_row: Vec<String> = rows(size, 0);
    one_row[size.rows / 2] = "        a row which is not like the others".to_string();

    vec![
        // A screen with nothing on it in common with the one before it.
        (
            "whole screen",
            vec![screen.clone(), fabric(rows(size, size.rows), size)],
        ),
        // The same screen over again, which is what an event that changes nothing costs.
        ("nothing changed", vec![screen.clone()]),
        // One column of one row different.
        ("one cell", vec![screen.clone(), fabric(one_cell, size)]),
        // One whole row different.
        ("one row", vec![screen.clone(), fabric(one_row, size)]),
        // Everything moved up a row, which is what scrolling a directory looks like.
        (
            "scrolled",
            vec![screen.clone(), fabric(rows(size, 1), size)],
        ),
        // The selected row moved down one, on a screen with colors on it.
        (
            "selection moved",
            vec![
                colored_fabric(rows(size, 0), size, size.rows / 2),
                colored_fabric(rows(size, 0), size, size.rows / 2 + 1),
            ],
        ),
    ]
}

/// Return how many bytes a frame of the scenario is sent as when it is drawn with the engine.
fn bytes(engine: Engine, fabrics: &[Fabric]) -> u64 {
    let mut renderer: Renderer<Vec<u8>> = Renderer::builder()
        .writer(Vec::new())
        .engine(engine)
        .build();

    // Draw them once so that what is counted is what a frame costs in the steady state rather than
    // what the first one costs, which has nothing on the screen to go on.
    for fabric in fabrics.iter().cloned() {
        renderer.render(fabric);
    }
    let before: usize = renderer.writer().len();

    for fabric in fabrics.iter().cloned() {
        renderer.render(fabric);
    }

    ((renderer.writer().len() - before) / fabrics.len()) as u64
}

/// Draw the fabrics one after another, over and over, with each of the engines.
fn bench(criterion: &mut Criterion, pty: &Pty, name: String, fabrics: &[Fabric]) {
    let mut group = criterion.benchmark_group(name.clone());
    for (engine_name, engine) in ENGINES {
        group.throughput(Throughput::Bytes(bytes(engine, fabrics)));
        group.bench_function(engine_name, |bencher| {
            // NOTE: What is written goes through the same sort of buffer as the standard output
            // of the app does, so that what is measured is the writes a frame really costs rather
            // than one for every character of it.
            let mut renderer: Renderer<LineWriter<File>> = Renderer::builder()
                .writer(LineWriter::new(pty.terminal()))
                .engine(engine)
                .build();
            let mut drawn: usize = 0;

            bencher.iter_batched(
                || {
                    let fabric = fabrics[drawn % fabrics.len()].clone();
                    drawn += 1;
                    fabric
                },
                |fabric| renderer.render(fabric),
                // NOTE: One fabric is made at a time rather than a batch of them at once, because
                // a screenful is a big enough thing that a batch of them does not fit in the
                // caches and what would be measured is waiting for memory.
                BatchSize::PerIteration,
            );
        });
    }
    group.finish();

    let mut group = criterion.benchmark_group(format!("{} drawn nowhere", name));
    for (engine_name, engine) in ENGINES {
        group.throughput(Throughput::Bytes(bytes(engine, fabrics)));
        group.bench_function(engine_name, |bencher| {
            let mut renderer: Renderer<Sink> = Renderer::builder()
                .writer(io::sink())
                .engine(engine)
                .build();
            let mut drawn: usize = 0;

            bencher.iter_batched(
                || {
                    let fabric = fabrics[drawn % fabrics.len()].clone();
                    drawn += 1;
                    fabric
                },
                |fabric| renderer.render(fabric),
                BatchSize::PerIteration,
            );
        });
    }
    group.finish();
}

/// Draw every scenario at every size.
fn render(criterion: &mut Criterion) {
    let pty = Pty::new();

    for (size_name, size) in sizes() {
        for (name, fabrics) in scenarios(size) {
            bench(criterion, &pty, format!("{} {}", name, size_name), &fabrics);
        }
    }
}

criterion_group!(benches, render);
criterion_main!(benches);
