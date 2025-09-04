# Hacker News TUI

A beautiful terminal user interface for browsing Hacker News top stories, built with Rust and Ratatui.

## Features

- 🚀 **Fast Loading**: Asynchronous story fetching with progress indicator
- 🎨 **Beautiful UI**: Clean, colorful terminal interface with emojis
- ⌨️ **Keyboard Navigation**: Vim-style and arrow key navigation
- 🔗 **URL Handling**: View URLs in terminal or open in browser
- 🔄 **Refresh**: Reload stories on demand
- 🛡️ **Error Handling**: Graceful error recovery with retry option

## Installation

```bash
cargo build --release
```

## Usage

Run the application:
```bash
cargo run --release
```

### Keyboard Controls

**Loading Screen:**
- `q` / `Q` / `Esc` - Quit application

**Error Screen:**
- `r` / `R` - Retry loading stories
- `q` / `Q` / `Esc` - Quit application

**Stories Screen:**
- `↑` / `k` - Move up
- `↓` / `j` - Move down  
- `Enter` - Show story details and URL
- `o` / `O` - Open story URL in browser
- `r` / `R` - Refresh stories
- `q` / `Q` / `Esc` - Quit application

## Story Information

Each story displays:
- 📰 **Title** with domain name
- 👍 **Score** (upvotes)
- 👤 **Author** username
- 💬 **Comments** count
- 🕒 **Time** posted (relative)

## Dependencies

- `tokio` - Async runtime
- `reqwest` - HTTP client
- `serde` - JSON serialization
- `ratatui` - Terminal UI framework
- `crossterm` - Cross-platform terminal handling
- `anyhow` - Error handling
- `open` - Browser integration

## Architecture

The application uses a state machine with three main states:
1. **Loading** - Fetching stories with progress bar
2. **Stories** - Main interface showing story list
3. **Error** - Error display with retry option

Stories are fetched asynchronously from the Hacker News API, with progress updates sent via channels to the UI thread.