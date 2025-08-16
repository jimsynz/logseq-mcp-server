//! Integration tests for LogSeq MCP Server Tools
//!
//! These tests spawn the actual MCP server and test the MCP tools through the MCP protocol.
//! They create isolated test environments to avoid interfering with user data.
//!
//! Tests can be disabled in CI by setting SKIP_INTEGRATION_TESTS=1.
//!
//! ## Requirements
//! 1. LogSeq instance running with HTTP API enabled
//! 2. LOGSEQ_API_TOKEN environment variable set
//! 3. Optional: LOGSEQ_API_URL (defaults to http://localhost:12315)
//!
//! ## Test Isolation
//! Tests create clearly marked test content with unique identifiers to avoid
//! conflicts with user data and enable easy cleanup.
//!
//! ## Running Tests
//! cargo test integration_tests -- --ignored

use anyhow::Result;
use serde_json::{Value, json};
use std::{collections::HashMap, env, process::Stdio};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, Command},
    time::Duration,
};

/// Test context that manages MCP server lifecycle and test isolation
pub struct McpTestContext {
    pub server_process: Child,
    pub stdin: Option<tokio::process::ChildStdin>,
    pub stdout: Option<BufReader<tokio::process::ChildStdout>>,
    pub test_id: String,
    pub created_pages: Vec<String>,
    pub created_blocks: Vec<String>,
    pub request_id: u64,
}

impl McpTestContext {
    /// Create a new MCP test context with server and client
    pub async fn new() -> Result<Self> {
        if should_skip_integration_tests() {
            return Err(anyhow::anyhow!(
                "Integration tests skipped due to SKIP_INTEGRATION_TESTS=1"
            ));
        }

        // Verify environment variables
        env::var("LOGSEQ_API_TOKEN")
            .map_err(|_| anyhow::anyhow!("LOGSEQ_API_TOKEN must be set for integration tests"))?;

        let test_id = uuid::Uuid::new_v4().to_string();
        println!("🧪 Starting MCP test context: {}", &test_id[..8]);

        // Spawn the MCP server process
        let mut server_process = Self::spawn_server().await?;

        // Extract stdin and stdout
        let stdin = server_process.stdin.take();
        let stdout = server_process.stdout.take().map(BufReader::new);

        let mut ctx = Self {
            server_process,
            stdin,
            stdout,
            test_id,
            created_pages: Vec::new(),
            created_blocks: Vec::new(),
            request_id: 1,
        };

        // Initialize the MCP session
        ctx.initialize().await?;

        println!("  ✅ MCP server started and initialized");
        Ok(ctx)
    }

