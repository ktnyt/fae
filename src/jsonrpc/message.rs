use serde_json::Value;

#[derive(Debug, Clone)]
pub struct JsonRpcRequest {
    pub id: u64,
    pub method: String,
    pub params: Option<Value>,
}

#[derive(Debug, Clone)]
pub struct JsonRpcNotification {
    pub method: String,
    pub params: Option<Value>,
}

#[derive(Debug, Clone)]
pub struct JsonRpcResponse {
    pub id: u64,
    pub result: Option<Value>,
    pub error: Option<JsonRpcError>,
}

/// JSON-RPC 2.0 仕様に準拠したエラーコード定数
/// https://www.jsonrpc.org/specification#error_object
pub mod error_codes {
    /// Parse error - Invalid JSON was received by the server
    pub const PARSE_ERROR: i32 = -32700;

    /// Invalid Request - The JSON sent is not a valid Request object
    pub const INVALID_REQUEST: i32 = -32600;

    /// Method not found - The method does not exist / is not available
    pub const METHOD_NOT_FOUND: i32 = -32601;

    /// Invalid params - Invalid method parameter(s)
    pub const INVALID_PARAMS: i32 = -32602;

    /// Internal error - Internal JSON-RPC error
    pub const INTERNAL_ERROR: i32 = -32603;

    // Server error codes (-32000 to -32099) reserved for implementation-defined server-errors
    /// Server error range start
    pub const SERVER_ERROR_START: i32 = -32099;

    /// Server error range end
    pub const SERVER_ERROR_END: i32 = -32000;
}

/// JSON-RPC 2.0 定義済みエラーコード列挙型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonRpcErrorCode {
    /// Parse error (-32700)
    ParseError,
    /// Invalid Request (-32600)
    InvalidRequest,
    /// Method not found (-32601)
    MethodNotFound,
    /// Invalid params (-32602)
    InvalidParams,
    /// Internal error (-32603)
    InternalError,
    /// Server error (-32000 to -32099)
    ServerError(i32),
    /// Application-specific error (outside standard range)
    ApplicationError(i32),
}

impl JsonRpcErrorCode {
    /// エラーコードから数値を取得
    pub fn to_code(self) -> i32 {
        match self {
            JsonRpcErrorCode::ParseError => error_codes::PARSE_ERROR,
            JsonRpcErrorCode::InvalidRequest => error_codes::INVALID_REQUEST,
            JsonRpcErrorCode::MethodNotFound => error_codes::METHOD_NOT_FOUND,
            JsonRpcErrorCode::InvalidParams => error_codes::INVALID_PARAMS,
            JsonRpcErrorCode::InternalError => error_codes::INTERNAL_ERROR,
            JsonRpcErrorCode::ServerError(code) => code,
            JsonRpcErrorCode::ApplicationError(code) => code,
        }
    }

    /// 数値からエラーコードを作成
    pub fn from_code(code: i32) -> Self {
        match code {
            error_codes::PARSE_ERROR => JsonRpcErrorCode::ParseError,
            error_codes::INVALID_REQUEST => JsonRpcErrorCode::InvalidRequest,
            error_codes::METHOD_NOT_FOUND => JsonRpcErrorCode::MethodNotFound,
            error_codes::INVALID_PARAMS => JsonRpcErrorCode::InvalidParams,
            error_codes::INTERNAL_ERROR => JsonRpcErrorCode::InternalError,
            code if code <= error_codes::SERVER_ERROR_END
                && code >= error_codes::SERVER_ERROR_START =>
            {
                JsonRpcErrorCode::ServerError(code)
            }
            code => JsonRpcErrorCode::ApplicationError(code),
        }
    }

    /// デフォルトエラーメッセージを取得
    pub fn default_message(self) -> &'static str {
        match self {
            JsonRpcErrorCode::ParseError => "Parse error",
            JsonRpcErrorCode::InvalidRequest => "Invalid Request",
            JsonRpcErrorCode::MethodNotFound => "Method not found",
            JsonRpcErrorCode::InvalidParams => "Invalid params",
            JsonRpcErrorCode::InternalError => "Internal error",
            JsonRpcErrorCode::ServerError(_) => "Server error",
            JsonRpcErrorCode::ApplicationError(_) => "Application error",
        }
    }
}

