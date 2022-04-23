mod app;
mod color;
mod component;
mod components;
mod path_finder;
mod phrase_searcher;
mod program;
mod programs;
mod rendering;
mod stateful;
mod system_effect;

use app::App;
use component::Component;
use components::{Insh, InshProps};
use stateful::Stateful;

fn main() {
    let mut app: App = App::new();
    let mut root = Insh::new(InshProps {});

    app.run(&mut root);
}