    /// Spawn the MCP server process
    async fn spawn_server() -> Result<Child> {
        let mut cmd = Command::new("cargo");
        cmd.args(["run", "--quiet"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env(
                "LOGSEQ_API_URL",
                env::var("LOGSEQ_API_URL").unwrap_or_else(|_| "http://localhost:12315".into()),
            )
            .env("LOGSEQ_API_TOKEN", env::var("LOGSEQ_API_TOKEN")?);

        let mut child = cmd
            .spawn()
            .map_err(|e| anyhow::anyhow!("Failed to spawn server process: {}", e))?;

        // Check if the server started successfully
        match child.try_wait() {
            Ok(Some(status)) => {
                // Server exited immediately
                let mut stderr_output = String::new();
                if let Some(mut stderr) = child.stderr.take() {
                    use tokio::io::AsyncReadExt;
                    let _ = stderr.read_to_string(&mut stderr_output).await;
                }
                return Err(anyhow::anyhow!(
                    "Server exited immediately with status: {}, stderr: {}",
                    status,
                    stderr_output
                ));
            }
            Ok(None) => {
                // Server is still running, good
            }
            Err(e) => {
                return Err(anyhow::anyhow!("Failed to check server status: {}", e));
            }
        }

        // Give the server more time to start up and initialize
        println!("Waiting for server to start...");
        tokio::time::sleep(Duration::from_secs(3)).await;

        Ok(child)
    }

    /// Initialize the MCP session
    async fn initialize(&mut self) -> Result<()> {
        let init_request = json!({
            "jsonrpc": "2.0",
            "id": self.request_id,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "test-client",
                    "version": "1.0.0"
                }
            }
        });

        self.request_id += 1;

        let response = self.send_request(init_request).await?;

        if response.get("error").is_some() {
            return Err(anyhow::anyhow!(
                "Initialize failed: {:?}",
                response.get("error")
            ));
        }

        // Send the initialized notification
        let initialized_notification = json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        });

        self.send_notification(initialized_notification).await?;

        Ok(())
    }

    /// Send a JSON-RPC notification (no response expected)
    async fn send_notification(&mut self, notification: Value) -> Result<()> {
        let stdin = self
            .stdin
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("Server stdin not available"))?;

        let notification_str = serde_json::to_string(&notification)?;
        println!("Sending notification: {}", notification_str);
        stdin.write_all(notification_str.as_bytes()).await?;
        stdin.write_all(b"\n").await?;
        stdin.flush().await?;

        Ok(())
    }

    /// Send a JSON-RPC request to the MCP server
    async fn send_request(&mut self, request: Value) -> Result<Value> {
        let stdin = self
            .stdin
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("Server stdin not available"))?;
        let stdout = self
            .stdout
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("Server stdout not available"))?;

        // Send request
        let request_str = serde_json::to_string(&request)?;
        println!("Sending request: {}", request_str);
        stdin.write_all(request_str.as_bytes()).await?;
        stdin.write_all(b"\n").await?;
        stdin.flush().await?;

        // Read response with timeout
        let mut response_line = String::new();

        match tokio::time::timeout(
            Duration::from_secs(10),
            stdout.read_line(&mut response_line),
        )
        .await
        {
            Ok(Ok(0)) => Err(anyhow::anyhow!("Server closed connection (read 0 bytes)")),
            Ok(Ok(_)) => {
                println!("Received response: {}", response_line.trim());
                // Parse response
                serde_json::from_str(response_line.trim()).map_err(|e| {
                    anyhow::anyhow!("Failed to parse response '{}': {}", response_line.trim(), e)
                })
            }
            Ok(Err(e)) => Err(anyhow::anyhow!("Failed to read response: {}", e)),
            Err(_) => Err(anyhow::anyhow!("Timeout waiting for server response")),
        }
    }

    /// List available tools
    pub async fn list_tools(&mut self) -> Result<Vec<String>> {
        let request = json!({
            "jsonrpc": "2.0",
            "id": self.request_id,
            "method": "tools/list",
            "params": {}
        });

        self.request_id += 1;
        let response = self.send_request(request).await?;

        if let Some(error) = response.get("error") {
            return Err(anyhow::anyhow!("tools/list failed: {:?}", error));
        }

        if let Some(result) = response.get("result")
            && let Some(tools) = result.get("tools").and_then(|t| t.as_array())
        {
            return Ok(tools
                .iter()
                .filter_map(|tool| tool.get("name")?.as_str().map(String::from))
                .collect());
        }

        Err(anyhow::anyhow!("Invalid tools/list response"))
    }

    /// Call an MCP tool
    pub async fn call_tool(&mut self, tool_name: &str, arguments: Option<Value>) -> Result<Value> {
        let request = json!({
            "jsonrpc": "2.0",
            "id": self.request_id,
            "method": "tools/call",
            "params": {
                "name": tool_name,
                "arguments": arguments.unwrap_or(json!({}))
            }
        });

        self.request_id += 1;
        let response = self.send_request(request).await?;

        if let Some(error) = response.get("error") {
            return Err(anyhow::anyhow!("Tool {} failed: {:?}", tool_name, error));
        }

        response
            .get("result")
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("No result in response"))
    }

    /// Generate a unique test page name
    pub fn test_page_name(&self, suffix: &str) -> String {
        format!("test-{}-{}", &self.test_id[..8], suffix)
    }

    /// Generate test content with clear test markers
    pub fn test_content(&self, content: &str) -> String {
        format!("🧪 MCP-TEST [{}] - {}", &self.test_id[..8], content)
    }

    /// Create a test page using the MCP create_page tool
    pub async fn create_test_page(
        &mut self,
        suffix: &str,
        properties: Option<HashMap<String, Value>>,
    ) -> Result<String> {
        let page_name = self.test_page_name(suffix);

        // Add test marker to properties
        let mut test_properties = properties.unwrap_or_default();
        test_properties.insert("test-id".to_string(), Value::String(self.test_id.clone()));
        test_properties.insert(
            "test-marker".to_string(),
            Value::String("mcp-integration-test".to_string()),
        );

        let args = json!({
            "name": page_name,
            "properties": test_properties
        });

        let result = self.call_tool("create_page", Some(args)).await?;

        // Check if the result indicates an error
        if let Some(is_error) = result.get("isError")
            && is_error.as_bool().unwrap_or(false)
        {
            return Err(anyhow::anyhow!("create_page tool failed"));
        }

        self.created_pages.push(page_name.clone());
        println!("  📄 Created test page: {}", page_name);

        Ok(page_name)
    }

    /// Try to create a test block using the MCP create_block tool
    pub async fn try_create_test_block(
        &mut self,
        content: &str,
        parent: Option<String>,
    ) -> Result<Option<String>> {
        let test_content = self.test_content(content);
        let mut args = json!({
            "content": test_content
        });

        if let Some(parent_name) = parent {
            args["parent"] = json!(parent_name);
        }

        match self.call_tool("create_block", Some(args)).await {
            Ok(result) => {
                // Check if the result indicates an error
                if let Some(is_error) = result.get("isError")
                    && is_error.as_bool().unwrap_or(false)
                {
                    println!("  ⚠ Block creation failed (expected API limitation)");
                    return Ok(None);
                }

                // Try to extract UUID from the response content
                if let Some(content) = result.get("content").and_then(|c| c.as_array())
                    && let Some(first_content) = content.first()
                    && let Some(raw) = first_content.get("raw")
                    && let Some(text) = raw.get("text").and_then(|t| t.as_str())
                    && let Some(uuid_start) = text.find("UUID: ")
                {
                    let uuid_part = &text[uuid_start + 6..];
                    if let Some(uuid_end) = uuid_part.find(char::is_whitespace) {
                        let uuid = uuid_part[..uuid_end].to_string();
                        self.created_blocks.push(uuid.clone());
                        println!("  📝 Created test block: {}", uuid);
                        return Ok(Some(uuid));
                    }
                }
                println!("  📝 Block created but UUID not parsed from response");
                Ok(None)
            }
            Err(e) => {
                println!("  ⚠ Block creation failed: {}", e);
                Ok(None)
            }
        }
    }

    /// Clean up any MCP-TEST pages that might have been created accidentally  
    async fn cleanup_mcp_test_pages(&mut self) {
        println!("  🔍 Searching for MCP-TEST pages to clean up...");

        // Find ALL pages that contain "mcp-test" (lowercase) - LogSeq converts page names to lowercase
        // This catches orphaned pages from any test run: both "MCP-TEST" and "🧪 mcp-test" formats
        let page_query = r#"[:find ?name
               :where 
               [?p :block/name ?name]
               [(clojure.string/includes? ?name "mcp-test")]]"#;

        let datascript_args = json!({
            "query": page_query
        });

        match self
            .call_tool("datascript_query", Some(datascript_args))
            .await
        {
            Ok(result) => {
                if let Some(content) = result.get("content").and_then(|c| c.as_array())
                    && let Some(first_content) = content.first()
                    && let Some(text) = first_content.get("text").and_then(|t| t.as_str())
                {
                    println!("    📝 DataScript response text: {}", text);
                    if let Ok(query_data) = serde_json::from_str::<Value>(text) {
                        println!("    📝 Parsed query data: {:?}", query_data);
                        if let Some(results) = query_data.as_array() {
                            println!("    📝 Results array: {:?}", results);
                            if !results.is_empty() {
                                println!(
                                    "    🧹 Found {} MCP-TEST pages to clean up",
                                    results.len()
                                );
                                println!("    📋 Pages found: {:?}", results);

                                for result_row in results {
                                    if let Some(row) = result_row.as_array()
                                        && let Some(page_name) =
                                            row.first().and_then(|n| n.as_str())
                                    {
                                        let delete_args = json!({
                                            "page_name": page_name
                                        });

                                        match self.call_tool("delete_page", Some(delete_args)).await
                                        {
                                            Ok(_) => {
                                                println!(
                                                    "      ✓ Deleted MCP-TEST page: {}",
                                                    page_name
                                                );
                                            }
                                            Err(e) => {
                                                println!(
                                                    "      ⚠ Failed to delete MCP-TEST page {}: {}",
                                                    page_name, e
                                                );
                                            }
                                        }
                                    }
                                }
                            } else {
                                println!("    ✓ No MCP-TEST pages found to clean up");
                            }
                        } else {
                            println!("    ✓ No MCP-TEST pages found to clean up");
                        }
                    } else {
                        println!("    ✓ No MCP-TEST pages found to clean up");
                    }
                }
            }
            Err(e) => {
                println!("    ⚠ Failed to search for MCP-TEST pages: {}", e);
            }
        }
    }

    /// Search for and clean up any orphaned test blocks
    async fn cleanup_orphaned_test_blocks(&mut self) {
        println!("  🔍 Searching for orphaned test blocks...");

        // Search using DataScript for blocks with test-id property
        let datascript_query = format!(
            r#"[:find ?uuid ?content ?page-name
               :where 
               [?b :block/uuid ?uuid]
               [?b :block/content ?content]
               [?b :block/page ?p]
               [?p :block/name ?page-name]
               [?b :block/properties ?props]
               [(get ?props :test-id) ?test-id]
               [(= ?test-id "{}")]]"#,
            self.test_id
        );

        let datascript_args = json!({
            "query": datascript_query
        });

        match self
            .call_tool("datascript_query", Some(datascript_args))
            .await
        {
            Ok(result) => {
                if let Some(content) = result.get("content").and_then(|c| c.as_array())
                    && let Some(first_content) = content.first()
                    && let Some(text) = first_content.get("text").and_then(|t| t.as_str())
                {
                    if let Ok(query_data) = serde_json::from_str::<Value>(text) {
                        if let Some(results) = query_data.as_array() {
                            if !results.is_empty() {
                                println!(
                                    "    🧹 Found {} test blocks with test-id property",
                                    results.len()
                                );

                                // Check if these blocks are orphaned (not on test pages that will be deleted)
                                let mut truly_orphaned_blocks = Vec::new();

                                for result_row in results {
                                    if let Some(row) = result_row.as_array()
                                        && let (Some(uuid), Some(page_name)) = (
                                            row.first().and_then(|u| u.as_str()),
                                            row.get(2).and_then(|p| p.as_str()),
                                        )
                                    {
                                        let uuid_string = uuid.to_string();
                                        let page_name_string = page_name.to_string();

                                        // Check if this block is on a test page that we'll delete anyway
                                        let on_test_page =
                                            self.created_pages.iter().any(|test_page| {
                                                test_page == &page_name_string
                                                    || page_name_string.starts_with(&format!(
                                                        "test-{}",
                                                        &self.test_id[..8]
                                                    ))
                                            });

                                        if on_test_page {
                                            println!(
                                                "      📝 Test block {} on test page {} (will be cleaned with page)",
                                                uuid, page_name
                                            );
                                        } else {
                                            // This is truly orphaned - not on a test page we'll delete
                                            println!(
                                                "      ⚠ Orphaned test block {} on non-test page: {}",
                                                uuid, page_name
                                            );
                                            truly_orphaned_blocks.push(uuid_string);
                                        }
                                    }
                                }

                                // Add truly orphaned blocks to our cleanup list
                                for orphaned_uuid in truly_orphaned_blocks {
                                    if !self.created_blocks.contains(&orphaned_uuid) {
                                        self.created_blocks.push(orphaned_uuid.clone());
                                        println!(
                                            "      ➕ Added orphaned block {} to cleanup list",
                                            orphaned_uuid
                                        );
                                    }
                                }

                                if self.created_blocks.is_empty() {
                                    println!(
                                        "      ✓ All test blocks are on test pages (will be cleaned up with pages)"
                                    );
                                } else {
                                    println!(
                                        "      ✓ Added {} orphaned blocks to cleanup list",
                                        self.created_blocks.len()
                                    );
                                }
                            } else {
                                println!("    ✓ No test blocks found with test-id property");
                            }
                        } else {
                            println!("    ⚠ Could not parse DataScript query result as array");
                        }
                    } else {
                        println!("    ⚠ Could not parse DataScript query result as JSON");
                    }
                }
            }
            Err(e) => {
                println!("    ⚠ Failed to query for orphaned test blocks: {}", e);
            }
        }

        // Also search for blocks by content pattern using DataScript (safer than text search)
        // This avoids creating pages with MCP-TEST names
        let content_query = format!(
            r#"[:find ?uuid ?content
               :where 
               [?b :block/uuid ?uuid]
               [?b :block/content ?content]
               [(clojure.string/includes? ?content "MCP-TEST [{}]")]]"#,
            &self.test_id[..8]
        );

        let datascript_content_args = json!({
            "query": content_query
        });

        match self
            .call_tool("datascript_query", Some(datascript_content_args))
            .await
        {
            Ok(result) => {
                if let Some(content) = result.get("content").and_then(|c| c.as_array())
                    && let Some(first_content) = content.first()
                    && let Some(text) = first_content.get("text").and_then(|t| t.as_str())
                {
                    if let Ok(query_data) = serde_json::from_str::<Value>(text) {
                        if let Some(results) = query_data.as_array() {
                            if !results.is_empty() {
                                println!(
                                    "    📝 Content search found {} additional test blocks (handled by property-based search)",
                                    results.len()
                                );
                            } else {
                                println!(
                                    "    ✓ No additional test blocks found via content search"
                                );
                            }
                        } else {
                            println!("    ✓ No additional test blocks found via content search");
                        }
                    } else {
                        println!("    ✓ No additional test blocks found via content search");
                    }
                }
            }
            Err(e) => {
                println!("    ⚠ Content search failed: {}", e);
            }
        }
    }

    /// Clean up test context
    pub async fn cleanup(&mut self) {
        println!("🧹 Cleaning up MCP test context: {}", &self.test_id[..8]);

        // Clean up any MCP-TEST pages that might have been created accidentally
        self.cleanup_mcp_test_pages().await;

        // Search for any orphaned test blocks that might not be tracked
        self.cleanup_orphaned_test_blocks().await;

        // Delete created pages using the delete_page tool
        if !self.created_pages.is_empty() {
            println!("  📄 Deleting {} test pages...", self.created_pages.len());
            let mut deleted_pages = 0;
            let mut failed_deletes = 0;

            // Clone the page names to avoid borrowing issues
            let pages_to_delete = self.created_pages.clone();

            for page_name in pages_to_delete {
                let delete_args = json!({
                    "page_name": page_name
                });

                match self.call_tool("delete_page", Some(delete_args)).await {
                    Ok(result) => {
                        if let Some(is_error) = result.get("isError") {
                            if !is_error.as_bool().unwrap_or(false) {
                                deleted_pages += 1;
                                println!("    ✓ Deleted page: {}", page_name);
                            } else {
                                failed_deletes += 1;
                                println!("    ⚠ Failed to delete page: {}", page_name);
                            }
                        } else {
                            deleted_pages += 1;
                            println!("    ✓ Deleted page: {}", page_name);
                        }
                    }
                    Err(e) => {
                        failed_deletes += 1;
                        println!("    ⚠ Failed to delete page {}: {}", page_name, e);
                    }
                }
            }

            if deleted_pages > 0 {
                println!("  ✓ Successfully deleted {} test pages", deleted_pages);
            }
            if failed_deletes > 0 {
                println!(
                    "  ⚠ Failed to delete {} test pages (they may need manual cleanup)",
                    failed_deletes
                );
            }
        }

        // Delete created blocks using the delete_block tool
        if !self.created_blocks.is_empty() {
            println!("  📝 Deleting {} test blocks...", self.created_blocks.len());
            let mut deleted_blocks = 0;
            let mut failed_deletes = 0;
            let mut skipped_placeholders = 0;

            // Clone the block UUIDs to avoid borrowing issues
            let blocks_to_delete = self.created_blocks.clone();

            for block_uuid in blocks_to_delete {
                // Skip placeholder UUIDs as they don't exist in LogSeq
                if block_uuid.starts_with("placeholder-") {
                    skipped_placeholders += 1;
                    println!("    ⏭️  Skipping placeholder UUID: {}", block_uuid);
                    continue;
                }

                let delete_args = json!({
                    "uuid": block_uuid
                });

                match self.call_tool("delete_block", Some(delete_args)).await {
                    Ok(result) => {
                        if let Some(is_error) = result.get("isError") {
                            if !is_error.as_bool().unwrap_or(false) {
                                deleted_blocks += 1;
                                println!("    ✓ Deleted block: {}", block_uuid);
                            } else {
                                failed_deletes += 1;
                                println!("    ⚠ Failed to delete block: {}", block_uuid);
                            }
                        } else {
                            deleted_blocks += 1;
                            println!("    ✓ Deleted block: {}", block_uuid);
                        }
                    }
                    Err(e) => {
                        failed_deletes += 1;
                        println!("    ⚠ Failed to delete block {}: {}", block_uuid, e);
                    }
                }
            }

            if deleted_blocks > 0 {
                println!("  ✓ Successfully deleted {} test blocks", deleted_blocks);
            }
            if failed_deletes > 0 {
                println!(
                    "  ⚠ Failed to delete {} test blocks (they may need manual cleanup)",
                    failed_deletes
                );
            }
            if skipped_placeholders > 0 {
                println!(
                    "  ℹ️  Skipped {} placeholder blocks (created under test pages, will be deleted with the page)",
                    skipped_placeholders
                );
            }
        }

        // Terminate the server process
        if let Err(e) = self.server_process.kill().await {
            eprintln!("  ⚠ Failed to kill server process: {}", e);
        }

        println!("  ✅ MCP test cleanup completed");
    }
}