#[derive(Debug, Clone)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    pub data: Option<Value>,
}

impl JsonRpcError {
    /// 新しいエラーを作成
    pub fn new(code: i32, message: impl Into<String>, data: Option<Value>) -> Self {
        Self {
            code,
            message: message.into(),
            data,
        }
    }

    /// エラーコード列挙型から新しいエラーを作成
    pub fn from_error_code(
        error_code: JsonRpcErrorCode,
        message: Option<impl Into<String>>,
        data: Option<Value>,
    ) -> Self {
        let msg = match message {
            Some(m) => m.into(),
            None => error_code.default_message().to_string(),
        };

        Self {
            code: error_code.to_code(),
            message: msg,
            data,
        }
    }

    /// Parse errorを作成
    pub fn parse_error(message: Option<impl Into<String>>, data: Option<Value>) -> Self {
        Self::from_error_code(JsonRpcErrorCode::ParseError, message, data)
    }

    /// Invalid Requestエラーを作成
    pub fn invalid_request(message: Option<impl Into<String>>, data: Option<Value>) -> Self {
        Self::from_error_code(JsonRpcErrorCode::InvalidRequest, message, data)
    }

    /// Method not foundエラーを作成
    pub fn method_not_found(message: Option<impl Into<String>>, data: Option<Value>) -> Self {
        Self::from_error_code(JsonRpcErrorCode::MethodNotFound, message, data)
    }

    /// Invalid paramsエラーを作成
    pub fn invalid_params(message: Option<impl Into<String>>, data: Option<Value>) -> Self {
        Self::from_error_code(JsonRpcErrorCode::InvalidParams, message, data)
    }

    /// Internal errorを作成
    pub fn internal_error(message: Option<impl Into<String>>, data: Option<Value>) -> Self {
        Self::from_error_code(JsonRpcErrorCode::InternalError, message, data)
    }

    /// Server errorを作成
    pub fn server_error(
        code: i32,
        message: Option<impl Into<String>>,
        data: Option<Value>,
    ) -> Self {
        if code > error_codes::SERVER_ERROR_END || code < error_codes::SERVER_ERROR_START {
            panic!(
                "Server error code must be between {} and {}",
                error_codes::SERVER_ERROR_START,
                error_codes::SERVER_ERROR_END
            );
        }
        Self::from_error_code(JsonRpcErrorCode::ServerError(code), message, data)
    }

    /// エラーコード列挙型を取得
    pub fn error_code(&self) -> JsonRpcErrorCode {
        JsonRpcErrorCode::from_code(self.code)
    }

    /// 標準のエラーコードかどうかを判定
    pub fn is_standard_error(&self) -> bool {
        matches!(
            self.error_code(),
            JsonRpcErrorCode::ParseError
                | JsonRpcErrorCode::InvalidRequest
                | JsonRpcErrorCode::MethodNotFound
                | JsonRpcErrorCode::InvalidParams
                | JsonRpcErrorCode::InternalError
        )
    }

    /// サーバーエラーかどうかを判定
    pub fn is_server_error(&self) -> bool {
        matches!(self.error_code(), JsonRpcErrorCode::ServerError(_))
    }
}

#[derive(Debug, Clone)]
pub enum JsonRpcPayload {
    Request(JsonRpcRequest),
    Notification(JsonRpcNotification),
    Response(JsonRpcResponse),
}

#[derive(Debug, Clone)]
pub enum JsonRpcSendError {
    ChannelClosed,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_error_code_constants() {
        // JSON-RPC 2.0仕様のエラーコードが正しく定義されているかテスト
        assert_eq!(error_codes::PARSE_ERROR, -32700);
        assert_eq!(error_codes::INVALID_REQUEST, -32600);
        assert_eq!(error_codes::METHOD_NOT_FOUND, -32601);
        assert_eq!(error_codes::INVALID_PARAMS, -32602);
        assert_eq!(error_codes::INTERNAL_ERROR, -32603);
        assert_eq!(error_codes::SERVER_ERROR_START, -32099);
        assert_eq!(error_codes::SERVER_ERROR_END, -32000);
    }

