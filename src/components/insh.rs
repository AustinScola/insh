use super::browser::{Browser, BrowserEffect};
use super::finder::FinderEffect;
use super::searcher::SearcherEffect;

use crate::component::Component;
use crate::programs::{Bash, Vim};
use crate::rendering::{Fabric, Size};
use crate::stateful::Stateful;
use crate::system_effect::SystemEffect;

use std::path::PathBuf;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

mod props {
    use super::Problem;
    use crate::args::{Args, Command};
    use crate::config::Config;
    use crate::current_dir;

    use std::path::PathBuf;

    pub struct Props {
        directory: Option<PathBuf>,
        start: Start,
        problem: Option<Problem>,
        config: Config,
    }

    impl From<(Args, Config)> for Props {
        fn from(args_and_config: (Args, Config)) -> Self {
            let (args, config) = args_and_config;

            let mut directory: Option<PathBuf> =
                args.directory().as_ref().map(|path| path.to_path_buf());

            // If the directory was not passed as an argument, and we are editing a file and then
            // browsing, then the directory should be the directory of the file (if a file was
            // passed).
            if directory.is_none() {
                if let Some(Command::Edit {
                    browse,
                    file_line_column,
                }) = args.command()
                {
                    if *browse {
                        if let Some(file_line_column) = file_line_column {
                            if let Some(file) = file_line_column.file() {
                                directory = match file.parent() {
                                    Some(parent) => Some(parent.to_path_buf()),
                                    None => Some(PathBuf::from("/")),
                                }
                            }
                        }
                    }
                }
            }

            // If the directory is relative, make it absolute.
            if let Some(directory_) = &directory {
                if directory_.is_relative() {
                    let mut absolute_directory = current_dir::current_dir();
                    absolute_directory.push(directory_);
                    directory = Some(absolute_directory);
                }
            }

            let mut problem: Option<Problem> = None;
            if let Some(directory_) = &directory {
                if directory_.is_file() {
                    #[cfg(feature = "logging")]
                    log::debug!("The directory {:?} is a file.", directory_);
                    problem = Some(Problem::DirectoryIsAFile);
                }
            }

            Self {
                directory,
                start: Start::from(args.command().clone()),
                problem,
                config,
            }
        }
    }

    impl Props {
        /// Return the directory to start in.
        pub fn directory(&self) -> &Option<PathBuf> {
            &self.directory
        }

        /// Return the problem if there is one.
        pub fn problem(&self) -> Option<Problem> {
            self.problem.clone()
        }

        /// Return the component to start running.
        pub fn start(&self) -> &Start {
            &self.start
        }

        /// Return the configuration settings.
        pub fn config(&self) -> &Config {
            &self.config
        }
    }

    pub enum Start {
        Browser,
        Finder { phrase: Option<String> },
        Searcher { phrase: Option<String> },
        Nothing,
    }

    impl From<Option<Command>> for Start {
        fn from(command: Option<Command>) -> Self {
            match command {
                Some(Command::Browse) | None => Start::Browser,
                Some(Command::Search { phrase }) => Start::Searcher { phrase },
                Some(Command::Find { phrase }) => Start::Finder { phrase },
                Some(Command::Edit { browse, .. }) => match browse {
                    true => Start::Browser,
                    false => Start::Nothing,
                },
            }
        }
    }
}
pub use props::{Props, Start};

mod problem {
    use super::PromptEffect;

    use crate::components::common::{PromptChoice, PromptProps};

    /// Represents a problem that has been encountered.
    #[derive(Clone)]
    pub enum Problem {
        /// The starting directory argument is actually a file.
        DirectoryIsAFile,
    }

    impl From<Problem> for PromptProps<PromptEffect> {
        fn from(problem: Problem) -> PromptProps<PromptEffect> {
            let text: String = match problem {
                Problem::DirectoryIsAFile => String::from("The directory argument is actually a file. Would you like to use the parent directory instead?"),
            };

            let choices: Vec<PromptChoice<PromptEffect>> = match problem {
                Problem::DirectoryIsAFile => vec![
                    PromptChoice::builder()
                        .text_str("Yes")
                        .effect(PromptEffect::UseParentDirectory)
                        .build(),
                    PromptChoice::builder()
                        .text_str("No")
                        .effect(PromptEffect::DontUseParentDirectory)
                        .build(),
                ],
            };

            PromptProps::builder().text(text).choices(choices).build()
        }
    }
}
pub use problem::Problem;

pub struct Insh {
    state: State,
}

impl Component<Props, Event, SystemEffect> for Insh {
    fn new(props: Props) -> Self {
        Self {
            state: State::from(props),
        }
    }