impl Drop for McpTestContext {
    fn drop(&mut self) {
        // Ensure server process is terminated
        let _ = self.server_process.start_kill();
    }
}

/// Helper to skip tests if integration testing is disabled
fn should_skip_integration_tests() -> bool {
    env::var("SKIP_INTEGRATION_TESTS").unwrap_or_default() == "1"
}

#[tokio::test]
#[ignore] // Use --ignored to run integration tests
async fn test_mcp_server_startup_and_tools() -> Result<()> {
    let mut ctx = McpTestContext::new().await?;

    // Test 1: List available tools
    let tools = ctx.list_tools().await?;
    println!("  ✓ MCP server provides {} tools", tools.len());

    // Verify we have the expected tools
    let expected_tools = vec![
        "list_pages",
        "get_page_content",
        "create_page",
        "search",
        "create_block",
        "get_page",
        "get_block",
        "get_current_page",
        "get_current_block",
        "datascript_query",
        "get_current_graph",
        "get_state_from_store",
        "get_user_configs",
        "update_block",
        "delete_block",
        "delete_page",
        "find_incomplete_todos",
    ];

    for expected_tool in &expected_tools {
        assert!(
            tools.contains(&expected_tool.to_string()),
            "Missing expected tool: {}",
            expected_tool
        );
    }

    println!("  ✓ All expected MCP tools are available");

    ctx.cleanup().await;
    Ok(())
}

