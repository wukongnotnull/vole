//! `mo analyze --json` 对齐的类型。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AnalyzeOutput {
    pub path: String,
    pub overview: bool,
    pub entries: Vec<AnalyzeEntry>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub large_files: Vec<AnalyzeFileEntry>,
    pub total_size: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_files: Option<i64>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AnalyzeEntry {
    pub name: String,
    pub path: String,
    pub size: i64,
    pub is_dir: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub insight: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub cleanable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_access: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct AnalyzeFileEntry {
    pub name: String,
    pub path: String,
    pub size: i64,
}

fn is_false(v: &bool) -> bool {
    !*v
}