    fn handle(&mut self, event: Event) -> Option<SystemEffect> {
        if let Event::Key(KeyEvent {
            code: KeyCode::Char('x'),
            modifiers: KeyModifiers::CONTROL,
        }) = event
        {
            return Some(SystemEffect::Exit);
        }

        let mut action: Option<Action> = None;

        match self.state.mode() {
            Mode::Browse => {
                let browser: &mut Browser = self.state.browser_mut().as_mut().unwrap();
                let browser_effect: Option<BrowserEffect> = browser.handle(event);
                match browser_effect {
                    Some(BrowserEffect::OpenFinder { directory }) => {
                        action = Some(Action::Find { directory });
                    }
                    Some(BrowserEffect::OpenSearcher { directory }) => {
                        action = Some(Action::Search { directory });
                    }
                    Some(BrowserEffect::OpenVim(vim_args)) => {
                        let program = Box::new(Vim::new(vim_args));
                        return Some(SystemEffect::RunProgram { program });
                    }
                    Some(BrowserEffect::RunBash { directory }) => {
                        let program = Box::new(Bash::new(directory));
                        return Some(SystemEffect::RunProgram { program });
                    }
                    Some(BrowserEffect::Bell) => {
                        action = Some(Action::Bell);
                    }
                    None => {}
                }
            }
            Mode::Finder => {
                let finder = self.state.finder_mut().as_mut().unwrap();
                let finder_effect: Option<FinderEffect> = finder.handle(event);
                match finder_effect {
                    Some(FinderEffect::Browse { directory, file }) => {
                        action = Some(Action::Browse { directory, file });
                    }
                    Some(FinderEffect::OpenVim(vim_args)) => {
                        let program = Box::new(Vim::new(vim_args));
                        return Some(SystemEffect::RunProgram { program });
                    }
                    Some(FinderEffect::Quit) => {
                        action = Some(Action::QuitFinder);
                    }
                    Some(FinderEffect::Bell) => {
                        action = Some(Action::Bell);
                    }
                    None => {}
                }
            }
            Mode::Searcher => {
                let searcher = self.state.searcher_mut().as_mut().unwrap();
                let searcher_effect: Option<SearcherEffect> = searcher.handle(event);
                match searcher_effect {
                    Some(SearcherEffect::Goto { directory, file }) => {
                        action = Some(Action::Browse { directory, file });
                    }
                    Some(SearcherEffect::Quit) => {
                        action = Some(Action::QuitSearcher);
                    }
                    Some(SearcherEffect::OpenVim(vim_args)) => {
                        let program = Box::new(Vim::new(vim_args));
                        return Some(SystemEffect::RunProgram { program });
                    }
                    Some(SearcherEffect::Bell) => {
                        action = Some(Action::Bell);
                    }
                    None => {}
                }
            }
            Mode::Nothing => {
                return Some(SystemEffect::Exit);
            }
        }

        if let Some(action) = action {
            let effect = self.state.perform(action);
            return effect;
        }

        None
    }

    fn render(&self, size: Size) -> Fabric {
        // If there is a prompt, then render the prompt (prompts are used to resolve problems).
        if let Some(prompt) = self.state.prompt() {
            return prompt.render(size);
        }

        match self.state.mode() {
            Mode::Browse => self.state.browser().as_ref().unwrap().render(size),
            Mode::Finder => self.state.finder().as_ref().unwrap().render(size),
            Mode::Searcher => self.state.searcher().as_ref().unwrap().render(size),
            Mode::Nothing => Fabric::new(size),
        }
    }
}

mod state {
    use super::super::browser::{Browser, BrowserProps};
    use super::super::finder::{Finder, FinderProps};
    use super::super::searcher::{Searcher, SearcherProps};
    use super::{Action, Mode, Problem, PromptEffect, Props, Start};

    use crate::component::Component;
    use crate::components::common::{Prompt, PromptProps};
    use crate::config::Config;
    use crate::current_dir;
    use crate::rendering::Size;
    use crate::stateful::Stateful;
    use crate::system_effect::SystemEffect;

    use std::path::PathBuf;

    use crossterm::terminal;

    pub struct State {
        mode: Mode,

        prompt: Option<Prompt<PromptEffect>>,

        browser: Option<Browser>,
        finder: Option<Finder>,
        searcher: Option<Searcher>,

        config: Config,
    }