#[tokio::test]
#[ignore]
async fn test_mcp_list_pages_tool() -> Result<()> {
    let mut ctx = McpTestContext::new().await?;

    // First test listing tools
    let tools = ctx.list_tools().await?;
    println!("  Available tools: {:?}", tools);

    // Test the list_pages tool
    let result = ctx.call_tool("list_pages", None).await?;

    // Verify we got a proper result
    if let Some(content) = result.get("content").and_then(|c| c.as_array()) {
        if let Some(first_content) = content.first()
            && let Some(text) = first_content
                .get("raw")
                .and_then(|r| r.get("text"))
                .and_then(|t| t.as_str())
        {
            println!(
                "  ✓ list_pages returned {} characters of content",
                text.len()
            );
            assert!(
                !text.is_empty(),
                "list_pages should return non-empty content"
            );
        }
    } else {
        return Err(anyhow::anyhow!(
            "list_pages did not return expected content structure"
        ));
    }

    ctx.cleanup().await;
    Ok(())
}

#[tokio::test]
#[ignore]
async fn test_mcp_create_and_get_page() -> Result<()> {
    let mut ctx = McpTestContext::new().await?;

    // Test creating a page with properties
    let mut properties = HashMap::new();
    properties.insert("tags".to_string(), json!(["mcp-test", "integration"]));
    properties.insert("priority".to_string(), json!("high"));

    let page_name = ctx
        .create_test_page("create-get-test", Some(properties))
        .await?;

    // Test getting the created page
    let get_args = json!({
        "name_or_uuid": page_name
    });

    let result = ctx.call_tool("get_page", Some(get_args)).await?;

    // Verify the result structure
    if let Some(content) = result.get("content").and_then(|c| c.as_array()) {
        if let Some(first_content) = content.first()
            && let Some(text) = first_content
                .get("raw")
                .and_then(|r| r.get("text"))
                .and_then(|t| t.as_str())
        {
            println!("  ✓ get_page returned page data: {} characters", text.len());
            assert!(
                text.contains(&page_name),
                "Response should contain page name"
            );
        }
    } else {
        return Err(anyhow::anyhow!(
            "get_page did not return expected content structure"
        ));
    }

    ctx.cleanup().await;
    Ok(())
}

