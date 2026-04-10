use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

use super::Content;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Knowledge {
    pub name: String,
    pub content: Content,
    pub created_at: NaiveDateTime,
}
