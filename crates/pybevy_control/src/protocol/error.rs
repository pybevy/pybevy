/// Error codes for control operations (JSON-RPC style)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ErrorCode {
    NotFound = -32001,
    Forbidden = -32003,
    NotSupported = -32601,
    InvalidParams = -32602,
    Internal = -32603,
}

impl ErrorCode {
    pub fn as_i32(self) -> i32 {
        self as i32
    }
}
/// Error type for control operations
#[derive(Debug)]
pub struct ControlError {
    pub code: ErrorCode,
    pub message: String,
}

impl ControlError {
    pub fn not_found(msg: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::NotFound,
            message: msg.into(),
        }
    }

    pub fn forbidden(msg: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::Forbidden,
            message: msg.into(),
        }
    }

    pub fn invalid_params(msg: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::InvalidParams,
            message: msg.into(),
        }
    }

    pub fn internal(msg: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::Internal,
            message: msg.into(),
        }
    }

    pub fn not_supported(operation: &str) -> Self {
        Self {
            code: ErrorCode::NotSupported,
            message: format!(
                "Operation '{}' is not supported by the current Python runtime backend",
                operation
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_error_not_found() {
        let err = ControlError::not_found("missing");
        assert_eq!(err.code, ErrorCode::NotFound);
        assert_eq!(err.message, "missing");
    }

    #[test]
    fn control_error_invalid_params() {
        let err = ControlError::invalid_params("bad input");
        assert_eq!(err.code, ErrorCode::InvalidParams);
        assert_eq!(err.message, "bad input");
    }

    #[test]
    fn control_error_internal() {
        let err = ControlError::internal("crash");
        assert_eq!(err.code, ErrorCode::Internal);
        assert_eq!(err.message, "crash");
    }
    #[test]
    fn control_error_debug_format() {
        let err = ControlError::not_found("test");
        let debug = format!("{:?}", err);
        assert!(debug.contains("NotFound"));
        assert!(debug.contains("test"));
    }
}