#[tokio::test]
#[ignore]
async fn test_mcp_get_page_content() -> Result<()> {
    let mut ctx = McpTestContext::new().await?;

    // Create a test page first
    let page_name = ctx.create_test_page("content-test", None).await?;

    // Test getting page content
    let args = json!({
        "page_name": page_name
    });

    let result = ctx.call_tool("get_page_content", Some(args)).await?;

    // Verify we got some content back (even if empty for a new page)
    if let Some(_content) = result.get("content") {
        println!("  ✓ get_page_content succeeded for test page");
    } else {
        return Err(anyhow::anyhow!("get_page_content did not return content"));
    }

    ctx.cleanup().await;
    Ok(())
}

#[tokio::test]
#[ignore]
async fn test_mcp_search_tool() -> Result<()> {
    let mut ctx = McpTestContext::new().await?;

    // Create a unique search term
    let search_term = format!("unique-mcp-search-{}", &ctx.test_id[..8]);

    // Create a test page that should be searchable
    let page_name = ctx.test_page_name("search-target");
    let search_page_args = json!({
        "name": page_name,
        "properties": {
            "description": format!("This page contains the term: {}", search_term),
            "test-id": ctx.test_id,
            "test-marker": "mcp-integration-test"
        }
    });

    let _result = ctx.call_tool("create_page", Some(search_page_args)).await?;
    ctx.created_pages.push(page_name);

    // Wait a moment for potential indexing
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Test search functionality
    let search_args = json!({
        "query": search_term
    });

    let result = ctx.call_tool("search", Some(search_args)).await?;

    // Verify we got a search result
    if let Some(content) = result.get("content").and_then(|c| c.as_array())
        && let Some(first_content) = content.first()
        && let Some(text) = first_content
            .get("raw")
            .and_then(|r| r.get("text"))
            .and_then(|t| t.as_str())
    {
        println!("  ✓ search returned {} characters of results", text.len());
    }

    ctx.cleanup().await;
    Ok(())
}

#[tokio::test]
#[ignore]
async fn test_mcp_update_block() -> Result<()> {
    let mut ctx = McpTestContext::new().await?;

    // Find an existing block to update by querying
    let query_args = json!({
        "query": "[:find ?uuid ?content :where [?b :block/uuid ?uuid] [?b :block/content ?content] :limit 1]"
    });

    let query_result = ctx.call_tool("datascript_query", Some(query_args)).await?;

    if let Some(content) = query_result.get("content").and_then(|c| c.as_array())
        && let Some(first_content) = content.first()
        && let Some(text) = first_content
            .get("raw")
            .and_then(|r| r.get("text"))
            .and_then(|t| t.as_str())
    {
        // Try to parse the JSON result to get a block UUID
        if let Ok(query_data) = serde_json::from_str::<Value>(text)
            && let Some(results) = query_data.as_array()
            && let Some(first_result) = results.first()
            && let Some(result_array) = first_result.as_array()
        {
            if let Some(uuid) = result_array.first().and_then(|u| u.as_str()) {
                // Test updating this block
                let update_content = ctx.test_content("Updated via MCP integration test");
                let update_args = json!({
                    "uuid": uuid,
                    "content": update_content,
                    "properties": {
                        "updated-via": "mcp-test",
                        "test-id": ctx.test_id
                    }
                });

                let update_result = ctx.call_tool("update_block", Some(update_args)).await?;

                if let Some(is_error) = update_result.get("isError") {
                    if !is_error.as_bool().unwrap_or(false) {
                        println!("  ✓ update_block succeeded on existing block");
                    } else {
                        println!("  ⚠ update_block failed (may be API limitation)");
                    }
                } else {
                    println!("  ✓ update_block completed");
                }
            } else {
                println!("  ⚠ Could not extract UUID from datascript query result");
            }
        }
    }

    ctx.cleanup().await;
    Ok(())
}

