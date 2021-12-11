mod action;
mod bash_shell;
mod color;
mod effect;
mod finder;
mod insh;
mod searcher;
mod state;
mod terminal_size;
mod vim;
mod walker;

fn main() {
    let mut insh = insh::Insh::new();
    insh.run();
}
