mod app;
mod color;
mod component;
mod components;
mod path_finder;
mod program;
mod programs;
mod rendering;
mod stateful;
mod system_effect;
mod walker;

use app::App;
use component::Component;
use components::{Insh, InshProps};

fn main() {
    let mut app: App = App::new();
    let mut root = Insh::new(InshProps {});

    app.run(&mut root);
}