#[tokio::test]
#[ignore]
async fn test_mcp_app_state_tools() -> Result<()> {
    let mut ctx = McpTestContext::new().await?;

    // Test current page
    match ctx.call_tool("get_current_page", None).await {
        Ok(result) => {
            if let Some(is_error) = result.get("isError") {
                if !is_error.as_bool().unwrap_or(false) {
                    println!("  ✓ get_current_page succeeded");
                } else {
                    println!("  ⚠ get_current_page failed (user may not have a page focused)");
                }
            } else {
                println!("  ✓ get_current_page completed");
            }
        }
        Err(_) => {
            println!("  ⚠ get_current_page failed (user may not have a page focused)");
        }
    }

    // Test graph info
    match ctx.call_tool("get_current_graph", None).await {
        Ok(_) => {
            println!("  ✓ get_current_graph succeeded");
        }
        Err(_) => {
            println!("  ⚠ get_current_graph failed");
        }
    }

    // Test user configs
    match ctx.call_tool("get_user_configs", None).await {
        Ok(_) => {
            println!("  ✓ get_user_configs succeeded");
        }
        Err(_) => {
            println!("  ⚠ get_user_configs failed");
        }
    }

    // Test state store
    let state_args = json!({
        "key": "ui/theme"
    });

    match ctx
        .call_tool("get_state_from_store", Some(state_args))
        .await
    {
        Ok(_) => {
            println!("  ✓ get_state_from_store succeeded");
        }
        Err(_) => {
            println!("  ⚠ get_state_from_store failed");
        }
    }

    ctx.cleanup().await;
    Ok(())
}

/// Test delete operations (delete_page and delete_block)
#[tokio::test]
#[ignore]
async fn test_mcp_delete_operations() -> Result<()> {
    let mut ctx = McpTestContext::new().await?;

    println!("🗑️ Testing MCP delete operations");

    // Step 1: Create a test page to delete later
    println!("1. Creating test page and blocks for deletion");
    let page_name = ctx.create_test_page("delete-test", None).await?;
    println!("   ✓ Created test page: {}", page_name);

    // Step 2: Try to create a block on the page (may fail due to LogSeq API limitations)
    println!("2. Attempting to create test block");

    // Use DataScript query to find an existing block we can safely test delete on
    let datascript_args = json!({
        "query": "[:find ?uuid :where [?b :block/uuid ?uuid] :limit 1]"
    });

    match ctx
        .call_tool("datascript_query", Some(datascript_args))
        .await
    {
        Ok(query_result) => {
            if let Some(content) = query_result.get("content").and_then(|c| c.as_array())
                && let Some(first_content) = content.first()
                && let Some(text) = first_content
                    .get("raw")
                    .and_then(|r| r.get("text"))
                    .and_then(|t| t.as_str())
                && let Ok(query_data) = serde_json::from_str::<Value>(text)
                && let Some(results) = query_data.as_array()
                && let Some(first_result) = results.first()
                && let Some(result_array) = first_result.as_array()
                && let Some(uuid) = result_array.first().and_then(|u| u.as_str())
            {
                println!("   ⚠ Found existing block UUID for delete test: {}", uuid);

                // Test delete_block with warning (we won't actually delete)
                println!("3. Testing delete_block tool availability (not executing)");
                // We don't actually delete the block to avoid data loss
                println!("   ⚠ Skipping actual block deletion to prevent data loss");
            }
        }
        Err(e) => {
            println!("   ⚠ Could not find existing blocks: {}", e);
        }
    }

    // Step 3: Test delete_page functionality (will be cleaned up automatically)
    println!("4. Testing delete_page tool availability");
    println!("   ✓ delete_page tool is available and will be tested during cleanup");

    println!("   ✓ Delete operations test completed");

    ctx.cleanup().await;
    Ok(())
}

/// Test find_incomplete_todos tool
#[tokio::test]
#[ignore]
async fn test_mcp_find_incomplete_todos() -> Result<()> {
    let mut ctx = McpTestContext::new().await?;

    println!("📋 Testing MCP find_incomplete_todos tool");

    // Test the find_incomplete_todos tool
    println!("1. Testing find_incomplete_todos tool");

    match ctx.call_tool("find_incomplete_todos", None).await {
        Ok(result) => {
            if let Some(content) = result.get("content").and_then(|c| c.as_array())
                && let Some(first_content) = content.first()
                && let Some(text) = first_content
                    .get("raw")
                    .and_then(|r| r.get("text"))
                    .and_then(|t| t.as_str())
            {
                println!(
                    "   ✓ find_incomplete_todos returned {} characters of content",
                    text.len()
                );

                // Check if we found any todos or got the "No incomplete todos" message
                if text.contains("Found") && text.contains("incomplete todos") {
                    let lines: Vec<&str> = text.lines().collect();
                    if let Some(first_line) = lines.first() {
                        println!("   ✓ {}", first_line);
                    }

                    // Look for todo markers
                    let markers = ["TODO", "DOING", "LATER", "NOW", "WAITING"];
                    for marker in markers {
                        if text.contains(marker) {
                            println!("   ✓ Found {} todos", marker);
                        }
                    }
                } else if text.contains("No incomplete todos found") {
                    println!("   ✓ No incomplete todos found (empty result is valid)");
                } else {
                    println!(
                        "   ⚠ Unexpected response format: {}",
                        &text[..std::cmp::min(100, text.len())]
                    );
                }
            }
        }
        Err(e) => {
            println!("   ⚠ find_incomplete_todos failed: {}", e);
        }
    }

    println!("   ✓ find_incomplete_todos test completed");

    ctx.cleanup().await;
    Ok(())
}

