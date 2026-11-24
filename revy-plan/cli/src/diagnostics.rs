use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitClass {
    DiagnosticsFound,
    UsageError,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Diagnostic {
    pub code: &'static str,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct Diagnostics {
    #[serde(skip)]
    exit_class: ExitClass,
    diagnostics: Vec<Diagnostic>,
}

impl Diagnostics {
    pub const fn diagnostics_found() -> Self {
        Self {
            exit_class: ExitClass::DiagnosticsFound,
            diagnostics: Vec::new(),
        }
    }

    pub const fn usage_error() -> Self {
        Self {
            exit_class: ExitClass::UsageError,
            diagnostics: Vec::new(),
        }
    }

    pub fn push(
        &mut self,
        code: &'static str,
        path: impl Into<String>,
        message: impl Into<String>,
    ) {
        self.diagnostics.push(Diagnostic {
            code,
            path: path.into(),
            message: message.into(),
        });
    }

    pub const fn is_empty(&self) -> bool {
        self.diagnostics.is_empty()
    }

    pub fn as_slice(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    pub fn append(&mut self, other: &mut Self) {
        self.diagnostics.append(&mut other.diagnostics);
    }

    pub const fn exit_code(&self) -> u8 {
        match self.exit_class {
            ExitClass::DiagnosticsFound => 1,
            ExitClass::UsageError => 2,
        }
    }

    pub fn to_human(&self) -> String {
        self.diagnostics
            .iter()
            .map(|diagnostic| {
                if diagnostic.path.is_empty() {
                    format!("[{}] {}", diagnostic.code, diagnostic.message)
                } else {
                    format!(
                        "{} [{}] {}",
                        diagnostic.path, diagnostic.code, diagnostic.message
                    )
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({ "diagnostics": self.diagnostics })
    }
}

impl Default for Diagnostics {
    fn default() -> Self {
        Self::diagnostics_found()
    }
}
