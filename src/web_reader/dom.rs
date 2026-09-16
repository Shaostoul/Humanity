//! The tree html5ever builds a page into: a flat arena of nodes addressed by
//! index, with a hand-written `TreeSink`.
//!
//! WHY OUR OWN SINK. html5ever ships no DOM of its own; the reference one
//! (`markup5ever_rcdom`) is published a version behind the parser (0.39
//! against html5ever 0.40 at pin time), its types do not line up, and its own
//! description is "basic, unsupported, for use by tests". The sink contract is
//! small (append, insert-before, reparent, attributes), and an index arena is
//! simpler than the `Rc<RefCell>` web the reference builds: no cycles, no
//! weak parents, and once parsing is done `into_nodes` hands the parser a
//! plain `Vec` it can walk by index.
//!
//! The parser drives the sink through `&self`, so the arena sits in a
//! `RefCell`. Every method borrows for its own duration only; nothing holds a
//! borrow across a call to another sink method.

use std::borrow::Cow;
use std::cell::{Ref, RefCell};

use html5ever::interface::{ElementFlags, NodeOrText, QuirksMode, TreeSink};
use html5ever::tendril::StrTendril;
use html5ever::{Attribute, QualName};

/// Index of a node in the arena. Node 0 is always the document.
pub type NodeId = usize;

/// What a node is. Only elements and text matter to the reader; the rest are
/// kept so the tree stays faithful to what the parser built.
#[derive(Debug)]
pub enum NodeData {
    Document,
    Doctype,
    Comment,
    Text(String),
    Element {
        name: QualName,
        attrs: Vec<Attribute>,
        /// A `<template>` owns a separate document fragment for its contents.
        /// The reader ignores templates (they are inert until a script
        /// stamps them), so this is only here to satisfy the sink contract.
        template_contents: Option<NodeId>,
    },
}

#[derive(Debug)]
pub struct Node {
    pub data: NodeData,
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,
}

impl Node {
    /// The element's local tag name in lowercase (`"div"`, `"a"`), or `None`
    /// for a non-element. The HTML namespace only: an `<svg>` subtree still
    /// reports its tag names, which is enough for the reader to drop it.
    pub fn tag(&self) -> Option<&str> {
        match &self.data {
            NodeData::Element { name, .. } => Some(name.local.as_ref()),
            _ => None,
        }
    }

    /// Value of attribute `local` (case as html5ever lowercased it).
    pub fn attr(&self, local: &str) -> Option<&str> {
        match &self.data {
            NodeData::Element { attrs, .. } => attrs
                .iter()
                .find(|a| &*a.name.local == local)
                .map(|a| &*a.value),
            _ => None,
        }
    }

    pub fn text(&self) -> Option<&str> {
        match &self.data {
            NodeData::Text(t) => Some(t),
            _ => None,
        }
    }
}

/// The arena plus the `TreeSink` implementation.
pub struct Dom {
    nodes: RefCell<Vec<Node>>,
}

impl Default for Dom {
    fn default() -> Self {
        Self::new()
    }
}

impl Dom {
    pub fn new() -> Self {
        Self {
            nodes: RefCell::new(vec![Node {
                data: NodeData::Document,
                parent: None,
                children: Vec::new(),
            }]),
        }
    }

    /// Hand the finished tree over as a plain vector, indexed by [`NodeId`].
    pub fn into_nodes(self) -> Vec<Node> {
        self.nodes.into_inner()
    }

    fn push(&self, data: NodeData) -> NodeId {
        let mut nodes = self.nodes.borrow_mut();
        nodes.push(Node { data, parent: None, children: Vec::new() });
        nodes.len() - 1
    }

    /// Detach `id` from whatever parent it has. Safe on an orphan.
    fn detach(nodes: &mut [Node], id: NodeId) {
        if let Some(p) = nodes[id].parent.take() {
            nodes[p].children.retain(|&c| c != id);
        }
    }