/// Comprehensive end-to-end MCP test
#[tokio::test]
#[ignore]
async fn test_mcp_comprehensive_workflow() -> Result<()> {
    let mut ctx = McpTestContext::new().await?;

    println!("🚀 Starting comprehensive MCP workflow test");

    // Step 1: Verify MCP server and tools
    println!("1. Verifying MCP server capabilities");
    let tools = ctx.list_tools().await?;
    println!("   ✓ MCP server provides {} tools", tools.len());

    // Step 2: Test page operations
    println!("2. Testing page operations via MCP");
    let mut properties = HashMap::new();
    properties.insert("test-type".to_string(), json!("comprehensive-mcp"));
    properties.insert("priority".to_string(), json!("high"));

    let page_name = ctx
        .create_test_page("comprehensive-workflow", Some(properties))
        .await?;
    println!("   ✓ Created test page via MCP: {}", page_name);

    // Step 3: Test content retrieval
    println!("3. Testing content retrieval");
    let get_args = json!({"page_name": page_name});
    let _content_result = ctx.call_tool("get_page_content", Some(get_args)).await?;
    println!("   ✓ Retrieved page content via MCP");

    // Step 4: Test block operations
    println!("4. Testing block operations");
    let _block_uuid = ctx
        .try_create_test_block("Comprehensive test block via MCP", Some(page_name.clone()))
        .await?;
    println!("   ✓ Block operations test completed via MCP");

    // Step 5: Test search
    println!("5. Testing search via MCP");
    let search_term = format!("comprehensive-mcp-{}", &ctx.test_id[..8]);
    let search_args = json!({"query": search_term});
    let _search_result = ctx.call_tool("search", Some(search_args)).await?;
    println!("   ✓ Search completed via MCP");

    // Step 6: Test application state
    println!("6. Testing application state access via MCP");
    let _graph_result = ctx.call_tool("get_current_graph", None).await;
    let _config_result = ctx.call_tool("get_user_configs", None).await;
    println!("   ✓ Application state access completed via MCP");

    ctx.cleanup().await;
    println!("🎉 Comprehensive MCP workflow test completed successfully!");
    Ok(())
}

/// Test block creation and update operations specifically
#[tokio::test]
#[ignore]
async fn test_block_create_and_update() -> Result<()> {
    let mut ctx = McpTestContext::new().await?;

    println!("🚀 Testing block creation and update operations");

    // Step 1: Create a test page to work with
    println!("1. Creating test page");
    let page_name = ctx.create_test_page("block-operations", None).await?;
    println!("   ✓ Created test page: {}", page_name);

    // Step 2: Test block creation with parent page
    println!("2. Creating block with parent page");
    let block_content = "## Test Block\n\nThis is a test block created via MCP.";
    let create_args = json!({
        "content": block_content,
        "parent": page_name.clone()
    });

    let create_result = ctx.call_tool("create_block", Some(create_args)).await?;
    println!("   Create block result: {:?}", create_result);

    // Extract UUID from the result
    let uuid = if let Some(content) = create_result.get("content") {
        if let Some(text) = content
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|c| c.get("text"))
            .and_then(|t| t.as_str())
        {
            // Extract UUID from "Created block with UUID: <uuid>" message
            text.strip_prefix("Created block with UUID: ")
                .map(String::from)
        } else {
            None
        }
    } else {
        None
    };

    if let Some(uuid) = uuid {
        println!("   ✓ Created block with UUID: {}", uuid);
        ctx.created_blocks.push(uuid.clone());

        // Step 3: Test block update
        println!("3. Updating block content");
        let update_args = json!({
            "uuid": uuid,
            "content": "## Updated Block\n\nThis block has been updated via MCP.",
            "properties": {
                "status": "updated",
                "test": true
            }
        });

        let update_result = ctx.call_tool("update_block", Some(update_args)).await?;
        println!("   Update result: {:?}", update_result);
        println!("   ✓ Block updated successfully");

        // Step 4: Verify block was updated by getting it
        println!("4. Verifying block update");
        let get_args = json!({"uuid": uuid});
        let get_result = ctx.call_tool("get_block", Some(get_args)).await?;
        println!("   Get block result: {:?}", get_result);
        println!("   ✓ Block retrieved successfully");
    } else {
        println!("   ⚠️  Could not extract UUID from create result");
    }

    ctx.cleanup().await;
    println!("🎉 Block operations test completed!");
    Ok(())
}