    #[test]
    fn test_error_code_enum_to_code() {
        assert_eq!(JsonRpcErrorCode::ParseError.to_code(), -32700);
        assert_eq!(JsonRpcErrorCode::InvalidRequest.to_code(), -32600);
        assert_eq!(JsonRpcErrorCode::MethodNotFound.to_code(), -32601);
        assert_eq!(JsonRpcErrorCode::InvalidParams.to_code(), -32602);
        assert_eq!(JsonRpcErrorCode::InternalError.to_code(), -32603);
        assert_eq!(JsonRpcErrorCode::ServerError(-32001).to_code(), -32001);
        assert_eq!(JsonRpcErrorCode::ApplicationError(1000).to_code(), 1000);
    }

    #[test]
    fn test_error_code_enum_from_code() {
        assert_eq!(
            JsonRpcErrorCode::from_code(-32700),
            JsonRpcErrorCode::ParseError
        );
        assert_eq!(
            JsonRpcErrorCode::from_code(-32600),
            JsonRpcErrorCode::InvalidRequest
        );
        assert_eq!(
            JsonRpcErrorCode::from_code(-32601),
            JsonRpcErrorCode::MethodNotFound
        );
        assert_eq!(
            JsonRpcErrorCode::from_code(-32602),
            JsonRpcErrorCode::InvalidParams
        );
        assert_eq!(
            JsonRpcErrorCode::from_code(-32603),
            JsonRpcErrorCode::InternalError
        );
        assert_eq!(
            JsonRpcErrorCode::from_code(-32001),
            JsonRpcErrorCode::ServerError(-32001)
        );
        assert_eq!(
            JsonRpcErrorCode::from_code(1000),
            JsonRpcErrorCode::ApplicationError(1000)
        );
    }

    #[test]
    fn test_error_code_default_messages() {
        assert_eq!(
            JsonRpcErrorCode::ParseError.default_message(),
            "Parse error"
        );
        assert_eq!(
            JsonRpcErrorCode::InvalidRequest.default_message(),
            "Invalid Request"
        );
        assert_eq!(
            JsonRpcErrorCode::MethodNotFound.default_message(),
            "Method not found"
        );
        assert_eq!(
            JsonRpcErrorCode::InvalidParams.default_message(),
            "Invalid params"
        );
        assert_eq!(
            JsonRpcErrorCode::InternalError.default_message(),
            "Internal error"
        );
        assert_eq!(
            JsonRpcErrorCode::ServerError(-32001).default_message(),
            "Server error"
        );
        assert_eq!(
            JsonRpcErrorCode::ApplicationError(1000).default_message(),
            "Application error"
        );
    }

    #[test]
    fn test_json_rpc_error_new() {
        let error = JsonRpcError::new(-32700, "Custom parse error", Some(json!({"line": 5})));
        assert_eq!(error.code, -32700);
        assert_eq!(error.message, "Custom parse error");
        assert_eq!(error.data, Some(json!({"line": 5})));
    }

    #[test]
    fn test_json_rpc_error_from_error_code() {
        // デフォルトメッセージを使用
        let error1 =
            JsonRpcError::from_error_code(JsonRpcErrorCode::ParseError, None::<String>, None);
        assert_eq!(error1.code, -32700);
        assert_eq!(error1.message, "Parse error");
        assert_eq!(error1.data, None);

        // カスタムメッセージを使用
        let error2 = JsonRpcError::from_error_code(
            JsonRpcErrorCode::MethodNotFound,
            Some("Custom method not found"),
            Some(json!({"method": "unknown_method"})),
        );
        assert_eq!(error2.code, -32601);
        assert_eq!(error2.message, "Custom method not found");
        assert_eq!(error2.data, Some(json!({"method": "unknown_method"})));
    }

