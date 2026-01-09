use crate::objects::{Document, DocumentType};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    pub document: Document,
    pub children: HashMap<String, Node>,
}

impl Node {
    pub fn new(document: Document) -> Self {
        Node {
            document,
            children: HashMap::new(),
        }
    }

    pub fn is_directory(&self) -> bool {
        self.document.doc_type == DocumentType::Collection
    }

    pub fn id(&self) -> String {
        self.document.id.to_string()
    }

    pub fn name(&self) -> &str {
        &self.document.display_name
    }
}

pub struct FileTree {
    pub root: Node,
}

impl FileTree {
    pub fn new() -> Self {
        let root_doc = Document {
            id: uuid::Uuid::nil(),
            display_name: "/".to_string(),
            doc_type: DocumentType::Collection,
            ..Default::default()
        };
        FileTree {
            root: Node::new(root_doc),
        }
    }

    pub fn build(documents: Vec<Document>) -> Self {
        let mut tree = Self::new();

        // Add special "trash" node
        let trash_id = "trash";
        let trash_node = Node::new(Document {
            id: uuid::Uuid::nil(), // dummy
            display_name: "trash".to_string(),
            doc_type: DocumentType::Collection,
            parent: "".to_string(),
            ..Default::default()
        });
        tree.root.children.insert(trash_id.to_string(), trash_node);

        let mut id_to_node: HashMap<String, Node> = documents
            .into_iter()
            .map(|d| (d.id.to_string(), Node::new(d)))
            .collect();

        let mut child_to_parent = HashMap::new();
        for (id, node) in &id_to_node {
            if !node.document.parent.is_empty() {
                child_to_parent.insert(id.clone(), node.document.parent.clone());
            }
        }

        let ids: Vec<String> = id_to_node.keys().cloned().collect();
        for id in ids {
            if !child_to_parent.contains_key(&id) {
                // Root level
                if let Some(node) = id_to_node.remove(&id) {
                    tree.root.children.insert(id, node);
                }
            }
        }

        let mut remaining = id_to_node;
        let mut progress = true;
        while !remaining.is_empty() && progress {
            progress = false;
            let current_remaining_ids: Vec<String> = remaining.keys().cloned().collect();
            for id in current_remaining_ids {
                let parent_id = child_to_parent.get(&id).unwrap();

                // Special case: if trash is the parent
                if parent_id == "trash" {
                    if let Some(node) = remaining.remove(&id) {
                        if let Some(trash) = tree.root.children.get_mut("trash") {
                            trash.children.insert(id, node);
                            progress = true;
                        }
                    }
                    continue;
                }

                if let Some(parent_node) = find_node_mut(&mut tree.root, parent_id) {
                    if let Some(node) = remaining.remove(&id) {
                        parent_node.children.insert(id, node);
                        progress = true;
                    }
                }
            }
        }

        if !remaining.is_empty() {
            for (id, node) in remaining {
                tree.root.children.insert(id, node);
            }
        }

        tree
    }
}

fn find_node_mut<'a>(current: &'a mut Node, id: &str) -> Option<&'a mut Node> {
    if current.id() == id {
        return Some(current);
    }
    for child in current.children.values_mut() {
        if let Some(found) = find_node_mut(child, id) {
            return Some(found);
        }
    }
    None
}
