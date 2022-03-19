use crate::component::Component;

use crate::program::{Program, ProgramCleanup, ProgramSetup};
use crate::rendering::{Fabric, Renderer, Size};
use crate::system_effect::SystemEffect;

use std::io::{self, Stdout, Write};
use std::panic;
use std::process::{Child, Command};

use crossterm::cursor::{Hide as HideCursor, MoveTo as MoveCursorTo, Show as ShowCursor};
use crossterm::event::{read, Event as CrosstermEvent};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::terminal::{Clear as ClearTerminal, ClearType as TerminalClearType};
use crossterm::QueueableCommand;

pub struct App {
    stdout: Stdout,
    renderer: Renderer,
}

impl App {
    pub fn new() -> Self {
        let stdout = io::stdout();
        let renderer = Renderer::new();
        App { stdout, renderer }
    }

    pub fn run<Props>(&mut self, root: &mut impl Component<Props, CrosstermEvent, SystemEffect>) {
        self.set_up();

        loop {
            let size: Size = Size::from(terminal::size().unwrap());
            let fabric: Fabric = root.render(size);

            self.renderer.render(fabric);

            let event: CrosstermEvent = read().unwrap();

            let effect: Option<SystemEffect> = root.handle(event);
            match effect {
                Some(SystemEffect::Exit) => {
                    break;
                }
                Some(SystemEffect::RunProgram { program }) => {
                    self.run_program(program);
                }
                None => {}
            }
        }

        self.teardown();
    }

    fn run_program(&mut self, program: Box<dyn Program>) {
        let setup: ProgramSetup = program.setup();
        if setup.raw_terminal == Some(true) {
            self.disable_raw_terminal();
        }
        if setup.clear_screen {
            self.lazy_clear_screen();
        }
        if setup.cursor_home {
            self.lazy_move_cursor_home();
        }
        if setup.cursor_visible == Some(true) {
            self.lazy_show_cursor();
        }
        if setup.any() {
            self.update_terminal();
        }

        let mut command: Command = (*program).run();
        let mut child: Child = command.spawn().unwrap();

        child.wait();

        let cleanup: ProgramCleanup = program.cleanup();
        if cleanup.hide_cursor {
            self.lazy_hide_cursor();
        }
        if cleanup.enable_raw_terminal {
            self.enable_raw_terminal();
        }
        if cleanup.any() {
            self.update_terminal();
        }
    }

    fn set_up(&mut self) {
        self.lazy_enable_alternate_terminal();
        self.enable_raw_terminal();
        self.lazy_hide_cursor();

        self.change_panic_hook();
    }

    fn teardown(&mut self) {
        self.lazy_disable_alternate_terminal();
        self.disable_raw_terminal();
        self.lazy_show_cursor();
    }

    fn lazy_enable_alternate_terminal(&mut self) {
        self.stdout.queue(EnterAlternateScreen).unwrap();
    }

    fn lazy_disable_alternate_terminal(&mut self) {
        self.stdout.queue(LeaveAlternateScreen).unwrap();
    }

    fn enable_raw_terminal(&mut self) {
        terminal::enable_raw_mode().unwrap();
    }

    fn disable_raw_terminal(&mut self) {
        terminal::disable_raw_mode().unwrap();
    }

    fn lazy_clear_screen(&mut self) {
        self.stdout
            .queue(ClearTerminal(TerminalClearType::All))
            .unwrap();
    }

    fn lazy_hide_cursor(&mut self) {
        self.stdout.queue(HideCursor).unwrap();
    }

    fn lazy_show_cursor(&mut self) {
        self.stdout.queue(ShowCursor).unwrap();
    }

    fn lazy_move_cursor_home(&mut self) {
        self.stdout.queue(MoveCursorTo(0, 0)).unwrap();
    }

    fn update_terminal(&mut self) {
        self.stdout.flush().unwrap();
    }

    fn change_panic_hook(&mut self) {
        let hook_before = panic::take_hook();
        panic::set_hook(Box::new(move |info| {
            let mut stdout = io::stdout();
            stdout.queue(LeaveAlternateScreen).unwrap();
            stdout.queue(ShowCursor).unwrap();
            stdout.flush().unwrap();
            terminal::disable_raw_mode().unwrap();
            hook_before(info);
        }));
    }
}
