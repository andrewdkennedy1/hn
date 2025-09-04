use std::io;
use std::time::Duration;

use anyhow::Result;
use reqwest::Client;
use serde::Deserialize;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
    Terminal,
};

// ===== DATA STRUCTURES =====

/// Represents a Hacker News story item
#[derive(Deserialize, Debug, Clone)]
struct Item {
    /// Unique identifier for the story
    id: u64,
    /// Title of the story
    title: String,
    /// Optional URL to the original article
    #[serde(default)]
    url: Option<String>,
    /// Number of upvotes the story has received
    score: u32,
    /// Username of the person who submitted the story
    by: String,
    /// Unix timestamp when the story was submitted
    time: u64,
    /// Optional number of comments on the story
    #[serde(default)]
    descendants: Option<u32>,
}

/// Main application state
#[derive(Debug)]
struct App {
    /// List of stories to display
    stories: Vec<Item>,
    /// Index of the currently selected story
    selected: usize,
}

// ===== APP IMPLEMENTATION =====

impl App {
    /// Creates a new App instance with default values
    fn new() -> Self {
        Self {
            stories: Vec::new(),
            selected: 0,
        }
    }

    /// Moves selection to the next story if available
    fn next(&mut self) {
        if self.selected < self.stories.len().saturating_sub(1) {
            self.selected += 1;
        }
    }

    /// Moves selection to the previous story if available
    fn previous(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    /// Returns a reference to the currently selected story
    fn selected_story(&self) -> Option<&Item> {
        self.stories.get(self.selected)
    }
}

// ===== API FUNCTIONS =====

/// Fetches the top story IDs from Hacker News API
async fn fetch_top_story_ids(client: &Client) -> Result<Vec<u64>> {
    let url = "https://hacker-news.firebaseio.com/v0/topstories.json";

    let response = client.get(url).send().await?;
    let ids: Vec<u64> = response.json().await?;

    Ok(ids.into_iter().take(30).collect())
}

/// Fetches a single story item by its ID from Hacker News API
async fn fetch_item(client: &Client, id: u64) -> Result<Item> {
    let url = format!("https://hacker-news.firebaseio.com/v0/item/{}.json", id);

    let response = client.get(&url).send().await?;
    let item: Item = response.json().await?;

    Ok(item)
}

/// Fetches the top 30 stories from Hacker News
async fn fetch_stories() -> Result<Vec<Item>> {
    let client = Client::new();
    let ids = fetch_top_story_ids(&client).await?;
    let mut stories = Vec::new();

    for id in ids {
        match fetch_item(&client, id).await {
            Ok(item) => stories.push(item),
            Err(e) => eprintln!("Failed to fetch item {}: {}", id, e),
        }
    }

    Ok(stories)
}

// ===== UI FUNCTIONS =====

/// Renders the user interface for the Hacker News application
fn ui(f: &mut Frame, app: &mut App) {
    let size = f.area();

    // Split the screen into title and stories sections
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)].as_ref())
        .split(size);

    // Render the title
    let title = Paragraph::new("Hacker News Top Stories")
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, chunks[0]);

    // Create list items for each story
    let items: Vec<ListItem> = app
        .stories
        .iter()
        .enumerate()
        .map(|(i, story)| {
            let style = if i == app.selected {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let content = vec![
                Line::from(vec![Span::styled(&story.title, style)]),
                Line::from(vec![
                    Span::styled(
                        format!("{} points by {} | ", story.score, story.by),
                        Style::default().fg(Color::Gray),
                    ),
                    Span::styled(
                        format!("{} comments", story.descendants.unwrap_or(0)),
                        Style::default().fg(Color::Gray),
                    ),
                ]),
            ];
            ListItem::new(content)
        })
        .collect();

    // Render the stories list
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("Stories"))
        .highlight_style(Style::default().add_modifier(Modifier::BOLD))
        .highlight_symbol(">> ");

    let mut state = ListState::default();
    state.select(Some(app.selected));

    f.render_stateful_widget(list, chunks[1], &mut state);
}

// ===== MAIN APPLICATION LOOP =====

/// Runs the main application loop, handling user input and rendering the UI
async fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> Result<()> {
    loop {
        // Render the current UI state
        terminal.draw(|f| ui(f, &mut app))?;

        // Poll for user input with a timeout
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => {
                        return Ok(());
                    }
                    KeyCode::Down => {
                        app.next();
                    }
                    KeyCode::Up => {
                        app.previous();
                    }
                    KeyCode::Enter => {
                        if let Some(story) = app.selected_story() {
                            if let Some(url) = &story.url {
                                // Open the story URL in the default browser
                                if let Err(e) = open::that(url) {
                                    eprintln!("Failed to open URL: {}", e);
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}

/// Main entry point for the Hacker News terminal application
#[tokio::main]
async fn main() -> Result<()> {
    // ===== TERMINAL SETUP =====
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // ===== FETCH STORIES =====
    let stories = fetch_stories().await?;
    let mut app = App::new();
    app.stories = stories;

    // ===== RUN APPLICATION =====
    let res = run_app(&mut terminal, app).await;

    // ===== TERMINAL CLEANUP =====
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    // ===== ERROR HANDLING =====
    if let Err(err) = res {
        println!("{:?}", err);
    }

    Ok(())
}
