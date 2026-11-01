//! A session, the windows in it, and the panes in those.

use std::collections::HashMap;

use super::pane::Pane;
use super::tree::Tree;

use insh_api::{Layout, PaneKind, PaneLayout, Split, Window as WindowSummary};
use rend::{Location, Size};

use typed_builder::TypedBuilder;
use uuid::Uuid;

/// One of the windows of a session.
#[derive(TypedBuilder)]
pub struct Window {
    /// Which window it is.
    #[builder(default = Uuid::new_v4())]
    id: Uuid,
    /// What it is called.
    #[builder(setter(into))]
    name: String,
    /// How the panes in it are arranged.
    tree: Tree,
    /// Which pane what is typed goes to.
    active: Uuid,
    /// The pane which has been made to fill the window, if one has.
    ///
    /// The panes are left arranged the way they were, so putting the window back shows them where
    /// they were before.
    #[builder(default)]
    zoomed: Option<Uuid>,
    /// Whether what is typed goes to every pane of the window at once.
    #[builder(default)]
    synchronized: bool,
}

impl Window {
    /// Return which window it is.
    pub fn id(&self) -> Uuid {
        return self.id;
    }

    /// Return what it is called.
    pub fn name(&self) -> &str {
        return &self.name;
    }

    /// Return how the panes in it are arranged.
    pub fn tree(&self) -> &Tree {
        return &self.tree;
    }

    /// Return how the panes in it are arranged, to be changed.
    pub fn tree_mut(&mut self) -> &mut Tree {
        return &mut self.tree;
    }

    /// Return which pane what is typed goes to.
    pub fn active(&self) -> Uuid {
        return self.active;
    }

    /// Call it something else.
    pub fn rename(&mut self, name: String) {
        self.name = name;
    }

    /// Send what is typed to the given pane, if it is one of the ones in the window.
    pub fn select(&mut self, pane: Uuid) -> bool {
        if !self.tree.panes().contains(&pane) {
            return false;
        }
        self.active = pane;
        return true;
    }

    /// Return the pane which has been made to fill the window, if one has.
    pub fn zoomed(&self) -> Option<Uuid> {
        // A pane which has gone takes the window back to showing all of them.
        return self.zoomed.filter(|pane| self.tree.panes().contains(pane));
    }

    /// Make the pane fill the window, or put the window back when it already does.
    pub fn zoom(&mut self, pane: Uuid) -> bool {
        if !self.tree.panes().contains(&pane) {
            return false;
        }

        self.zoomed = match self.zoomed() == Some(pane) {
            true => None,
            false => Some(pane),
        };
        // What is typed goes to whichever pane fills the window, since it is the only one shown.
        self.active = pane;

        return true;
    }

    /// Return whether what is typed goes to every pane of the window at once.
    pub fn synchronized(&self) -> bool {
        return self.synchronized;
    }

    /// Send what is typed to every pane of the window at once, or stop.
    pub fn synchronize(&mut self) {
        self.synchronized = !self.synchronized;
    }
}

/// A session: the windows a client shows and the panes in them.
///
/// The daemon owns this rather than a client, which is what lets a client go away and come back to
/// find the programs in it still running.
#[derive(TypedBuilder)]
pub struct Session {
    /// Which session it is.
    #[builder(default = Uuid::new_v4())]
    id: Uuid,
    /// What it is called.
    #[builder(setter(into))]
    name: String,
    /// Which one it is by the order they were made in, so that a new one is listed after the ones
    /// which were there before it however it is named.
    #[builder(default)]
    made: usize,
    /// The windows of it, in the order they are shown in.
    windows: Vec<Window>,
    /// Which window is being looked at.
    active: Uuid,
    /// The panes of every window of it.
    panes: HashMap<Uuid, Pane>,
    /// How big the smallest of the terminals showing it is, which is how big it is drawn.
    size: Size,
}

impl Session {
    /// How many rows the footer takes up along the bottom of a client.
    pub const FOOTER_ROWS: usize = 1;

    /// Return which session it is.
    pub fn id(&self) -> Uuid {
        return self.id;
    }

    /// Return what it is called.
    pub fn name(&self) -> &str {
        return &self.name;
    }

    /// Return which one it is by the order they were made in.
    pub fn made(&self) -> usize {
        return self.made;
    }

    /// Call it something else.
    pub fn rename(&mut self, name: String) {
        self.name = name;
    }

