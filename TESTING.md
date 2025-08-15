# Testing Guide

This project includes both unit tests and integration tests. Integration tests spawn the actual MCP server and test the MCP tools through the MCP protocol, creating isolated test environments to avoid interfering with user data.

## Unit Tests

Run unit tests with:
```bash
cargo test --lib
```

## Integration Tests

Integration tests spawn the actual MCP server process and test all MCP tools through the MCP protocol. They require:

1. A running LogSeq instance with HTTP API enabled
2. Environment variables set for API access  
3. Manual execution (they are ignored by default)

### MCP Testing Strategy

These tests validate the entire MCP server stack:

1. **Server Process Management**: Spawn and manage MCP server processes
2. **Protocol Communication**: Test JSON-RPC communication with the MCP server
3. **Tool Validation**: Verify all MCP tools function correctly
4. **Test Isolation**: Create clearly marked test content to avoid user data conflicts
5. **Automatic Cleanup**: Terminate server processes and report created test data

### What Gets Tested

- **MCP Protocol**: Server initialization, tool listing, tool calling
- **All MCP Tools**: Every tool is tested individually and in workflows
- **Error Handling**: Invalid parameters, missing resources, API failures
- **Integration Workflows**: End-to-end scenarios using multiple tools

### Setup for Integration Tests

#### 1. Start LogSeq with HTTP API

1. Open LogSeq
2. Go to Settings → Features 
3. Enable "HTTP APIs"
4. Note the API URL (usually `http://localhost:12315`)
5. Note/create the API token

#### 2. Set Environment Variables

Set environment variables (the MCP server reads them directly):

```bash
# Required - API token from LogSeq settings  
export LOGSEQ_API_TOKEN=your_api_token_here

# Optional - defaults to http://localhost:12315
export LOGSEQ_API_URL=http://localhost:12315

# Optional - set to 1 to skip integration tests in CI
export SKIP_INTEGRATION_TESTS=0
```

#### 3. Run Integration Tests

```bash
# Run all integration tests
cargo test integration_tests -- --ignored

# Run a specific integration test
cargo test integration_tests::test_get_all_pages -- --ignored

# Run with output
cargo test integration_tests -- --ignored --nocapture
```

### MCP Integration Test Categories

The integration tests validate all aspects of the MCP server:

#### MCP Protocol Tests
- `test_mcp_server_startup_and_tools()` - Server initialization and tool discovery  
- `test_mcp_list_pages_tool()` - Basic tool functionality validation

#### Page Operations via MCP
- `test_mcp_create_and_get_page()` - Page creation and retrieval through MCP tools
- `test_mcp_get_page_content()` - Content access via MCP

#### Block Operations via MCP  
- `test_mcp_update_block()` - Block modification through MCP tools

#### Search and Query via MCP
- `test_mcp_search_tool()` - Search functionality through MCP protocol

#### Application State via MCP
- `test_mcp_app_state_tools()` - Current page, graph info, configs via MCP

#### End-to-End MCP Workflows
- `test_mcp_comprehensive_workflow()` - Complete workflows using multiple MCP tools

Each test:
- Spawns a fresh MCP server process
- Communicates via JSON-RPC over stdin/stdout  
- Validates tool responses and error handling
- Creates isolated test data with clear markers
- Cleans up server processes automatically

### CI/CD Configuration

Integration tests are automatically skipped in CI environments by setting `SKIP_INTEGRATION_TESTS=1`. 

To enable integration tests in CI (not recommended without a test LogSeq instance):
1. Set up a LogSeq instance accessible to CI
2. Configure the environment variables as secrets
3. Remove or modify the `SKIP_INTEGRATION_TESTS` setting in `.github/workflows/ci.yml`

### Troubleshooting Integration Tests

#### Common Issues

**"Integration tests skipped due to SKIP_INTEGRATION_TESTS=1"**
- Unset the environment variable: `unset SKIP_INTEGRATION_TESTS`
- Or set it to 0: `export SKIP_INTEGRATION_TESTS=0`

**"LOGSEQ_API_TOKEN must be set"**
- Ensure LogSeq HTTP API is enabled
- Set the `LOGSEQ_API_TOKEN` environment variable
- Check that the token is correct

**Connection refused errors**
- Verify LogSeq is running
- Check that HTTP API is enabled in LogSeq settings
- Verify the `LOGSEQ_API_URL` is correct

**API method failures**
- Some LogSeq API methods may have known issues
- Tests are designed to handle expected failures gracefully
- Check test output for specific error details

#### Debugging Tips

1. Run tests with output to see detailed information:
   ```bash
   cargo test integration_tests -- --ignored --nocapture
   ```

2. Run individual tests to isolate issues:
   ```bash
   cargo test integration_tests::test_get_all_pages -- --ignored --nocapture
   ```

3. Check LogSeq's developer console for API errors

4. Verify API token permissions in LogSeq settings

### Test Data Cleanup

Integration tests create test content with clear markers to identify it as test data:

- **Page Names**: Format like `test-{short-uuid}-{suffix}` (e.g., `test-a1b2c3d4-basic-operations`)
- **Content Markers**: All test content starts with `🧪 TEST [short-uuid] - {content}`
- **Properties**: All test pages/blocks include `test-id` and `test-marker` properties
- **Tracking**: Tests report all created content for reference

#### Manual Cleanup (if needed)

Since LogSeq's HTTP API doesn't include delete operations, test data remains after tests. To clean up:

1. **Search for test content**: Use LogSeq's search to find pages with `test-marker: integration-test`
2. **Filter by test run**: Search for specific `test-id` values shown in test output
3. **Delete manually**: Remove test pages and blocks through LogSeq's interface

Test content is clearly marked and doesn't interfere with normal LogSeq usage, so cleanup is optional.

### Contributing Test Cases

When adding new API functionality:

1. Add corresponding integration tests
2. Mark them with `#[ignore]` attribute  
3. Include error handling for expected failures
4. Add descriptive output for test results
5. Update this documentation if needed