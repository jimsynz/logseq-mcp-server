mod logseq;
mod tools;

use anyhow::Result;
use logseq::api::{InsertBlockOptions, LogSeqClient};
use rmcp::{
    ErrorData as McpError,
    handler::server::ServerHandler,
    model::{
        CallToolRequestParam, CallToolResult, Implementation, InitializeResult, ListToolsResult,
        PaginatedRequestParam, ProtocolVersion, RawContent, RawTextContent, ServerCapabilities,
        ServerInfo, Tool,
    },
    service::{RequestContext, RoleServer, ServiceExt},
    transport::io::stdio,
};
use std::env;
use std::sync::Arc;
use tools::{format_blocks_as_markdown, format_search_results, format_todos};

#[derive(Clone, Default)]
pub struct LogSeqMcpServer {
    logseq_client: Option<Arc<LogSeqClient>>,
}

impl LogSeqMcpServer {
    fn new(logseq_client: LogSeqClient) -> Self {
        Self {
            logseq_client: Some(Arc::new(logseq_client)),
        }
    }

    fn get_client(&self) -> Result<Arc<LogSeqClient>, McpError> {
        self.logseq_client
            .clone()
            .ok_or_else(|| McpError::internal_error("LogSeq client not initialized", None))
    }
}

impl ServerHandler for LogSeqMcpServer {
    fn get_info(&self) -> ServerInfo {
        InitializeResult {
            protocol_version: ProtocolVersion::LATEST,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: "logseq-mcp-server".into(),
                version: env!("CARGO_PKG_VERSION").into(),
            },
            instructions: Some("A LogSeq MCP server for managing your knowledge graph".into()),
        }
    }

    async fn list_tools(
        &self,
        _params: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        Ok(ListToolsResult {
            tools: vec![
                Tool {
                    name: "list_pages".into(),
                    description: Some("List all pages in the current LogSeq graph. Returns a list of page names that can be used with other page-related tools.".into()),
                    input_schema: Arc::new(
                        serde_json::json!({
                            "type": "object",
                            "properties": {},
                            "additionalProperties": false
                        })
                        .as_object()
                        .unwrap()
                        .clone(),
                    ),
                    annotations: None,
                    output_schema: None,
                },
                Tool {
                    name: "get_page_content".into(),
                    description: Some("Get the content of a specific page formatted as markdown. Use this to read and understand the structure of a page's blocks and content.".into()),
                    input_schema: Arc::new(
                        serde_json::json!({
                            "type": "object",
                            "properties": {
                                "page_name": {
                                    "type": "string",
                                    "description": "The name or UUID of the page. Page names are case-sensitive and should match exactly as they appear in LogSeq."
                                }
                            },
                            "required": ["page_name"],
                            "additionalProperties": false
                        })
                        .as_object()
                        .unwrap()
                        .clone(),
                    ),
                    annotations: None,
                    output_schema: None,
                },
                Tool {
                    name: "create_page".into(),
                    description: Some("Create a new page in LogSeq. You can optionally specify page properties like tags, template, aliases, and custom properties.".into()),
                    input_schema: Arc::new(
                        serde_json::json!({
                            "type": "object",
                            "properties": {
                                "name": {
                                    "type": "string",
                                    "description": "The name of the new page"
                                },
                "properties": {
                                    "type": "object",
                                    "description": "Optional page properties. Common properties include: 'tags' (array of strings), 'template' (string), 'alias' (array of strings), 'public' (boolean), 'filters' (object), and any custom properties you want to associate with the page.",
                                    "properties": {
                                        "tags": {
                                            "type": "array",
                                            "items": {"type": "string"},
                                            "description": "Tags to apply to the page"
                                        },
                                        "template": {
                                            "type": "string",
                                            "description": "Template to use for the page"
                                        },
                                        "alias": {
                                            "type": "array",
                                            "items": {"type": "string"},
                                            "description": "Alternative names for the page"
                                        },
                                        "public": {
                                            "type": "boolean",
                                            "description": "Whether the page should be public"
                                        },
                                        "filters": {
                                            "type": "object",
                                            "description": "Filters to apply to the page view"
                                        }
                                    },
                                    "additionalProperties": true
                                }
                            },
                            "required": ["name"],
                            "additionalProperties": false
                        })
                        .as_object()
                        .unwrap()
                        .clone(),
                    ),
                    annotations: None,
                    output_schema: None,
                },
                Tool {
                    name: "search".into(),
                    description: Some("Search for content across all pages and blocks in the LogSeq graph. Returns matching blocks with their content and context.".into()),
                    input_schema: Arc::new(
                        serde_json::json!({
                            "type": "object",
                            "properties": {
                                "query": {
                                    "type": "string",
                                    "description": "Search query string. Supports text search across block content. Use keywords or phrases to find relevant blocks."
                                }
                            },
                            "required": ["query"],
                            "additionalProperties": false
                        })
                        .as_object()
                        .unwrap()
                        .clone(),
                    ),
                    annotations: None,
                    output_schema: None,
                },
                Tool {
                    name: "create_block".into(),
                    description: Some("Insert a new block into LogSeq. You can specify a parent page/block or insert relative to a sibling block. Returns the created block's UUID.".into()),
                    input_schema: Arc::new(
                        serde_json::json!({
                            "type": "object",
                            "properties": {
                                "content": {
                                    "type": "string",
                                    "description": "Block content in markdown format. Can include text, links, formatting, and LogSeq-specific syntax."
                                },
                                "parent": {
                                    "type": "string",
                                    "description": "Parent page name or block UUID where this block should be created. If not specified, block will be created on the current page."
                                },
                                "sibling": {
                                    "type": "string",
                                    "description": "Block UUID of an existing block. The new block will be inserted as a sibling at the same level."
                                }
                            },
                            "required": ["content"],
                            "additionalProperties": false
                        })
                        .as_object()
                        .unwrap()
                        .clone(),
                    ),
                    annotations: None,
                    output_schema: None,
                },
                Tool {
                    name: "get_page".into(),
                    description: Some("Get detailed information about a specific page by name or UUID. Returns page metadata including properties, UUID, and structure.".into()),
                    input_schema: Arc::new(
                        serde_json::json!({
                            "type": "object",
                            "properties": {
                                "name_or_uuid": {
                                    "type": "string",
                                    "description": "The page name (case-sensitive) or UUID. Use page names as they appear in LogSeq, or the UUID from other API calls."
                                }
                            },
                            "required": ["name_or_uuid"],
                            "additionalProperties": false
                        })
                        .as_object()
                        .unwrap()
                        .clone(),
                    ),
                    annotations: None,
                    output_schema: None,
                },
                Tool {
                    name: "get_block".into(),
                    description: Some("Get detailed information about a specific block by UUID. Returns block content, properties, children, and metadata.".into()),
                    input_schema: Arc::new(
                        serde_json::json!({
                            "type": "object",
                            "properties": {
                                "uuid": {
                                    "type": "string",
                                    "description": "The UUID of the block to retrieve. UUIDs can be obtained from other API calls like create_block, search, or datascript_query."
                                }
                            },
                            "required": ["uuid"],
                            "additionalProperties": false
                        })
                        .as_object()
                        .unwrap()
                        .clone(),
                    ),
                    annotations: None,
                    output_schema: None,
                },
                Tool {
                    name: "get_current_page".into(),
                    description: Some("Get information about the currently active/focused page in the LogSeq interface. Useful for context-aware operations.".into()),
                    input_schema: Arc::new(
                        serde_json::json!({
                            "type": "object",
                            "properties": {},
                            "additionalProperties": false
                        })
                        .as_object()
                        .unwrap()
                        .clone(),
                    ),
                    annotations: None,
                    output_schema: None,
                },
                Tool {
                    name: "get_current_block".into(),
                    description: Some("Get information about the currently active/focused block in the LogSeq interface. Useful for context-aware operations.".into()),
                    input_schema: Arc::new(
                        serde_json::json!({
                            "type": "object",
                            "properties": {},
                            "additionalProperties": false
                        })
                        .as_object()
                        .unwrap()
                        .clone(),
                    ),
                    annotations: None,
                    output_schema: None,
                },
                Tool {
                    name: "datascript_query".into(),
                    description: Some("Execute a Datascript query against the LogSeq database for advanced data retrieval. Use this for complex queries that other tools cannot handle. Requires knowledge of Datascript syntax and LogSeq's data model.".into()),
                    input_schema: Arc::new(
                        serde_json::json!({
                            "type": "object",
                            "properties": {
                                "query": {
                                    "type": "string",
                                    "description": "Datascript query string. Example: '[:find ?uuid ?content :where [?b :block/uuid ?uuid] [?b :block/content ?content] :limit 10]'. Requires knowledge of LogSeq's data schema."
                                }
                            },
                            "required": ["query"],
                            "additionalProperties": false
                        })
                        .as_object()
                        .unwrap()
                        .clone(),
                    ),
                    annotations: None,
                    output_schema: None,
                },
                Tool {
                    name: "get_current_graph".into(),
                    description: Some("Get information about the current LogSeq graph including name, path, and configuration details.".into()),
                    input_schema: Arc::new(
                        serde_json::json!({
                            "type": "object",
                            "properties": {},
                            "additionalProperties": false
                        })
                        .as_object()
                        .unwrap()
                        .clone(),
                    ),
                    annotations: None,
                    output_schema: None,
                },
                Tool {
                    name: "get_state_from_store".into(),
                    description: Some("Get application state from the LogSeq store using a key path (e.g., 'ui/theme', 'ui/sidebar-open'). Useful for accessing LogSeq's internal application state.".into()),
                    input_schema: Arc::new(
                        serde_json::json!({
                            "type": "object",
                            "properties": {
                                "key": {
                                    "type": "string",
                                    "description": "State key path to retrieve from LogSeq's application store. Examples: 'ui/theme', 'ui/sidebar-open', 'config/preferred-format'."
                                }
                            },
                            "required": ["key"],
                            "additionalProperties": false
                        })
                        .as_object()
                        .unwrap()
                        .clone(),
                    ),
                    annotations: None,
                    output_schema: None,
                },
                Tool {
                    name: "get_user_configs".into(),
                    description: Some("Get user configuration settings for the LogSeq application. Returns the current user preferences and configuration options.".into()),
                    input_schema: Arc::new(
                        serde_json::json!({
                            "type": "object",
                            "properties": {},
                            "additionalProperties": false
                        })
                        .as_object()
                        .unwrap()
                        .clone(),
                    ),
                    annotations: None,
                    output_schema: None,
                },
                Tool {
                    name: "update_block".into(),
                    description: Some("Update the content of an existing block by UUID. Can also update block properties. Use this to modify existing content in LogSeq.".into()),
                    input_schema: Arc::new(
                        serde_json::json!({
                            "type": "object",
                            "properties": {
                                "uuid": {
                                    "type": "string",
                                    "description": "The UUID of the block to update. Must be an existing block UUID."
                                },
                                "content": {
                                    "type": "string",
                                    "description": "The new content for the block in markdown format. This will replace the existing block content."
                                },
                                "properties": {
                                    "type": "object",
                                    "description": "Optional block properties to update. These are key-value pairs that define metadata for the block (e.g., {'priority': 'high', 'status': 'todo'}).",
                                    "additionalProperties": true
                                }
                            },
                            "required": ["uuid", "content"],
                            "additionalProperties": false
                        })
                        .as_object()
                        .unwrap()
                        .clone(),
                    ),
                    annotations: None,
                    output_schema: None,
                },
                Tool {
                    name: "delete_block".into(),
                    description: Some("Delete an existing block by UUID. Use with caution as this operation cannot be undone. The block and all its children will be permanently removed from LogSeq.".into()),
                    input_schema: Arc::new(
                        serde_json::json!({
                            "type": "object",
                            "properties": {
                                "uuid": {
                                    "type": "string",
                                    "description": "The UUID of the block to delete. Must be an existing block UUID. This operation will also delete all child blocks."
                                }
                            },
                            "required": ["uuid"],
                            "additionalProperties": false
                        })
                        .as_object()
                        .unwrap()
                        .clone(),
                    ),
                    annotations: None,
                    output_schema: None,
                },
                Tool {
                    name: "delete_page".into(),
                    description: Some("Delete an existing page by name. Use with caution as this operation cannot be undone. The page and all its content will be permanently removed from LogSeq.".into()),
                    input_schema: Arc::new(
                        serde_json::json!({
                            "type": "object",
                            "properties": {
                                "page_name": {
                                    "type": "string",
                                    "description": "The name of the page to delete. Must be an existing page name as it appears in LogSeq. This operation will delete the entire page and all its blocks."
                                }
                            },
                            "required": ["page_name"],
                            "additionalProperties": false
                        })
                        .as_object()
                        .unwrap()
                        .clone(),
                    ),
                    annotations: None,
                    output_schema: None,
                },
                Tool {
                    name: "find_incomplete_todos".into(),
                    description: Some("Search for all incomplete todos across all pages in LogSeq. Returns todos with markers like TODO, DOING, LATER, NOW, and WAITING. Useful for getting an overview of all outstanding tasks and their current status.".into()),
                    input_schema: Arc::new(
                        serde_json::json!({
                            "type": "object",
                            "properties": {},
                            "additionalProperties": false
                        })
                        .as_object()
                        .unwrap()
                        .clone(),
                    ),
                    annotations: None,
                    output_schema: None,
                },
            ],
            next_cursor: None,
        })
    }

    async fn call_tool(
        &self,
        params: CallToolRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let client = self.get_client()?;

        match params.name.as_ref() {
            "list_pages" => {
                let pages = client
                    .get_all_pages()
                    .await
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;

                let content_text = pages
                    .iter()
                    .map(|p| format!("- {}", p.name))
                    .collect::<Vec<_>>()
                    .join("\n");

                Ok(CallToolResult {
                    content: Some(vec![rmcp::model::Content {
                        raw: RawContent::Text(RawTextContent { text: content_text }),
                        annotations: None,
                    }]),
                    structured_content: None,
                    is_error: Some(false),
                })
            }
            "get_page_content" => {
                let page_name = params
                    .arguments
                    .and_then(|args| args.get("page_name")?.as_str().map(String::from))
                    .ok_or_else(|| McpError::invalid_params("Missing page_name parameter", None))?;

                let blocks = client
                    .get_page_blocks_tree(&page_name)
                    .await
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;

                let content_text = format_blocks_as_markdown(&blocks);
                Ok(CallToolResult {
                    content: Some(vec![rmcp::model::Content {
                        raw: RawContent::Text(RawTextContent { text: content_text }),
                        annotations: None,
                    }]),
                    structured_content: None,
                    is_error: Some(false),
                })
            }
            "create_page" => {
                let arguments = params.arguments.ok_or_else(|| {
                    McpError::invalid_params("Missing arguments for create_page", None)
                })?;
                let name = arguments
                    .get("name")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| McpError::invalid_params("Missing name parameter", None))?;
                let properties = arguments
                    .get("properties")
                    .and_then(|v| serde_json::from_value(v.clone()).ok());

                let page = client
                    .create_page(name, properties)
                    .await
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;

                Ok(CallToolResult {
                    content: Some(vec![rmcp::model::Content {
                        raw: RawContent::Text(RawTextContent {
                            text: format!("Created page: {}", page.name),
                        }),
                        annotations: None,
                    }]),
                    structured_content: None,
                    is_error: Some(false),
                })
            }
            "search" => {
                let query = params
                    .arguments
                    .and_then(|args| args.get("query")?.as_str().map(String::from))
                    .ok_or_else(|| McpError::invalid_params("Missing query parameter", None))?;

                let results = client
                    .search(&query)
                    .await
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;

                let content_text = format_search_results(&results);
                Ok(CallToolResult {
                    content: Some(vec![rmcp::model::Content {
                        raw: RawContent::Text(RawTextContent { text: content_text }),
                        annotations: None,
                    }]),
                    structured_content: None,
                    is_error: Some(false),
                })
            }
            "create_block" => {
                let arguments = params.arguments.ok_or_else(|| {
                    McpError::invalid_params("Missing arguments for create_block", None)
                })?;
                let content = arguments
                    .get("content")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| McpError::invalid_params("Missing content parameter", None))?;
                let parent = arguments
                    .get("parent")
                    .and_then(|v| v.as_str())
                    .map(String::from);
                let sibling = arguments
                    .get("sibling")
                    .and_then(|v| v.as_str())
                    .map(String::from);

                let opts = InsertBlockOptions {
                    parent,
                    sibling,
                    ..Default::default()
                };

                let block = client
                    .insert_block(content, opts)
                    .await
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;

                Ok(CallToolResult {
                    content: Some(vec![rmcp::model::Content {
                        raw: RawContent::Text(RawTextContent {
                            text: format!("Created block with UUID: {}", block.uuid),
                        }),
                        annotations: None,
                    }]),
                    structured_content: None,
                    is_error: Some(false),
                })
            }
            "get_page" => {
                let name_or_uuid = params
                    .arguments
                    .and_then(|args| args.get("name_or_uuid")?.as_str().map(String::from))
                    .ok_or_else(|| {
                        McpError::invalid_params("Missing name_or_uuid parameter", None)
                    })?;

                let page = client
                    .get_page(&name_or_uuid)
                    .await
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;

                Ok(CallToolResult {
                    content: Some(vec![rmcp::model::Content {
                        raw: RawContent::Text(RawTextContent {
                            text: serde_json::to_string_pretty(&page)
                                .unwrap_or_else(|_| "Error serializing page".to_string()),
                        }),
                        annotations: None,
                    }]),
                    structured_content: None,
                    is_error: Some(false),
                })
            }
            "get_block" => {
                let uuid = params
                    .arguments
                    .and_then(|args| args.get("uuid")?.as_str().map(String::from))
                    .ok_or_else(|| McpError::invalid_params("Missing uuid parameter", None))?;

                let block = client
                    .get_block(&uuid)
                    .await
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;

                Ok(CallToolResult {
                    content: Some(vec![rmcp::model::Content {
                        raw: RawContent::Text(RawTextContent {
                            text: serde_json::to_string_pretty(&block)
                                .unwrap_or_else(|_| "Error serializing block".to_string()),
                        }),
                        annotations: None,
                    }]),
                    structured_content: None,
                    is_error: Some(false),
                })
            }
            "get_current_page" => {
                let page = client
                    .get_current_page()
                    .await
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;

                Ok(CallToolResult {
                    content: Some(vec![rmcp::model::Content {
                        raw: RawContent::Text(RawTextContent {
                            text: serde_json::to_string_pretty(&page)
                                .unwrap_or_else(|_| "Error serializing page".to_string()),
                        }),
                        annotations: None,
                    }]),
                    structured_content: None,
                    is_error: Some(false),
                })
            }
            "get_current_block" => {
                let block = client
                    .get_current_block()
                    .await
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;

                Ok(CallToolResult {
                    content: Some(vec![rmcp::model::Content {
                        raw: RawContent::Text(RawTextContent {
                            text: serde_json::to_string_pretty(&block)
                                .unwrap_or_else(|_| "Error serializing block".to_string()),
                        }),
                        annotations: None,
                    }]),
                    structured_content: None,
                    is_error: Some(false),
                })
            }
            "datascript_query" => {
                let query = params
                    .arguments
                    .and_then(|args| args.get("query")?.as_str().map(String::from))
                    .ok_or_else(|| McpError::invalid_params("Missing query parameter", None))?;

                let result = client
                    .datascript_query(&query)
                    .await
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;

                Ok(CallToolResult {
                    content: Some(vec![rmcp::model::Content {
                        raw: RawContent::Text(RawTextContent {
                            text: serde_json::to_string_pretty(&result)
                                .unwrap_or_else(|_| "Error serializing result".to_string()),
                        }),
                        annotations: None,
                    }]),
                    structured_content: None,
                    is_error: Some(false),
                })
            }
            "get_current_graph" => {
                let graph = client
                    .get_current_graph()
                    .await
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;

                Ok(CallToolResult {
                    content: Some(vec![rmcp::model::Content {
                        raw: RawContent::Text(RawTextContent {
                            text: serde_json::to_string_pretty(&graph)
                                .unwrap_or_else(|_| "Error serializing graph info".to_string()),
                        }),
                        annotations: None,
                    }]),
                    structured_content: None,
                    is_error: Some(false),
                })
            }
            "get_state_from_store" => {
                let key = params
                    .arguments
                    .and_then(|args| args.get("key")?.as_str().map(String::from))
                    .ok_or_else(|| McpError::invalid_params("Missing key parameter", None))?;

                let state = client
                    .get_state_from_store(&key)
                    .await
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;

                Ok(CallToolResult {
                    content: Some(vec![rmcp::model::Content {
                        raw: RawContent::Text(RawTextContent {
                            text: serde_json::to_string_pretty(&state)
                                .unwrap_or_else(|_| "Error serializing state".to_string()),
                        }),
                        annotations: None,
                    }]),
                    structured_content: None,
                    is_error: Some(false),
                })
            }
            "get_user_configs" => {
                let configs = client
                    .get_user_configs()
                    .await
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;

                Ok(CallToolResult {
                    content: Some(vec![rmcp::model::Content {
                        raw: RawContent::Text(RawTextContent {
                            text: serde_json::to_string_pretty(&configs)
                                .unwrap_or_else(|_| "Error serializing configs".to_string()),
                        }),
                        annotations: None,
                    }]),
                    structured_content: None,
                    is_error: Some(false),
                })
            }
            "update_block" => {
                let arguments = params.arguments.ok_or_else(|| {
                    McpError::invalid_params("Missing arguments for update_block", None)
                })?;
                let uuid = arguments
                    .get("uuid")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| McpError::invalid_params("Missing uuid parameter", None))?;
                let content = arguments
                    .get("content")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| McpError::invalid_params("Missing content parameter", None))?;
                let properties = arguments
                    .get("properties")
                    .and_then(|v| serde_json::from_value(v.clone()).ok());

                let block = client
                    .update_block(uuid, content, properties)
                    .await
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;

                Ok(CallToolResult {
                    content: Some(vec![rmcp::model::Content {
                        raw: RawContent::Text(RawTextContent {
                            text: format!("Updated block with UUID: {}", block.uuid),
                        }),
                        annotations: None,
                    }]),
                    structured_content: None,
                    is_error: Some(false),
                })
            }
            "delete_block" => {
                let uuid = params
                    .arguments
                    .and_then(|args| args.get("uuid")?.as_str().map(String::from))
                    .ok_or_else(|| McpError::invalid_params("Missing uuid parameter", None))?;

                client
                    .remove_block(&uuid)
                    .await
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;

                Ok(CallToolResult {
                    content: Some(vec![rmcp::model::Content {
                        raw: RawContent::Text(RawTextContent {
                            text: format!("Successfully deleted block with UUID: {}", uuid),
                        }),
                        annotations: None,
                    }]),
                    structured_content: None,
                    is_error: Some(false),
                })
            }
            "delete_page" => {
                let page_name = params
                    .arguments
                    .and_then(|args| args.get("page_name")?.as_str().map(String::from))
                    .ok_or_else(|| McpError::invalid_params("Missing page_name parameter", None))?;

                client
                    .delete_page(&page_name)
                    .await
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;

                Ok(CallToolResult {
                    content: Some(vec![rmcp::model::Content {
                        raw: RawContent::Text(RawTextContent {
                            text: format!("Successfully deleted page: {}", page_name),
                        }),
                        annotations: None,
                    }]),
                    structured_content: None,
                    is_error: Some(false),
                })
            }
            "find_incomplete_todos" => {
                let todos = client
                    .find_incomplete_todos()
                    .await
                    .map_err(|e| McpError::internal_error(e.to_string(), None))?;

                let content_text = format_todos(&todos);
                Ok(CallToolResult {
                    content: Some(vec![rmcp::model::Content {
                        raw: RawContent::Text(RawTextContent { text: content_text }),
                        annotations: None,
                    }]),
                    structured_content: None,
                    is_error: Some(false),
                })
            }
            _ => Err(McpError::method_not_found::<
                rmcp::model::CallToolRequestMethod,
            >()),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize environment and logging
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();

    // Create LogSeq client
    let logseq_url = env::var("LOGSEQ_API_URL").unwrap_or_else(|_| "http://localhost:12315".into());
    let logseq_token = env::var("LOGSEQ_API_TOKEN").expect("LOGSEQ_API_TOKEN must be set");
    let logseq_client = LogSeqClient::new(&logseq_url, &logseq_token)?;

    // Create and run MCP server with STDIO transport
    let service = LogSeqMcpServer::new(logseq_client);
    let server = service.serve(stdio()).await?;

    server.waiting().await?;
    Ok(())
}