    /// Return the given window, to be changed, or the one being looked at when there is none.
    pub fn window_or_active_mut(&mut self, window: Option<&Uuid>) -> &mut Window {
        let which: Option<Uuid> = window
            .copied()
            .filter(|window| self.windows.iter().any(|shown| shown.id() == *window));

        return match which {
            Some(which) => self
                .windows
                .iter_mut()
                .find(|shown| shown.id() == which)
                .unwrap(),
            None => self.window_mut(),
        };
    }

    /// Return how many windows it has.
    pub fn window_count(&self) -> usize {
        return self.windows.len();
    }

    /// Return the window which is being looked at.
    pub fn window(&self) -> &Window {
        return self
            .windows
            .iter()
            .find(|window| window.id() == self.active)
            .unwrap_or(&self.windows[0]);
    }

    /// Return the window which is being looked at, to be changed.
    pub fn window_mut(&mut self) -> &mut Window {
        let active: Uuid = self.window().id();
        return self
            .windows
            .iter_mut()
            .find(|window| window.id() == active)
            .unwrap();
    }

    /// Return the pane what is typed goes to.
    pub fn active_pane(&self) -> Uuid {
        return self.window().active();
    }

    /// Return the panes what is typed goes to, which is every pane of the window when they are
    /// being typed in together and only the one otherwise.
    pub fn typed_in(&self) -> Vec<Uuid> {
        let window: &Window = self.window();

        return match window.synchronized() {
            true => window.tree().panes(),
            false => vec![window.active()],
        };
    }

    /// Return the given pane, to be written to.
    pub fn pane_mut(&mut self, pane: &Uuid) -> Option<&mut Pane> {
        return self.panes.get_mut(pane);
    }

    /// Return every pane of it, to be written to.
    pub fn panes_mut(&mut self) -> impl Iterator<Item = &mut Pane> {
        return self.panes.values_mut();
    }

    /// Return whether the pane is one of the ones in the session at all, whichever of its windows
    /// it is in.
    pub fn has_pane(&self, pane: &Uuid) -> bool {
        return self.panes.contains_key(pane);
    }

    /// Return whether the given pane is one of the ones in the window being looked at.
    pub fn is_shown(&self, pane: &Uuid) -> bool {
        return self.window().tree().panes().contains(pane);
    }

    /// Take the pane as being one of the ones in the session.
    pub fn add_pane(&mut self, pane: Pane) {
        self.panes.insert(pane.id(), pane);
    }

    /// Add a window with the one pane in it and look at it.
    pub fn add_window(&mut self, name: String, pane: Pane) {
        let window = Window::builder()
            .name(name)
            .tree(Tree::Pane(pane.id()))
            .active(pane.id())
            .build();

        self.active = window.id();
        self.windows.push(window);
        self.add_pane(pane);
    }

    /// Look at the given window, if it is one of the ones in the session.
    pub fn select_window(&mut self, window: Uuid) -> bool {
        if !self.windows.iter().any(|shown| shown.id() == window) {
            return false;
        }
        self.active = window;
        return true;
    }

    /// Take the pane out of whichever window it is in, closing the window along with it when it
    /// was the last one, and return whether there is anything left of the session.
    pub fn remove_pane(&mut self, pane: &Uuid) -> bool {
        if let Some(mut pane) = self.panes.remove(pane) {
            pane.close();
        }

        for window in &mut self.windows {
            window.tree_mut().remove(pane);
        }

        // NOTE: A window whose only pane is the one which went has nothing left in it. Taking the
        // pane out of the arrangement cannot do that, because the one pane is the whole of the
        // arrangement and there is nothing for it to collapse into, so the window goes here.
        self.windows
            .retain(|window| !matches!(window.tree(), Tree::Pane(only) if only == pane));

        // A window with no panes left in it is no window at all.
        self.windows
            .retain(|window| !window.tree().panes().is_empty());

        for window in &mut self.windows {
            // Whichever pane what was typed went to may have been the one which went.
            let panes: Vec<Uuid> = window.tree().panes();
            if !panes.contains(&window.active()) {
                window.select(panes[0]);
            }
        }

        if self.windows.is_empty() {
            return false;
        }
        if !self.windows.iter().any(|window| window.id() == self.active) {
            self.active = self.windows[0].id();
        }

        return true;
    }

