use std::process::ExitCode;

/// Process outcome: success exits 0, Fail 1, Interrupted 130.
pub(crate) enum AppError {
    Interrupted,
    Fail(anyhow::Error),
}

impl From<anyhow::Error> for AppError {
    fn from(err: anyhow::Error) -> Self {
        Self::Fail(err)
    }
}

pub(crate) fn to_exit_code(result: Result<(), AppError>) -> ExitCode {
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(AppError::Fail(err)) => {
            eprintln!("{err:#}");
            ExitCode::FAILURE
        }
        Err(AppError::Interrupted) => ExitCode::from(130),
    }
}