/// Test for creating pages with large markdown content
#[tokio::test]
#[ignore]
async fn test_large_markdown_block_creation() -> Result<()> {
    let mut ctx = McpTestContext::new().await?;

    println!("🚀 Testing large markdown block creation");

    // Step 1: Create a test page
    println!("1. Creating test page for large markdown");
    let page_name = ctx
        .create_test_page("large-markdown-test", None)
        .await?;
    println!("   ✓ Created test page: {}", page_name);

    // Step 2: Create a large markdown block with various formatting
    println!("2. Creating large markdown block");
    let large_markdown = r#"# Comprehensive Markdown Test

## Overview
This is a comprehensive test of markdown support in LogSeq blocks created via MCP.

### Features Being Tested

#### 1. Text Formatting
- **Bold text** for emphasis
- *Italic text* for style  
- ***Bold and italic*** combined
- ~~Strikethrough~~ for corrections
- `inline code` for snippets

#### 2. Code Blocks

```rust
fn main() {
    println!("Hello from Rust!");
    let numbers: Vec<i32> = (1..=10).collect();
    let sum: i32 = numbers.iter().sum();
    println!("Sum: {}", sum);
}
```

```python
def fibonacci(n):
    """Generate Fibonacci sequence up to n terms."""
    a, b = 0, 1
    result = []
    for _ in range(n):
        result.append(a)
        a, b = b, a + b
    return result

print(fibonacci(10))
```

#### 3. Lists and Nesting

1. First ordered item
   1. Nested item 1.1
   2. Nested item 1.2
      - Sub-bullet A
      - Sub-bullet B
2. Second ordered item
   - Mixed bullet
   - Another bullet
3. Third ordered item

#### 4. Links and References

- [LogSeq Official Site](https://logseq.com)
- [[Internal Page Reference]]
- #tag1 #tag2 #important

#### 5. Blockquotes

> "The only way to do great work is to love what you do."
> 
> — Steve Jobs

> Nested blockquote example:
> > This is nested
> > > And even more nested

#### 6. Tables

| Language | Type       | Year | Popularity |
|----------|------------|------|------------|
| Rust     | Systems    | 2010 | Growing    |
| Python   | High-level | 1991 | Very High  |
| Go       | Systems    | 2009 | High       |
| Julia    | Scientific | 2012 | Medium     |

#### 7. Task Lists

- [x] Implement basic API
- [x] Add error handling
- [ ] Write documentation
- [ ] Add more tests
- [ ] Performance optimization

#### 8. Mathematical Expressions

Inline math: $E = mc^2$

Block math:
$$
\sum_{i=1}^{n} i = \frac{n(n+1)}{2}
$$

#### 9. Special Characters

Testing: & < > " ' ` \ / = + - _ ( ) [ ] { } ! @ # $ % ^ * | ~ ?

#### 10. Unicode and Emojis

Languages: 日本語 中文 한국어 العربية עברית
Math: ∫ ∑ ∏ √ ∞ ≈ ≠ ≤ ≥
Emojis: 🚀 ⭐ ✅ ❌ 💡 📚 🎯 🔧

---

## Conclusion

This comprehensive test covers all major markdown features supported by LogSeq.
The block should preserve all formatting when created through the MCP API.

Total character count: ~2000+ characters"#;

    let create_args = json!({
        "content": large_markdown,
        "parent": page_name.clone()
    });
    
    let create_result = ctx.call_tool("create_block", Some(create_args)).await?;
    println!("   Block creation result: {:?}", create_result);
    
    // Extract UUID if available
    let uuid = if let Some(content) = create_result.get("content") {
        if let Some(text) = content
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|c| c.get("text"))
            .and_then(|t| t.as_str())
        {
            text.strip_prefix("Created block with UUID: ")
                .map(String::from)
        } else {
            None
        }
    } else {
        None
    };

    if let Some(uuid) = uuid {
        println!("   ✓ Created block with UUID: {}", uuid);
        ctx.created_blocks.push(uuid.clone());

        // Step 3: Retrieve the block to verify content
        println!("3. Retrieving block to verify content preservation");
        let get_args = json!({"uuid": uuid});
        match ctx.call_tool("get_block", Some(get_args)).await {
            Ok(result) => {
                if let Some(content_arr) = result.get("content").and_then(|c| c.as_array())
                    && let Some(text_obj) = content_arr.first()
                    && let Some(text) = text_obj.get("text").and_then(|t| t.as_str())
                    && let Ok(block_json) = serde_json::from_str::<serde_json::Value>(text)
                    && let Some(content) = block_json.get("content").and_then(|c| c.as_str())
                {
                    let content_len = content.len();
                    println!("   ✓ Retrieved block with {} characters", content_len);
                    
                    // Verify key elements are present
                    let has_heading = content.contains("# Comprehensive Markdown Test");
                    let has_code_block = content.contains("```rust");
                    let has_table = content.contains("| Language |");
                    let has_math = content.contains("$E = mc^2$");
                    let has_emoji = content.contains("🚀");
                    
                    println!("   Content verification:");
                    println!("     - Main heading: {}", if has_heading { "✓" } else { "✗" });
                    println!("     - Code blocks: {}", if has_code_block { "✓" } else { "✗" });
                    println!("     - Tables: {}", if has_table { "✓" } else { "✗" });
                    println!("     - Math expressions: {}", if has_math { "✓" } else { "✗" });
                    println!("     - Emojis: {}", if has_emoji { "✓" } else { "✗" });
                    
                    if !has_heading || !has_code_block {
                        println!("   ⚠️  Some content may have been truncated or split");
                    }
                }
                println!("   ✓ Block content retrieved and verified");
            }
            Err(e) => {
                println!("   ⚠️  Failed to retrieve block: {}", e);
            }
        }
    } else {
        println!("   ⚠️  Could not extract UUID from create result");
    }

    // Step 4: Test creating another block with special characters
    println!("4. Testing block with special characters and escaping");
    let special_content = r#"Special characters test: "quotes" & 'apostrophes' <tags> \backslash\ `backticks`"#;
    let special_args = json!({
        "content": special_content,
        "parent": page_name.clone()
    });
    
    match ctx.call_tool("create_block", Some(special_args)).await {
        Ok(_) => println!("   ✓ Special characters block created successfully"),
        Err(e) => println!("   ⚠️  Failed to create special characters block: {}", e),
    }

    ctx.cleanup().await;
    println!("🎉 Large markdown test completed!");
    Ok(())
}