    /// Append `text` to the end of an existing text node, if `id` is one.
    /// The parser emits text in many small pieces (every entity, every line
    /// end); merging keeps one text node per run so the reader sees words,
    /// not fragments.
    fn merge_text(nodes: &mut [Node], id: NodeId, text: &str) -> bool {
        if let NodeData::Text(t) = &mut nodes[id].data {
            t.push_str(text);
            true
        } else {
            false
        }
    }
}

impl TreeSink for Dom {
    type Handle = NodeId;
    type Output = Dom;
    // `Ref<QualName>` implements `ElemName` upstream, which is exactly what a
    // RefCell arena can hand out without copying the name.
    type ElemName<'a>
        = Ref<'a, QualName>
    where
        Self: 'a;

    fn finish(self) -> Dom {
        self
    }

    /// Parse errors are recoverable by definition (html5ever repairs the
    /// tree the way a browser would); the reader has no use for the list.
    fn parse_error(&self, _msg: Cow<'static, str>) {}

    fn get_document(&self) -> NodeId {
        0
    }

    fn elem_name<'a>(&'a self, target: &'a NodeId) -> Ref<'a, QualName> {
        Ref::map(self.nodes.borrow(), |nodes| match &nodes[*target].data {
            NodeData::Element { name, .. } => name,
            _ => panic!("elem_name on a non-element node"),
        })
    }

    fn create_element(&self, name: QualName, attrs: Vec<Attribute>, flags: ElementFlags) -> NodeId {
        let template_contents = if flags.template { Some(self.push(NodeData::Document)) } else { None };
        self.push(NodeData::Element { name, attrs, template_contents })
    }

    fn create_comment(&self, _text: StrTendril) -> NodeId {
        self.push(NodeData::Comment)
    }

    fn create_pi(&self, _target: StrTendril, _data: StrTendril) -> NodeId {
        self.push(NodeData::Comment)
    }

    fn append(&self, parent: &NodeId, child: NodeOrText<NodeId>) {
        let mut nodes = self.nodes.borrow_mut();
        let id = match child {
            NodeOrText::AppendText(text) => {
                if let Some(&last) = nodes[*parent].children.last() {
                    if Self::merge_text(&mut nodes, last, &text) {
                        return;
                    }
                }
                nodes.push(Node { data: NodeData::Text(text.to_string()), parent: None, children: Vec::new() });
                nodes.len() - 1
            }
            NodeOrText::AppendNode(id) => {
                Self::detach(&mut nodes, id);
                id
            }
        };
        nodes[id].parent = Some(*parent);
        nodes[*parent].children.push(id);
    }

    fn append_based_on_parent_node(
        &self,
        element: &NodeId,
        prev_element: &NodeId,
        child: NodeOrText<NodeId>,
    ) {
        let has_parent = self.nodes.borrow()[*element].parent.is_some();
        if has_parent {
            self.append_before_sibling(element, child);
        } else {
            self.append(prev_element, child);
        }
    }

    fn append_doctype_to_document(&self, _name: StrTendril, _public_id: StrTendril, _system_id: StrTendril) {
        let id = self.push(NodeData::Doctype);
        let mut nodes = self.nodes.borrow_mut();
        nodes[id].parent = Some(0);
        nodes[0].children.push(id);
    }

    fn get_template_contents(&self, target: &NodeId) -> NodeId {
        match &self.nodes.borrow()[*target].data {
            NodeData::Element { template_contents: Some(c), .. } => *c,
            _ => panic!("get_template_contents on a non-template node"),
        }
    }

    fn same_node(&self, x: &NodeId, y: &NodeId) -> bool {
        x == y
    }

    /// Quirks mode changes CSS layout, which the reader does not do.
    fn set_quirks_mode(&self, _mode: QuirksMode) {}

    fn append_before_sibling(&self, sibling: &NodeId, new_node: NodeOrText<NodeId>) {
        let mut nodes = self.nodes.borrow_mut();
        let parent = nodes[*sibling].parent.expect("append_before_sibling on a node without a parent");
        let index = nodes[parent]
            .children
            .iter()
            .position(|&c| c == *sibling)
            .expect("sibling is in its parent's child list");
        let id = match new_node {
            NodeOrText::AppendText(text) => {
                // Same merging rule as `append`: text placed right after an
                // existing text node joins it.
                if index > 0 {
                    let prev = nodes[parent].children[index - 1];
                    if Self::merge_text(&mut nodes, prev, &text) {
                        return;
                    }
                }
                nodes.push(Node { data: NodeData::Text(text.to_string()), parent: None, children: Vec::new() });
                nodes.len() - 1
            }
            NodeOrText::AppendNode(id) => {
                Self::detach(&mut nodes, id);
                id
            }
        };
        nodes[id].parent = Some(parent);
        // Re-find the index: detaching `id` may have shifted the list when it
        // was already a child of this parent.
        let index = nodes[parent].children.iter().position(|&c| c == *sibling).unwrap_or(index);
        nodes[parent].children.insert(index, id);
    }

    fn add_attrs_if_missing(&self, target: &NodeId, new_attrs: Vec<Attribute>) {
        let mut nodes = self.nodes.borrow_mut();
        if let NodeData::Element { attrs, .. } = &mut nodes[*target].data {
            for a in new_attrs {
                if !attrs.iter().any(|e| e.name == a.name) {
                    attrs.push(a);
                }
            }
        }
    }

    fn remove_from_parent(&self, target: &NodeId) {
        let mut nodes = self.nodes.borrow_mut();
        Self::detach(&mut nodes, *target);
    }

    fn reparent_children(&self, node: &NodeId, new_parent: &NodeId) {
        let mut nodes = self.nodes.borrow_mut();
        let moved = std::mem::take(&mut nodes[*node].children);
        for &c in &moved {
            nodes[c].parent = Some(*new_parent);
        }
        nodes[*new_parent].children.extend(moved);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use html5ever::tendril::TendrilSink;

    fn parse(html: &str) -> Vec<Node> {
        html5ever::parse_document(Dom::new(), Default::default()).one(html).into_nodes()
    }

    /// Text arrives from the tokenizer in pieces; the sink must hand the
    /// reader one node per run, or every entity would split a word.
    #[test]
    fn adjacent_text_pieces_merge_into_one_node() {
        let nodes = parse("<p>a&amp;b and c</p>");
        let p = nodes.iter().position(|n| n.tag() == Some("p")).expect("p");
        assert_eq!(nodes[p].children.len(), 1, "one text node under <p>, got {:?}", nodes[p].children);
        assert_eq!(nodes[nodes[p].children[0]].text(), Some("a&b and c"));
    }

    /// The parser reparents when it repairs misnested markup (the classic
    /// `<b><i></b></i>`). Parents and child lists must agree afterwards.
    #[test]
    fn repaired_tree_keeps_parent_and_child_lists_consistent() {
        let nodes = parse("<p><b>bold <i>both</b> italic</i> plain</p>");
        for (id, n) in nodes.iter().enumerate() {
            for &c in &n.children {
                assert_eq!(nodes[c].parent, Some(id), "child {c} does not point back at {id}");
            }
            if let Some(p) = n.parent {
                assert!(nodes[p].children.contains(&id), "parent {p} does not list {id}");
            }
        }
        // The visible text survives the repair in order.
        let mut text = String::new();
        fn walk(nodes: &[Node], id: usize, out: &mut String) {
            if let Some(t) = nodes[id].text() {
                out.push_str(t);
            }
            for &c in &nodes[id].children {
                walk(nodes, c, out);
            }
        }
        walk(&nodes, 0, &mut text);
        assert_eq!(text, "bold both italic plain");
    }
}
