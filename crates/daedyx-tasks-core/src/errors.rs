use thiserror::Error;

#[derive(Debug, Error)]
pub enum TaskError {
    #[error("Liste nicht gefunden")]
    ListNotFound,
    #[error("Abschnitt nicht gefunden")]
    SectionNotFound,
    #[error("Aufgabe nicht gefunden")]
    TaskNotFound,
    #[error("Eingabe ungültig: {0}")]
    Invalid(&'static str),
    #[error("Unteraufgaben dürfen keine eigenen Unteraufgaben haben")]
    TooDeep,
    #[error("Datenbank: {0}")]
    Db(#[from] rusqlite::Error),
}

impl TaskError {
    /// Stabiler Code für die Oberfläche (`tasks/errors.ts`, `de.json` `tasks.error.*`).
    pub fn code(&self) -> &'static str {
        match self {
            TaskError::ListNotFound => "list_not_found",
            TaskError::SectionNotFound => "section_not_found",
            TaskError::TaskNotFound => "task_not_found",
            TaskError::Invalid(_) => "invalid",
            TaskError::TooDeep => "too_deep",
            TaskError::Db(_) => "db",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_stable() {
        assert_eq!(TaskError::ListNotFound.code(), "list_not_found");
        assert_eq!(TaskError::SectionNotFound.code(), "section_not_found");
        assert_eq!(TaskError::TooDeep.code(), "too_deep");
        assert_eq!(TaskError::Invalid("x").code(), "invalid");
    }
}
