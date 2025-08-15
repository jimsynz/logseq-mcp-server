use crate::logseq::api::{Block, SearchResult};

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