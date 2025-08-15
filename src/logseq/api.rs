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

impl LogSeqClient {
    pub fn new(base_url: &str, token: &str) -> Result<Self> {
        Ok(Self {
            base_url: base_url.to_string(),
            token: token.to_string(),
            client: reqwest::Client::new(),
        })
    }

    async fn call_api(&self, method: &str, args: Vec<Value>) -> Result<Value> {
        tracing::debug!("Making API call to {} with method: {}", self.base_url, method);
        
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
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            tracing::error!("API call failed with status {}: {}", status, error_text);
            Err(anyhow::anyhow!("API call failed: {} - {}", status, error_text))
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
        let args = vec![content.into(), serde_json::to_value(opts)?];
        let result = self.call_api("logseq.Editor.insertBlock", args).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn update_block(&self, uuid: &str, content: &str, properties: Option<HashMap<String, Value>>) -> Result<Block> {
        let mut args = vec![uuid.into(), content.into()];
        if let Some(props) = properties {
            args.push(serde_json::json!({"properties": props}));
        }
        let result = self.call_api("logseq.Editor.updateBlock", args).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn search(&self, query: &str) -> Result<Vec<SearchResult>> {
        let result = self.call_api("logseq.Db.q", vec![query.into()]).await?;
        Ok(serde_json::from_value(result)?)
    }

    // New Editor methods
    pub async fn get_block(&self, uuid: &str) -> Result<Block> {
        let result = self.call_api("logseq.Editor.getBlock", vec![uuid.into()]).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn get_current_page(&self) -> Result<Page> {
        let result = self.call_api("logseq.Editor.getCurrentPage", vec![]).await?;
        Ok(serde_json::from_value(result)?)
    }

    pub async fn get_current_block(&self) -> Result<Block> {
        let result = self.call_api("logseq.Editor.getCurrentBlock", vec![]).await?;
        Ok(serde_json::from_value(result)?)
    }

    // Database methods
    pub async fn datascript_query(&self, query: &str) -> Result<Value> {
        let result = self.call_api("logseq.DB.datascriptQuery", vec![query.into()]).await?;
        Ok(result)
    }

    // App methods
    pub async fn get_current_graph(&self) -> Result<Value> {
        let result = self.call_api("logseq.App.getCurrentGraph", vec![]).await?;
        Ok(result)
    }

    pub async fn get_state_from_store(&self, key: &str) -> Result<Value> {
        let result = self.call_api("logseq.App.getStateFromStore", vec![key.into()]).await?;
        Ok(result)
    }

    pub async fn get_user_configs(&self) -> Result<Value> {
        let result = self.call_api("logseq.App.getUserConfigs", vec![]).await?;
        Ok(result)
    }
}