    impl From<Props> for State {
        fn from(props: Props) -> Self {
            let directory: PathBuf = props
                .directory()
                .clone()
                .unwrap_or_else(current_dir::current_dir);
            let size: Size = Size::from(terminal::size().unwrap());

            let problem: Option<Problem> = props.problem().clone();
            let prompt: Option<Prompt<PromptEffect>> = match problem {
                None => None,
                Some(problem) => {
                    let promt_props: PromptProps<PromptEffect> = problem.into();
                    let prompt: Prompt<PromptEffect> = Prompt::new(promt_props);
                    Some(prompt)
                }
            };

            let browser_props = BrowserProps::new(directory.clone(), size, None);
            let browser = Some(Browser::new(browser_props));
            match props.start() {
                Start::Browser => Self {
                    mode: Mode::Browse,
                    prompt,
                    browser,
                    finder: None,
                    searcher: None,
                    config: props.config().clone(),
                },
                Start::Finder { phrase } => {
                    let finder_props = FinderProps::new(directory, size, phrase.clone());
                    let finder = Some(Finder::new(finder_props));
                    Self {
                        mode: Mode::Finder,
                        prompt,
                        browser,
                        finder,
                        searcher: None,
                        config: props.config().clone(),
                    }
                }
                Start::Searcher { phrase } => {
                    let searcher_props =
                        SearcherProps::new(props.config().clone(), directory, size, phrase.clone());
                    let searcher = Some(Searcher::new(searcher_props));
                    Self {
                        mode: Mode::Searcher,
                        prompt,
                        browser,
                        finder: None,
                        searcher,
                        config: props.config().clone(),
                    }
                }
                Start::Nothing => Self {
                    mode: Mode::Nothing,
                    prompt,
                    browser: None,
                    finder: None,
                    searcher: None,
                    config: props.config().clone(),
                },
            }
        }
    }

    impl State {
        pub fn mode(&self) -> &Mode {
            &self.mode
        }

        pub fn prompt(&self) -> &Option<Prompt<PromptEffect>> {
            &self.prompt
        }

        pub fn browser(&self) -> &Option<Browser> {
            &self.browser
        }

        pub fn browser_mut(&mut self) -> &mut Option<Browser> {
            &mut self.browser
        }

        pub fn finder(&self) -> &Option<Finder> {
            &self.finder
        }

        pub fn finder_mut(&mut self) -> &mut Option<Finder> {
            &mut self.finder
        }

        pub fn searcher(&self) -> &Option<Searcher> {
            &self.searcher
        }

        pub fn searcher_mut(&mut self) -> &mut Option<Searcher> {
            &mut self.searcher
        }

        pub fn browse(
            &mut self,
            directory: PathBuf,
            file: Option<PathBuf>,
        ) -> Option<SystemEffect> {
            self.mode = Mode::Browse;
            let size: Size = Size::from(terminal::size().unwrap());
            let browser_props = BrowserProps::new(directory, size, file);
            self.browser = Some(Browser::new(browser_props));
            None
        }

        pub fn find(&mut self, directory: PathBuf) -> Option<SystemEffect> {
            self.mode = Mode::Finder;
            let size: Size = Size::from(terminal::size().unwrap());
            let phrase = None;
            let finder_props = FinderProps::new(directory, size, phrase);
            self.finder = Some(Finder::new(finder_props));
            None
        }

        pub fn search(&mut self, directory: PathBuf) -> Option<SystemEffect> {
            self.mode = Mode::Searcher;
            let size: Size = Size::from(terminal::size().unwrap());
            let phrase = None;
            let searcher_props = SearcherProps::new(self.config.clone(), directory, size, phrase);
            self.searcher = Some(Searcher::new(searcher_props));
            None
        }

        pub fn quit_finder(&mut self) -> Option<SystemEffect> {
            self.mode = Mode::Browse;
            None
        }

        pub fn quit_searcher(&mut self) -> Option<SystemEffect> {
            self.mode = Mode::Browse;
            None
        }

        /// If the bell sound is configured to be made, then return the effect for making the bell
        /// sound.
        pub fn bell(&self) -> Option<SystemEffect> {
            match self.config.general().bell() {
                true => Some(SystemEffect::Bell),
                false => None,
            }
        }
    }

    impl Stateful<Action, SystemEffect> for State {
        fn perform(&mut self, action: Action) -> Option<SystemEffect> {
            match action {
                Action::Browse { directory, file } => self.browse(directory, file),
                Action::Find { directory } => self.find(directory),
                Action::Search { directory } => self.search(directory),
                Action::QuitFinder => self.quit_finder(),
                Action::QuitSearcher => self.quit_searcher(),
                Action::Bell => self.bell(),
            }
        }
    }
}
use state::State;

mod mode {
    pub enum Mode {
        Browse,
        Finder,
        Searcher,
        Nothing,
    }

    impl Default for Mode {
        fn default() -> Self {
            Mode::Browse
        }
    }
}
pub use mode::Mode;

enum Action {
    Browse {
        directory: PathBuf,
        file: Option<PathBuf>,
    },
    Find {
        directory: PathBuf,
    },
    Search {
        directory: PathBuf,
    },
    Bell,
    QuitFinder,
    QuitSearcher,
}

mod prompt_effect {
    #[derive(Clone)]
    pub enum PromptEffect {
        UseParentDirectory,
        DontUseParentDirectory,
    }
}
pub use prompt_effect::PromptEffect;