    #[test]
    fn test_json_rpc_error_convenience_methods() {
        // Parse error
        let parse_error =
            JsonRpcError::parse_error(Some("Invalid JSON syntax"), Some(json!({"position": 42})));
        assert_eq!(parse_error.code, -32700);
        assert_eq!(parse_error.message, "Invalid JSON syntax");
        assert_eq!(parse_error.data, Some(json!({"position": 42})));

        // Invalid request
        let invalid_request = JsonRpcError::invalid_request(None::<String>, None);
        assert_eq!(invalid_request.code, -32600);
        assert_eq!(invalid_request.message, "Invalid Request");
        assert_eq!(invalid_request.data, None);

        // Method not found
        let method_not_found =
            JsonRpcError::method_not_found(Some("Method 'ping' not found"), None);
        assert_eq!(method_not_found.code, -32601);
        assert_eq!(method_not_found.message, "Method 'ping' not found");

        // Invalid params
        let invalid_params = JsonRpcError::invalid_params(Some("Expected array, got object"), None);
        assert_eq!(invalid_params.code, -32602);
        assert_eq!(invalid_params.message, "Expected array, got object");

        // Internal error
        let internal_error = JsonRpcError::internal_error(Some("Database connection failed"), None);
        assert_eq!(internal_error.code, -32603);
        assert_eq!(internal_error.message, "Database connection failed");
    }

    #[test]
    fn test_json_rpc_error_server_error() {
        let server_error = JsonRpcError::server_error(-32001, Some("Custom server error"), None);
        assert_eq!(server_error.code, -32001);
        assert_eq!(server_error.message, "Custom server error");
        assert!(server_error.is_server_error());
    }

    #[test]
    #[should_panic(expected = "Server error code must be between -32099 and -32000")]
    fn test_json_rpc_error_server_error_invalid_code() {
        JsonRpcError::server_error(-31999, Some("Invalid server error code"), None);
    }

    #[test]
    fn test_json_rpc_error_error_code_conversion() {
        let error = JsonRpcError::parse_error(None::<String>, None);
        assert_eq!(error.error_code(), JsonRpcErrorCode::ParseError);

        let server_error = JsonRpcError::server_error(-32010, None::<String>, None);
        assert_eq!(
            server_error.error_code(),
            JsonRpcErrorCode::ServerError(-32010)
        );
    }

    #[test]
    fn test_json_rpc_error_type_checking() {
        let parse_error = JsonRpcError::parse_error(None::<String>, None);
        assert!(parse_error.is_standard_error());
        assert!(!parse_error.is_server_error());

        let method_not_found = JsonRpcError::method_not_found(None::<String>, None);
        assert!(method_not_found.is_standard_error());
        assert!(!method_not_found.is_server_error());

        let server_error = JsonRpcError::server_error(-32001, None::<String>, None);
        assert!(!server_error.is_standard_error());
        assert!(server_error.is_server_error());

        let app_error = JsonRpcError::new(1000, "Application error", None);
        assert!(!app_error.is_standard_error());
        assert!(!app_error.is_server_error());
    }

    #[test]
    fn test_server_error_range_validation() {
        // 有効なサーバーエラーコード範囲
        for code in -32099..=-32000 {
            let error_code = JsonRpcErrorCode::from_code(code);
            assert_eq!(error_code, JsonRpcErrorCode::ServerError(code));
        }

        // 範囲外のコードはApplicationErrorになる
        assert_eq!(
            JsonRpcErrorCode::from_code(-31999),
            JsonRpcErrorCode::ApplicationError(-31999)
        );
        assert_eq!(
            JsonRpcErrorCode::from_code(-32100),
            JsonRpcErrorCode::ApplicationError(-32100)
        );
    }

    #[test]
    fn test_error_code_round_trip() {
        let test_codes = vec![
            JsonRpcErrorCode::ParseError,
            JsonRpcErrorCode::InvalidRequest,
            JsonRpcErrorCode::MethodNotFound,
            JsonRpcErrorCode::InvalidParams,
            JsonRpcErrorCode::InternalError,
            JsonRpcErrorCode::ServerError(-32001),
            JsonRpcErrorCode::ApplicationError(1000),
        ];

        for original_code in test_codes {
            let numeric = original_code.to_code();
            let reconstructed = JsonRpcErrorCode::from_code(numeric);
            assert_eq!(original_code, reconstructed);
        }
    }
}
