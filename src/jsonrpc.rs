use serde::{Deserialize, Serialize};
use serde_json::Value;

/// JSONRPC 2.0 Request message
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Request {
    /// JSONRPC version - MUST be exactly "2.0"
    pub jsonrpc: String,
    
    /// Method name to be invoked
    pub method: String,
    
    /// Parameters for the method (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
    
    /// Request identifier (optional for notifications)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
}

/// JSONRPC 2.0 Response message
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Response {
    /// JSONRPC version - MUST be exactly "2.0"
    pub jsonrpc: String,
    
    /// Result value (required on success)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    
    /// Error object (required on error)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorObject>,
    
    /// Request identifier from the original request
    pub id: Value,
}

/// JSONRPC 2.0 Error object
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ErrorObject {
    /// Error type indicator (must be integer)
    pub code: i32,
    
    /// Short error description
    pub message: String,
    
    /// Additional error information (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// Pre-defined JSONRPC error codes
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ErrorCode {
    ParseError = -32700,
    InvalidRequest = -32600,
    MethodNotFound = -32601,
    InvalidParams = -32602,
    InternalError = -32603,
}

impl ErrorCode {
    pub fn message(&self) -> &'static str {
        match self {
            ErrorCode::ParseError => "Parse error",
            ErrorCode::InvalidRequest => "Invalid Request",
            ErrorCode::MethodNotFound => "Method not found",
            ErrorCode::InvalidParams => "Invalid params",
            ErrorCode::InternalError => "Internal error",
        }
    }
}

/// JSONRPC message (either Request or Response)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum Message {
    Request(Request),
    Response(Response),
}

/// Batch of JSONRPC messages
pub type Batch = Vec<Message>;

impl Request {
    /// Create a new request
    pub fn new(method: String, params: Option<Value>, id: Option<Value>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            method,
            params,
            id,
        }
    }
    
    /// Check if this is a notification (no id)
    pub fn is_notification(&self) -> bool {
        self.id.is_none()
    }
}

impl Response {
    /// Create a success response
    pub fn success(result: Value, id: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: Some(result),
            error: None,
            id,
        }
    }
    
    /// Create an error response
    pub fn error(error: ErrorObject, id: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(error),
            id,
        }
    }
}

impl ErrorObject {
    /// Create a new error object
    pub fn new(code: ErrorCode, data: Option<Value>) -> Self {
        Self {
            code: code as i32,
            message: code.message().to_string(),
            data,
        }
    }
    
    /// Create a custom error
    pub fn custom(code: i32, message: String, data: Option<Value>) -> Self {
        Self { code, message, data }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_request_serialization() {
        let request = Request::new(
            "subtract".to_string(),
            Some(json!([42, 23])),
            Some(json!(1)),
        );
        
        let json = serde_json::to_string(&request).unwrap();
        let expected = r#"{"jsonrpc":"2.0","method":"subtract","params":[42,23],"id":1}"#;
        assert_eq!(json, expected);
    }
    
    #[test]
    fn test_notification() {
        let notification = Request::new(
            "update".to_string(),
            Some(json!([1, 2, 3, 4, 5])),
            None,
        );
        
        assert!(notification.is_notification());
        
        let json = serde_json::to_string(&notification).unwrap();
        let expected = r#"{"jsonrpc":"2.0","method":"update","params":[1,2,3,4,5]}"#;
        assert_eq!(json, expected);
    }
    
    #[test]
    fn test_response_success() {
        let response = Response::success(json!(19), json!(1));
        
        let json = serde_json::to_string(&response).unwrap();
        let expected = r#"{"jsonrpc":"2.0","result":19,"id":1}"#;
        assert_eq!(json, expected);
    }
    
    #[test]
    fn test_response_error() {
        let error = ErrorObject::new(ErrorCode::MethodNotFound, None);
        let response = Response::error(error, json!("1"));
        
        let json = serde_json::to_string(&response).unwrap();
        let expected = r#"{"jsonrpc":"2.0","error":{"code":-32601,"message":"Method not found"},"id":"1"}"#;
        assert_eq!(json, expected);
    }
}