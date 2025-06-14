use std::path::PathBuf;

use crate::jsonrpc::handler::JsonRpcHandler;
use super::literal_search::LiteralSearchHandler;

/// 利用可能なサービスタイプ
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceType {
    /// リテラル検索サービス
    LiteralSearch,
    // 将来追加予定
    // SymbolSearch,
    // FileSearch,
    // RegexSearch,
    // GitSearch,
}

impl ServiceType {
    /// 文字列からServiceTypeを解析
    pub fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "search:literal" => Ok(ServiceType::LiteralSearch),
            // 将来追加予定
            // "search:symbol" => Ok(ServiceType::SymbolSearch),
            // "search:file" => Ok(ServiceType::FileSearch),
            // "search:regex" => Ok(ServiceType::RegexSearch),
            // "search:git" => Ok(ServiceType::GitSearch),
            _ => Err(format!("Unknown service type: {}", s)),
        }
    }

    /// ServiceTypeを文字列に変換
    pub fn to_string(&self) -> &'static str {
        match self {
            ServiceType::LiteralSearch => "search:literal",
            // 将来追加予定
            // ServiceType::SymbolSearch => "search:symbol",
            // ServiceType::FileSearch => "search:file",
            // ServiceType::RegexSearch => "search:regex",
            // ServiceType::GitSearch => "search:git",
        }
    }

    /// 利用可能なサービスタイプのリストを取得
    pub fn available_services() -> Vec<&'static str> {
        vec![
            "search:literal",
            // 将来追加予定
            // "search:symbol",
            // "search:file", 
            // "search:regex",
            // "search:git",
        ]
    }

    /// サービスの説明を取得
    pub fn description(&self) -> &'static str {
        match self {
            ServiceType::LiteralSearch => "ripgrepを使用した高速リテラル検索サービス",
            // 将来追加予定
            // ServiceType::SymbolSearch => "Tree-sitterを使用したシンボル検索サービス",
            // ServiceType::FileSearch => "ファイル名・パス検索サービス",
            // ServiceType::RegexSearch => "正規表現検索サービス",
            // ServiceType::GitSearch => "Git履歴統合検索サービス",
        }
    }
}

/// サービスファクトリ - 動的にサービスを作成
pub struct ServiceFactory;

impl ServiceFactory {
    /// 指定されたサービスタイプのハンドラーを作成（async版）
    /// 注意: 現在の実装では、main.rsで直接ハンドラーを作成します
    pub async fn create_handler_async(
        service_type: ServiceType,
        search_root: PathBuf,
    ) -> Result<Box<dyn JsonRpcHandler + Send>, String> {
        match service_type {
            ServiceType::LiteralSearch => {
                let handler = LiteralSearchHandler::new(search_root).await;
                Ok(Box::new(handler))
            }
            // 将来追加予定
            // ServiceType::SymbolSearch => {
            //     let handler = SymbolSearchHandler::new(search_root).await;
            //     Ok(Box::new(handler))
            // }
            // ServiceType::FileSearch => {
            //     let handler = FileSearchHandler::new(search_root).await;
            //     Ok(Box::new(handler))
            // }
            // ServiceType::RegexSearch => {
            //     let handler = RegexSearchHandler::new(search_root).await;
            //     Ok(Box::new(handler))
            // }
            // ServiceType::GitSearch => {
            //     let handler = GitSearchHandler::new(search_root).await;
            //     Ok(Box::new(handler))
            // }
        }
    }

    /// 利用可能なサービス一覧を表示用の文字列として取得
    pub fn list_services() -> String {
        let mut result = String::from("Available services:\n");
        
        for service_name in ServiceType::available_services() {
            if let Ok(service_type) = ServiceType::from_str(service_name) {
                result.push_str(&format!(
                    "  {:<20} - {}\n",
                    service_name,
                    service_type.description()
                ));
            }
        }
        
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_type_from_str() {
        assert_eq!(
            ServiceType::from_str("search:literal").unwrap(),
            ServiceType::LiteralSearch
        );
        
        assert!(ServiceType::from_str("unknown:service").is_err());
    }

    #[test]
    fn test_service_type_to_string() {
        assert_eq!(ServiceType::LiteralSearch.to_string(), "search:literal");
    }

    #[test]
    fn test_service_type_description() {
        assert!(!ServiceType::LiteralSearch.description().is_empty());
    }

    #[test]
    fn test_available_services() {
        let services = ServiceType::available_services();
        assert!(!services.is_empty());
        assert!(services.contains(&"search:literal"));
    }

    #[tokio::test]
    async fn test_create_literal_search_handler() {
        let search_root = PathBuf::from("/tmp");
        let result = ServiceFactory::create_handler_async(ServiceType::LiteralSearch, search_root).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_list_services() {
        let list = ServiceFactory::list_services();
        assert!(list.contains("Available services:"));
        assert!(list.contains("search:literal"));
        assert!(list.contains("ripgrep"));
    }

    #[test]
    fn test_service_name_parsing_round_trip() {
        let service_names = ServiceType::available_services();
        
        for &service_name in &service_names {
            let service_type = ServiceType::from_str(service_name).unwrap();
            assert_eq!(service_type.to_string(), service_name);
        }
    }
}