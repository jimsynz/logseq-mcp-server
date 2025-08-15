use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Clone)]
pub struct LogSeqClient {
    base_url: String,
    token: String,
    client: reqwest::Client,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Page {
    pub name: String,
    pub uuid: String,
    #[serde(rename = "original-name")]
    pub original_name: Option<String>,
    pub properties: Option<HashMap<String, Value>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Block {
    pub uuid: String,
    pub content: String,
    #[serde(default)]
    pub page: Option<PageRef>,
    #[serde(default)]
    pub properties: Option<HashMap<String, Value>>,
    #[serde(default)]
    pub children: Vec<Block>,
    #[serde(default)]
    pub level: Option<u32>,
    #[serde(default)]
    pub format: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PageRef {
    pub id: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SearchResult {
    pub block: Block,
    pub score: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct InsertBlockOptions {
    pub parent: Option<String>,
    pub sibling: Option<String>,
    pub before: Option<bool>,
    pub properties: Option<HashMap<String, Value>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TodoItem {
    pub uuid: String,
    pub content: String,
    pub marker: String,
    pub page_name: String,
    pub priority: Option<String>,
}

impl LogSeqClient {
    pub fn new(base_url: &str, token: &str) -> Result<Self> {
        Ok(Self {
            base_url: base_url.to_string(),
            token: token.to_string(),
            client: reqwest::Client::new(),
        })
    }

    async fn call_api(&self, method: &str, args: Vec<Value>) -> Result<Value> {
        tracing::debug!(
            "Making API call to {} with method: {}",
            self.base_url,
            method
        );

        let response = self
            .client
            .post(format!("{}/api", self.base_url))
            .header("Authorization", format!("Bearer {}", self.token))
            .json(&serde_json::json!({
                "method": method,
                "args": args
            }))
            .send()
            .await?;

        let status = response.status();
        if status.is_success() {
            Ok(response.json().await?)
        } else {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            tracing::error!("API call failed with status {}: {}", status, error_text);
            Err(anyhow::anyhow!(
                "API call failed: {} - {}",
                status,
                error_text
            ))
        }
    }

    pub async fn get_all_pages(&self) -> Result<Vec<Page>> {
        let result = self.call_api("logseq.Editor.getAllPages", vec![]).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn get_page(&self, name_or_uuid: &str) -> Result<Page> {
        let result = self
            .call_api("logseq.Editor.getPage", vec![name_or_uuid.into()])
            .await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn create_page(
        &self,
        name: &str,
        properties: Option<HashMap<String, Value>>,
    ) -> Result<Page> {
        let args = vec![
            name.into(),
            serde_json::to_value(properties).unwrap_or(Value::Null),
        ];
        let result = self.call_api("logseq.Editor.createPage", args).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn get_page_blocks_tree(&self, page_name_or_uuid: &str) -> Result<Vec<Block>> {
        let result = self
            .call_api(
                "logseq.Editor.getPageBlocksTree",
                vec![page_name_or_uuid.into()],
            )
            .await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn insert_block(&self, content: &str, opts: InsertBlockOptions) -> Result<Block> {
        let properties_backup = opts.properties.clone();
        let args = vec![content.into(), serde_json::to_value(opts)?];
        tracing::debug!("insert_block args: {:?}", args);
        let result = self.call_api("logseq.Editor.insertBlock", args).await?;
        tracing::debug!("insert_block result: {:?}", result);

        // The LogSeq API can return different formats depending on success/failure
        // Let's handle all possible return types more gracefully

        if result.is_null() {
            return Err(anyhow::anyhow!(
                "insertBlock returned null - block creation may have failed"
            ));
        }

        // Try to extract UUID from various possible response formats
        let uuid_to_fetch = if let Some(uuid_value) = result.get("uuid") {
            // Response has a uuid field
            uuid_value.as_str().map(String::from)
        } else if let Some(uuid_str) = result.as_str() {
            // Response is directly a UUID string
            Some(uuid_str.to_string())
        } else if let Ok(block) = serde_json::from_value::<Block>(result.clone()) {
            // Response is already a complete Block object
            return Ok(block);
        } else {
            None
        };

        if let Some(uuid) = uuid_to_fetch {
            tracing::debug!("Fetching block details for UUID: {}", uuid);
            match self.get_block(&uuid).await {
                Ok(block) => Ok(block),
                Err(e) => {
                    tracing::warn!("Failed to fetch block details for {}: {}", uuid, e);
                    // Return a minimal block if we can't fetch details
                    Ok(Block {
                        uuid,
                        content: content.to_string(),
                        page: None,
                        properties: properties_backup,
                        children: vec![],
                        level: None,
                        format: None,
                    })
                }
            }
        } else {
            Err(anyhow::anyhow!(
                "Unexpected insertBlock response format: {}",
                serde_json::to_string_pretty(&result)
                    .unwrap_or_else(|_| "<unparseable>".to_string())
            ))
        }
    }

    pub async fn update_block(
        &self,
        uuid: &str,
        content: &str,
        properties: Option<HashMap<String, Value>>,
    ) -> Result<Block> {
        let mut args = vec![uuid.into(), content.into()];
        if let Some(props) = properties {
            args.push(serde_json::to_value(props)?);
        }
        tracing::debug!("update_block args: {:?}", args);
        let result = self.call_api("logseq.Editor.updateBlock", args).await?;
        tracing::debug!("update_block result: {:?}", result);

        // If the API returns null, fetch the updated block instead
        if result.is_null() {
            self.get_block(uuid).await
        } else {
            Ok(serde_json::from_value(result)?)
        }
    }

    pub async fn search(&self, query: &str) -> Result<Vec<SearchResult>> {
        // Use DataScript to search for blocks containing the query text
        let datascript_query = format!(
            "[:find ?uuid ?content :where [?b :block/uuid ?uuid] [?b :block/content ?content] [(clojure.string/includes? ?content \"{}\")]]",
            query.replace('"', "\\\"")
        );

        let result = self
            .call_api("logseq.DB.datascriptQuery", vec![datascript_query.into()])
            .await?;
        tracing::debug!("Search DataScript result: {:?}", result);

        // Convert the DataScript result to SearchResult format
        let mut search_results = Vec::new();

        if let Some(results_array) = result.as_array() {
            for result_row in results_array {
                if let Some(row) = result_row.as_array()
                    && row.len() >= 2
                    && let (Some(uuid), Some(content)) = (
                        row[0].as_str().map(String::from),
                        row[1].as_str().map(String::from),
                    )
                {
                    let block = Block {
                        uuid,
                        content,
                        page: None, // We don't have page info from this query
                        properties: None,
                        children: vec![],
                        level: None,
                        format: None,
                    };
                    search_results.push(SearchResult {
                        block,
                        score: None, // DataScript doesn't provide scoring
                    });
                }
            }
        }

        Ok(search_results)
    }

    // New Editor methods
    pub async fn get_block(&self, uuid: &str) -> Result<Block> {
        let result = self
            .call_api("logseq.Editor.getBlock", vec![uuid.into()])
            .await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn get_current_page(&self) -> Result<Page> {
        let result = self
            .call_api("logseq.Editor.getCurrentPage", vec![])
            .await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn get_current_block(&self) -> Result<Block> {
        let result = self
            .call_api("logseq.Editor.getCurrentBlock", vec![])
            .await?;
        Ok(serde_json::from_value(result)?)
    }

    // Database methods
    pub async fn datascript_query(&self, query: &str) -> Result<Value> {
        let result = self
            .call_api("logseq.DB.datascriptQuery", vec![query.into()])
            .await?;
        Ok(result)
    }

    // App methods
    pub async fn get_current_graph(&self) -> Result<Value> {
        let result = self.call_api("logseq.App.getCurrentGraph", vec![]).await?;
        Ok(result)
    }

    pub async fn get_state_from_store(&self, key: &str) -> Result<Value> {
        let result = self
            .call_api("logseq.App.getStateFromStore", vec![key.into()])
            .await?;
        Ok(result)
    }

    pub async fn get_user_configs(&self) -> Result<Value> {
        let result = self.call_api("logseq.App.getUserConfigs", vec![]).await?;
        Ok(result)
    }

    // Delete operations
    pub async fn remove_block(&self, block_uuid: &str) -> Result<()> {
        let result = self
            .call_api("logseq.Editor.removeBlock", vec![block_uuid.into()])
            .await?;
        tracing::debug!("remove_block result: {:?}", result);

        // The API should return null/void on success
        if result.is_null() {
            Ok(())
        } else {
            // Check if there's an error in the response
            if let Some(error) = result.get("error") {
                Err(anyhow::anyhow!("Failed to remove block: {}", error))
            } else {
                Ok(()) // Assume success if no explicit error
            }
        }
    }

    pub async fn delete_page(&self, page_name: &str) -> Result<()> {
        let result = self
            .call_api("logseq.Editor.deletePage", vec![page_name.into()])
            .await?;
        tracing::debug!("delete_page result: {:?}", result);

        // The API should return null/void on success
        if result.is_null() {
            Ok(())
        } else {
            // Check if there's an error in the response
            if let Some(error) = result.get("error") {
                Err(anyhow::anyhow!("Failed to delete page: {}", error))
            } else {
                Ok(()) // Assume success if no explicit error
            }
        }
    }

    // Search for incomplete todos across all pages
    pub async fn find_incomplete_todos(&self) -> Result<Vec<TodoItem>> {
        // Use DataScript query to find all incomplete todos
        // Based on LogSeq docs, incomplete todos are marked as TODO, DOING, LATER, NOW
        let datascript_query = r#"[:find ?uuid ?content ?marker ?page-name
            :where 
            [?b :block/uuid ?uuid]
            [?b :block/content ?content]
            [?b :block/marker ?marker]
            [?b :block/page ?p]
            [?p :block/name ?page-name]
            [(contains? #{"TODO" "DOING" "LATER" "NOW" "WAITING"} ?marker)]]"#;

        let result = self
            .call_api("logseq.DB.datascriptQuery", vec![datascript_query.into()])
            .await?;
        tracing::debug!("find_incomplete_todos DataScript result: {:?}", result);

        // Convert the DataScript result to TodoItem format
        let mut todos = Vec::new();

        if let Some(results_array) = result.as_array() {
            for result_row in results_array {
                if let Some(row) = result_row.as_array()
                    && row.len() >= 4
                    && let (Some(uuid), Some(content), Some(marker), Some(page_name)) = (
                        row[0].as_str().map(String::from),
                        row[1].as_str().map(String::from),
                        row[2].as_str().map(String::from),
                        row[3].as_str().map(String::from),
                    )
                {
                    todos.push(TodoItem {
                        uuid,
                        content,
                        marker,
                        page_name,
                        priority: None, // Could be extended to include priority
                    });
                }
            }
        }

        Ok(todos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let result = LogSeqClient::new("http://localhost:12315", "test-token");
        assert!(
            result.is_ok(),
            "Client creation should succeed with valid URL and token"
        );

        let client = result.unwrap();
        assert_eq!(client.base_url, "http://localhost:12315");
        assert_eq!(client.token, "test-token");
    }

    #[test]
    fn test_insert_block_options_default() {
        let opts = InsertBlockOptions::default();
        assert!(opts.parent.is_none());
        assert!(opts.sibling.is_none());
        assert!(opts.before.is_none());
        assert!(opts.properties.is_none());
    }

    #[test]
    fn test_block_structure() {
        // Test that we can create block structures correctly
        let block = Block {
            uuid: "test-uuid".to_string(),
            content: "test content".to_string(),
            page: Some(PageRef { id: 123 }),
            properties: None,
            children: vec![],
            level: Some(1),
            format: Some("markdown".to_string()),
        };

        assert_eq!(block.uuid, "test-uuid");
        assert_eq!(block.content, "test content");
        assert_eq!(block.level, Some(1));
        assert!(block.children.is_empty());
    }
}