    /// Change how big the terminal showing it is, and every pane in it along with that.
    pub fn resize(&mut self, size: Size) {
        self.size = size;
        self.fit();
    }

    /// Give every pane of the window being looked at the size the arrangement of them says.
    pub fn fit(&mut self) {
        // NOTE: The sizes of the parts are only put in the same units as the screen when all of
        // them are shown. A pane which has been made to fill the window is given the whole of it
        // without the arrangement changing, so that the window goes back to what it was.
        if self.window().zoomed().is_none() {
            let size: Size = self.window_size();
            self.window_mut().tree_mut().rescale(size);
        }

        for layout in self.layouts() {
            if let Some(pane) = self.panes.get_mut(layout.pane()) {
                pane.resize(layout.size());
            }
        }
    }

    /// Return how big the window being looked at is, which is the whole of the client but for the
    /// footer along the bottom of it.
    pub fn window_size(&self) -> Size {
        let rows: usize = self.size.rows.saturating_sub(Self::FOOTER_ROWS).max(1);
        return Size::new(rows, self.size.columns);
    }

    /// Return where every pane of the window being looked at is, along with what it holds.
    ///
    /// A window with a pane made to fill it has only that pane in it, since the rest of them are
    /// not shown while it does.
    pub fn layouts(&self) -> Vec<PaneLayout> {
        let window: &Window = self.window();

        if let Some(zoomed) = window.zoomed() {
            let kind: PaneKind = self
                .panes
                .get(&zoomed)
                .map_or(PaneKind::Component, Pane::kind);

            return vec![PaneLayout::builder()
                .pane(zoomed)
                .at(Location::default())
                .size(self.window_size())
                .kind(kind)
                .build()];
        }

        return window
            .tree()
            .layout(Location::default(), self.window_size())
            .into_iter()
            .map(|layout| {
                let kind: PaneKind = self
                    .panes
                    .get(layout.pane())
                    .map_or(PaneKind::Component, Pane::kind);

                PaneLayout::builder()
                    .pane(*layout.pane())
                    .at(layout.at())
                    .size(layout.size())
                    .kind(kind)
                    .build()
            })
            .collect();
    }

    /// Return each of the windows of the session, and the panes of each, for a client which is
    /// listing them.
    pub fn window_summaries(&self) -> Vec<insh_api::WindowSummary> {
        return self
            .windows
            .iter()
            .map(|window| {
                let panes: Vec<insh_api::PaneSummary> = window
                    .tree()
                    .panes()
                    .into_iter()
                    .map(|pane| {
                        insh_api::PaneSummary::builder()
                            .pane(pane)
                            .active(pane == window.active())
                            .build()
                    })
                    .collect();

                insh_api::WindowSummary::builder()
                    .window(window.id())
                    .name(window.name())
                    .panes(panes)
                    .active(window.id() == self.active)
                    .build()
            })
            .collect();
    }

    /// Return everything a client needs in order to know what to draw where.
    pub fn layout(&self) -> Layout {
        return Layout::builder()
            .session(self.id)
            .name(self.name.clone())
            .windows(
                self.windows
                    .iter()
                    .map(|window| {
                        WindowSummary::builder()
                            .window(window.id())
                            .name(window.name())
                            .build()
                    })
                    .collect(),
            )
            .window(self.window().id())
            .panes(self.layouts())
            .pane(self.active_pane())
            .zoomed(self.window().zoomed().is_some())
            .synchronized(self.window().synchronized())
            .build();
    }

    /// Move the line on the far side of the pane the given number of rows or columns.
    pub fn drag(&mut self, panes: &[Uuid], split: Split, by: isize) -> bool {
        if !self.window_mut().tree_mut().drag(panes, split, by) {
            return false;
        }

        self.fit();
        return true;
    }

    /// Swap the pane with the one before or after it in the arrangement.
    pub fn swap(&mut self, pane: &Uuid, after: bool) -> bool {
        return self.window_mut().tree_mut().swap(pane, after);
    }

    /// Split the pane what is typed goes to, putting the new pane beside or below it.
    pub fn split(&mut self, pane: &Uuid, split: Split, new: Pane) -> bool {
        let id: Uuid = new.id();

        if !self.window_mut().tree_mut().split(pane, split, id) {
            return false;
        }

        self.window_mut().select(id);
        self.add_pane(new);
        self.fit();

        return true;
    }
}
