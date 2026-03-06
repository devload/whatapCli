use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolFile {
    #[serde(default)]
    pub file_name: Option<String>,
    #[serde(rename = "fileName", default)]
    pub file_name_alt: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub host: Option<String>,
    #[serde(rename = "fileSize", default)]
    pub file_size: Option<i64>,
    #[serde(rename = "uploadTime", default)]
    pub upload_time: Option<i64>,
}

impl SymbolFile {
    pub fn name(&self) -> &str {
        self.file_name
            .as_deref()
            .or(self.file_name_alt.as_deref())
            .unwrap_or("unknown")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolListResponse {
    #[serde(default)]
    pub data: Vec<SymbolFile>,
    #[serde(default)]
    pub records: Option<Vec<SymbolFile>>,
}

impl SymbolListResponse {
    pub fn files(&self) -> &[SymbolFile] {
        if !self.data.is_empty() {
            &self.data
        } else if let Some(records) = &self.records {
            records
        } else {
            &[]
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct UploadResponse {
    #[serde(default)]
    pub success: bool,
    #[serde(default)]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct DeleteResponse {
    #[serde(default)]
    pub success: bool,
    #[serde(default)]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SymbolType {
    Sourcemap,
    Proguard,
    Dsym,
}

impl SymbolType {
    pub fn name(&self) -> &str {
        match self {
            SymbolType::Sourcemap => "sourcemap",
            SymbolType::Proguard => "proguard",
            SymbolType::Dsym => "dsym",
        }
    }

    pub fn upload_path(&self) -> &str {
        match self {
            SymbolType::Sourcemap => "sourcemap/data/upload",
            SymbolType::Proguard => "proguard/data/upload",
            SymbolType::Dsym => "dsym/data/upload",
        }
    }

    pub fn list_path(&self) -> &str {
        match self {
            SymbolType::Sourcemap => "sourcemap/data/search/list",
            SymbolType::Proguard => "proguard/data/search/list",
            SymbolType::Dsym => "dsym/data/search/list",
        }
    }

    pub fn delete_path(&self) -> &str {
        match self {
            SymbolType::Sourcemap => "sourcemap/data/delete",
            SymbolType::Proguard => "proguard/data/delete",
            SymbolType::Dsym => "dsym/data/delete",
        }
    }

    pub fn max_file_size_mb(&self) -> u64 {
        match self {
            SymbolType::Sourcemap | SymbolType::Proguard => 50,
            SymbolType::Dsym => 100,
        }
    }

    pub fn max_files_per_upload(&self) -> usize {
        5
    }
}
