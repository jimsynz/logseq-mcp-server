use crate::logseq::api::{Block, SearchResult, TodoItem};

pub fn format_blocks_as_markdown(blocks: &[Block]) -> String {
    let mut result = String::new();
    for block in blocks {
        format_block_recursive(&mut result, block, 0);
    }
    result
}

fn format_block_recursive(result: &mut String, block: &Block, indent_level: usize) {
    let indent = "  ".repeat(indent_level);
    result.push_str(&format!("{}* {}\n", indent, block.content));

    for child in &block.children {
        format_block_recursive(result, child, indent_level + 1);
    }
}

pub fn format_search_results(results: &[SearchResult]) -> String {
    if results.is_empty() {
        return "No results found.".to_string();
    }

    let mut content = String::new();
    content.push_str(&format!("Found {} results:\n\n", results.len()));

    for (i, result) in results.iter().enumerate() {
        content.push_str(&format!("{}. {}\n", i + 1, result.block.content));
        if let Some(page) = &result.block.page {
            content.push_str(&format!("   Page ID: {}\n", page.id));
        }
        if let Some(score) = result.score {
            content.push_str(&format!("   Score: {:.2}\n", score));
        }
        content.push('\n');
    }

    content
}

pub fn format_todos(todos: &[TodoItem]) -> String {
    if todos.is_empty() {
        return "No incomplete todos found.".to_string();
    }

    let mut content = String::new();
    content.push_str(&format!("Found {} incomplete todos:\n\n", todos.len()));

    // Group todos by marker type for better organization
    let mut by_marker: std::collections::HashMap<&str, Vec<&TodoItem>> =
        std::collections::HashMap::new();
    for todo in todos {
        by_marker.entry(&todo.marker).or_default().push(todo);
    }

    // Sort by marker priority: NOW > DOING > TODO > LATER > WAITING
    let marker_order = ["NOW", "DOING", "TODO", "LATER", "WAITING"];

    for marker in marker_order {
        if let Some(marker_todos) = by_marker.get(marker) {
            content.push_str(&format!("## {} ({} items)\n", marker, marker_todos.len()));

            for (i, todo) in marker_todos.iter().enumerate() {
                content.push_str(&format!(
                    "{}. **{}** {}\n",
                    i + 1,
                    todo.marker,
                    todo.content
                ));
                content.push_str(&format!("   📄 Page: {}\n", todo.page_name));
                content.push_str(&format!("   🆔 UUID: {}\n", todo.uuid));
                content.push('\n');
            }
        }
    }

    // Add summary
    content.push_str("---\n");
    content.push_str("**Summary by Status:**\n");
    for marker in marker_order {
        if let Some(marker_todos) = by_marker.get(marker) {
            content.push_str(&format!("- {}: {} todos\n", marker, marker_todos.len()));
        }
    }

    content
}
