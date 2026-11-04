//! A note: the `notes` table (migrations/0001_create_notes.sql).

use rust_web_server::Model;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, Model)]
#[table(name = "notes")]
pub struct Note {
    #[primary_key(auto_increment)]
    pub id: i64,
    pub text: String,
}
