mod finder;
mod insh;
mod searcher;
mod vim;
mod walker;

fn main() {
    let mut insh = insh::Insh::new();
    insh.run();
}
