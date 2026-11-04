//! `add_note`, `search_notes` and the `notes://{id}` resource.

use std::sync::Arc;

use rust_web_server::mcp::{McpContent, McpServer};
use rust_web_server::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::Store;

/// A note.
#[derive(Clone, Serialize, Deserialize, JsonSchema)]
pub struct Note {
    pub id: usize,
    pub text: String,
}

/// What to remember.
#[derive(Deserialize, JsonSchema)]
pub struct AddNote {
    /// The note's text.
    pub text: String,
}

/// What to look for.
#[derive(Deserialize, JsonSchema)]
pub struct Search {
    /// Words the note must contain (case-insensitive).
    pub query: String,
}

/// The matching notes.
#[derive(Serialize, JsonSchema)]
pub struct Notes {
    pub notes: Vec<Note>,
}

/// Add the notes tools and resource to `server`.
pub fn register(server: McpServer, store: Store) -> McpServer {
    let (add, search, read) = (Arc::clone(&store), Arc::clone(&store), store);
    server
        .typed_tool("add_note", "Remember a note; returns it with its id.", move |_, args: AddNote| {
            let mut notes = add.lock().unwrap();
            let note = Note { id: notes.len() + 1, text: args.text };
            notes.push(note.clone());
            Ok(note)
        })
        .typed_tool("search_notes", "Find notes containing the query.", move |_, args: Search| {
            let query = args.query.to_lowercase();
            let notes = search.lock().unwrap().iter().filter(|n| n.text.to_lowercase().contains(&query)).cloned().collect();
            Ok(Notes { notes })
        })
        .resource("notes://{id}", "note", "One note by id.", move |uri| {
            let id: usize = uri.trim_start_matches("notes://").parse().map_err(|_| format!("not a note: {uri}"))?;
            let notes = read.lock().unwrap();
            let note = notes.iter().find(|n| n.id == id).ok_or_else(|| format!("no note {id}"))?;
            Ok(McpContent::text(note.text.clone()))
        })
}
