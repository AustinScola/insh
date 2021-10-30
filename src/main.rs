mod finder;
mod insh;
mod vim;

fn main() {
    let mut insh = insh::Insh::new();
    insh.run();
}
