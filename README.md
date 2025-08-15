# 🗃️ LogSeq MCP Server

A Model Context Protocol (MCP) server that provides comprehensive access to LogSeq's HTTP API, enabling AI assistants like Claude to interact with your LogSeq knowledge graph. ✨

## 📋 Overview

This server bridges LogSeq and MCP clients by exposing LogSeq's HTTP API as MCP tools. It allows AI assistants to:
- 📖 Read and create pages and blocks
- 🔍 Search your knowledge graph  
- 🗄️ Execute powerful Datascript queries
- ⚙️ Access graph metadata and configuration

The server communicates with LogSeq's built-in HTTP API server, which runs locally alongside your LogSeq application. 🖥️

## 📦 Installation

Install directly from the Git repository using Cargo:

```bash
cargo install --locked --git https://harton.dev/james/logseq-mcp-server.git logseq-mcp-server
```

## 🛠️ Setup

### 1. 🔧 Enable LogSeq HTTP API

#### Step-by-Step Instructions:

1. **Open LogSeq Settings** 📱
   - Launch LogSeq on your computer
   - Click the **Settings** icon (usually in the top-right corner)

2. **Navigate to Features** 🎛️
   - In the Settings menu, find and click on **Features**
   - This opens the features configuration panel

3. **Enable HTTP APIs Server** 🌐
   - Look for **"HTTP APIs Server"** in the features list
   - Toggle the switch to **enable** the HTTP APIs Server
   - The server will start automatically once enabled

4. **Configure Authorization Token** 🔑
   - After enabling the API server, you need to set up authentication
   - Look for **"Authorization tokens"** section in the API settings
   - Click **"Add new token"**
   - Fill in the token details:
     - **Name**: `logseq-mcp-server` (or any descriptive name)
     - **Value**: Generate a secure random string (e.g., `mcp-token-abc123def456`)
   - **Save** the token configuration

5. **Configure Auto-Start (Recommended)** 🚀
   - In the **"Server configurations"** section
   - Enable **"Auto start server with the app launched"**
   - Click **"Save & Apply"**

6. **Start the Server** ▶️
   - If auto-start is not enabled, manually start the server
   - Look for **"Start Server"** button in the API panel
   - The server typically runs on `http://localhost:12315`

> **💡 Pro Tip**: Keep your authorization token secure and private. It grants access to your entire LogSeq graph!

### 2. 🔐 Configure Environment Variables

The server requires these environment variables:

```bash
export LOGSEQ_API_URL="http://localhost:12315"  # Default LogSeq API URL
export LOGSEQ_API_TOKEN="your-api-token-here"  # Token from LogSeq settings
```

### 3. 🤖 Configure Claude Desktop

Add the server to your `claude_desktop_config.json`:

📍 **macOS**: `~/Library/Application Support/Claude/claude_desktop_config.json`  
📍 **Windows**: `%APPDATA%\Claude\claude_desktop_config.json`

```json
{
  "mcpServers": {
    "logseq": {
      "command": "logseq-mcp-server",
      "env": {
        "LOGSEQ_API_URL": "http://localhost:12315",
        "LOGSEQ_API_TOKEN": "your-api-token-here"
      }
    }
  }
}
```

### 4. 🔄 Restart Claude Desktop

Restart Claude Desktop to load the new server configuration. 🚀

## 🧰 Available Tools

The server provides 13 MCP tools organized into these categories:

### 📄 Page Management
- **`list_pages`** 📋 - List all pages in your LogSeq graph
- **`get_page`** 📃 - Get specific page information by name or UUID
- **`get_page_content`** 📝 - Get page content formatted as markdown
- **`create_page`** ➕ - Create new pages with optional properties (tags, template, alias, etc.)
- **`get_current_page`** 👁️ - Get the currently active page

### 🧱 Block Operations
- **`get_block`** 🟦 - Get specific block by UUID
- **`create_block`** ✏️ - Insert new blocks with positioning options
- **`update_block`** 📝 - Update the content of an existing block
- **`get_current_block`** 🎯 - Get the currently active block

### 🔍 Search & Query
- **`search`** 🕵️ - Search across all pages using LogSeq's built-in search
- **`datascript_query`** 🗄️ - Execute Datascript queries against the LogSeq database

### ⚙️ Application Info
- **`get_current_graph`** 🌐 - Get information about the current graph
- **`get_user_configs`** 👤 - Get user configuration settings
- **`get_state_from_store`** 💾 - Get application state values (theme, UI settings, etc.)

## 🚀 Example Usage with Claude

Once configured, you can ask Claude to:

```
"Show me all my pages about machine learning" 🤖
"Create a new page called 'Project Ideas' with the tag 'brainstorming'" 💡
"Search for blocks containing 'TODO' and show me the results" ✅
"What's the current theme setting in my LogSeq?" 🎨
"Execute this Datascript query to find all pages created this week" 📅
```

## 🔬 Advanced: Datascript Queries

Use the `datascript_query` tool for powerful database queries:

```datalog
# Find all blocks with content
[:find ?e :where [?e :block/content]]

# Find all pages
[:find ?page :where [?page :block/name]]

# Find all TODO/DOING blocks
[:find ?h :where [?h :block/marker]]

# Find blocks referencing a specific page
[:find ?b :where [?b :block/refs ?r] [?r :block/name "Project Ideas"]]
```

## 🔧 Troubleshooting

### ⚠️ Common Issues

1. **"401 Unauthorized"** 🚫 - Check that your API token is correct
2. **"Connection refused"** 🔌 - Ensure LogSeq is running and HTTP API is enabled
3. **"Method not found"** ❓ - Verify you're using the correct API method names

### 📊 Logging

The server logs to stderr. To see detailed logs:

```bash
RUST_LOG=debug logseq-mcp-server
```

### 🧪 Testing with MCP Inspector

You can test the server using the MCP Inspector:

```bash
npx @modelcontextprotocol/inspector
```

Configure it to use `logseq-mcp-server` as the command.

## 📋 Supported LogSeq API Methods

This server exposes the following confirmed working LogSeq HTTP API methods:

- ✅ `logseq.Editor.getAllPages`
- ✅ `logseq.Editor.getPage` 
- ✅ `logseq.Editor.getPageBlocksTree`
- ✅ `logseq.Editor.createPage`
- ✅ `logseq.Editor.getBlock`
- ✅ `logseq.Editor.getCurrentPage`
- ✅ `logseq.Editor.getCurrentBlock`
- ✅ `logseq.Editor.insertBlock`
- ✅ `logseq.Editor.updateBlock`
- ✅ `logseq.DB.datascriptQuery`
- ✅ `logseq.App.getCurrentGraph`
- ✅ `logseq.App.getStateFromStore`
- ✅ `logseq.App.getUserConfigs`

## 🤝 Contributing

This project welcomes contributions! Please feel free to:
- 🐛 Report bugs and issues
- 💡 Suggest new features
- 🔧 Submit pull requests
- 📚 Improve documentation

## 📄 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 👨‍💻 Author

**James Harton** <james@harton.nz>

## 🔗 Links

- 📦 **Repository**: https://harton.dev/james/logseq-mcp-server
- 🗃️ **LogSeq**: https://logseq.com/
- 🤖 **Model Context Protocol**: https://modelcontextprotocol.io/

## Github Mirror

This repository is mirrored [on Github](https://github.com/jimsynz/logseq-mcp-server) from it's primary location [on my Forgejo instance](https://harton.dev/james/logseq-mcp-server). Feel free to raise issues and open PRs on Github.
