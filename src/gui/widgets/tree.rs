//! Universal tree widget — themed expand/collapse rows with persistent state.
//!
//! Used by file browsers, equipment trees, skill hierarchies, market category
//! browsers, and anything else with parent-child structure. Every node is
//! identified by a stable string id; expansion + selection state lives in
//! `TreeState` so the page only owns "does this id mean anything to me".
//!
//! Two entry points:
//! - `tree_node()` — render a node with children. Caller passes a closure that
//!   recurses for children when expanded.
//! - `tree_leaf()` — render a leaf (no children). Just a clickable row.
//!
//! Example:
//! ```ignore
//! fn draw(ui, theme, state, items) {
//!     for item in items {
//!         if item.children.is_empty() {
//!             if widgets::tree_leaf(ui, theme, state, &item.id, &item.label, None).clicked {
//!                 // handle selection
//!             }
//!         } else {
//!             widgets::tree_node(ui, theme, state, &item.id, &item.label, None, |ui| {
//!                 draw(ui, theme, state, &item.children);
//!             });
//!         }
//!     }
//! }
//! ```

use egui::{Color32, RichText, Ui};
use std::collections::HashSet;
use super::super::theme::Theme;

/// Persistent state for a tree view. Tracks which nodes are expanded and which
/// is currently selected. One `TreeState` per logical tree (e.g. one per page).
#[derive(Debug, Default, Clone)]
pub struct TreeState {
    /// Set of node ids that are currently expanded.
    expanded: HashSet<String>,
    /// Id of the currently selected node, if any.
    selected: Option<String>,
}

impl TreeState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Open the given node (and any of its ancestors that share id prefix).
    pub fn expand(&mut self, id: &str) {
        self.expanded.insert(id.to_string());
    }

    /// Close the given node.
    pub fn collapse(&mut self, id: &str) {
        self.expanded.remove(id);
    }

    /// Whether the given node is open.
    pub fn is_expanded(&self, id: &str) -> bool {
        self.expanded.contains(id)
    }

    /// Mark the given node as selected.
    pub fn select(&mut self, id: &str) {
        self.selected = Some(id.to_string());
    }

    /// Clear selection.
    pub fn clear_selection(&mut self) {
        self.selected = None;
    }

    /// Whether the given node is selected.
    pub fn is_selected(&self, id: &str) -> bool {
        self.selected.as_deref() == Some(id)
    }

    /// Currently selected node id, if any.
    pub fn selected_id(&self) -> Option<&str> {
        self.selected.as_deref()
    }
}

/// Response from rendering a tree row.
#[derive(Debug, Default, Clone, Copy)]
pub struct TreeNodeResponse {
    /// True if the row label was clicked this frame (for selection).
    pub clicked: bool,
    /// True if the expand chevron was toggled this frame.
    pub toggled: bool,
    /// Whether the node is currently open after this frame.
    pub is_expanded: bool,
}

/// Render a tree node with children. The `add_children` closure receives the
/// `&mut TreeState` back so recursive calls can re-borrow it without fighting
/// the borrow checker. Runs only when the node is expanded.
///
/// ```ignore
/// widgets::tree_node(ui, theme, tree, &id, label, None, |ui, tree| {
///     for child in children {
///         draw_my_tree(ui, theme, tree, child);
///     }
/// });
/// ```
pub fn tree_node(
    ui: &mut Ui,
    theme: &Theme,
    state: &mut TreeState,
    id: &str,
    label: &str,
    icon: Option<&str>,
    add_children: impl FnOnce(&mut Ui, &mut TreeState),
) -> TreeNodeResponse {
    let mut resp = TreeNodeResponse::default();
    let was_expanded = state.is_expanded(id);
    let is_selected = state.is_selected(id);

    let chevron = if was_expanded { "v" } else { ">" };
    let header = match icon {
        Some(ic) => format!("{} {}  {}", chevron, ic, label),
        None     => format!("{}  {}", chevron, label),
    };

    let label_color = if is_selected {
        theme.text_on_accent()
    } else {
        theme.accent()
    };

    let row = ui.selectable_label(
        is_selected,
        RichText::new(&header).color(label_color),
    );

    if row.clicked() {
        // Single click on header toggles expand AND selects.
        if was_expanded {
            state.collapse(id);
        } else {
            state.expand(id);
        }
        state.select(id);
        resp.toggled = true;
        resp.clicked = true;
    }

    let now_expanded = state.is_expanded(id);
    resp.is_expanded = now_expanded;

    if now_expanded {
        ui.indent(egui::Id::new(id), |ui| {
            add_children(ui, state);
        });
    }

    resp
}

/// Render a leaf row (no children). Returns whether it was clicked.
pub fn tree_leaf(
    ui: &mut Ui,
    theme: &Theme,
    state: &mut TreeState,
    id: &str,
    label: &str,
    icon: Option<&str>,
) -> TreeNodeResponse {
    let mut resp = TreeNodeResponse::default();
    let is_selected = state.is_selected(id);

    let header = match icon {
        Some(ic) => format!("{}  {}", ic, label),
        None     => label.to_string(),
    };

    let label_color = if is_selected {
        theme.text_on_accent()
    } else {
        theme.text_primary()
    };

    let row = ui.selectable_label(
        is_selected,
        RichText::new(&header).color(label_color),
    );

    if row.clicked() {
        state.select(id);
        resp.clicked = true;
    }

    resp
}

/// Same as `tree_leaf` but with an explicit color override (e.g. for muted /
/// disabled / warning items).
pub fn tree_leaf_colored(
    ui: &mut Ui,
    state: &mut TreeState,
    id: &str,
    label: &str,
    icon: Option<&str>,
    color: Color32,
) -> TreeNodeResponse {
    let mut resp = TreeNodeResponse::default();
    let is_selected = state.is_selected(id);

    let header = match icon {
        Some(ic) => format!("{}  {}", ic, label),
        None     => label.to_string(),
    };

    let row = ui.selectable_label(
        is_selected,
        RichText::new(&header).color(color),
    );

    if row.clicked() {
        state.select(id);
        resp.clicked = true;
    }

    resp
}
