use std::io;
use std::time::Duration;

use anyhow::Result;
use reqwest::Client;
use serde::Deserialize;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, ListState, Paragraph},
    Frame, Terminal,
};

// ===== DATA STRUCTURES =====

/// Represents a Hacker News story item
#[derive(Deserialize, Debug, Clone)]
struct Item {
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

/// Application state enum to handle different screens
#[derive(Debug, PartialEq)]
enum AppState {
    Loading,
    Stories,
    Error(String),
}

/// Main application state
#[derive(Debug)]
struct App {
    /// List of stories to display
    stories: Vec<Item>,
    /// Index of the currently selected story
    selected: usize,
    /// Current application state
    state: AppState,
    /// Loading progress (0-100)
    loading_progress: u16,
}

// ===== APP IMPLEMENTATION =====

impl App {
    /// Creates a new App instance with default values
    fn new() -> Self {
        Self {
            stories: Vec::new(),
            selected: 0,
            state: AppState::Loading,
            loading_progress: 0,
        }
    }

    /// Moves selection to the next story if available
    fn next(&mut self) {
        if !self.stories.is_empty() && self.selected < self.stories.len().saturating_sub(1) {
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

    /// Sets the stories and transitions to Stories state
    fn set_stories(&mut self, stories: Vec<Item>) {
        self.stories = stories;
        self.state = AppState::Stories;
        self.selected = 0;
    }

    /// Sets error state
    fn set_error(&mut self, error: String) {
        self.state = AppState::Error(error);
    }

    /// Updates loading progress
    fn update_loading_progress(&mut self, progress: u16) {
        self.loading_progress = progress.min(100);
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

/// Fetches the top 30 stories from Hacker News with progress updates
async fn fetch_stories_with_progress<F>(progress_callback: F) -> Result<Vec<Item>>
where
    F: Fn(u16),
{
    let client = Client::builder().timeout(Duration::from_secs(10)).build()?;

    progress_callback(10);

    let ids = fetch_top_story_ids(&client).await?;
    progress_callback(20);

    let mut stories = Vec::new();
    let total_ids = ids.len() as f32;

    for (index, id) in ids.iter().enumerate() {
        match fetch_item(&client, *id).await {
            Ok(item) => stories.push(item),
            Err(e) => eprintln!("Failed to fetch item {}: {}", id, e),
        }

        // Update progress (20% to 90% for fetching items)
        let progress = 20 + ((index as f32 / total_ids) * 70.0) as u16;
        progress_callback(progress);
    }

    progress_callback(100);
    Ok(stories)
}

// ===== UI FUNCTIONS =====

/// Renders the user interface for the Hacker News application
fn ui(f: &mut Frame, app: &mut App) {
    match &app.state {
        AppState::Loading => render_loading_screen(f, app),
        AppState::Stories => render_stories_screen(f, app),
        AppState::Error(error) => render_error_screen(f, error),
    }
}

/// Renders the loading screen
fn render_loading_screen(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(35),
            Constraint::Length(5),
            Constraint::Length(1),
            Constraint::Length(5),
            Constraint::Percentage(35),
        ])
        .split(f.area());

    // Title with border
    let title = Paragraph::new("📰 Hacker News TUI")
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
                .title("Welcome"),
        );
    f.render_widget(title, chunks[1]);

    // Loading bar with better styling
    let gauge = Gauge::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Loading Stories...")
                .border_style(Style::default().fg(Color::White))
                .title_style(
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
        )
        .gauge_style(Style::default().fg(Color::Cyan).bg(Color::DarkGray))
        .percent(app.loading_progress)
        .label(format!("{}%", app.loading_progress));
    f.render_widget(gauge, chunks[3]);
}

/// Renders the error screen
fn render_error_screen(f: &mut Frame, error: &str) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(30),
            Constraint::Length(7),
            Constraint::Length(1),
            Constraint::Length(5),
            Constraint::Percentage(30),
        ])
        .split(f.area());

    // Error message with better formatting
    let error_msg = Paragraph::new(format!("❌ Connection Failed\n\n{}", error))
        .style(Style::default().fg(Color::White))
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Red))
                .title("Error")
                .title_style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
        );
    f.render_widget(error_msg, chunks[1]);

    // Instructions with border
    let instructions = Paragraph::new("Press 'R' to retry or 'Q' to quit")
        .style(
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Gray))
                .title("Controls"),
        );
    f.render_widget(instructions, chunks[3]);
}

/// Renders the main stories screen
fn render_stories_screen(f: &mut Frame, app: &mut App) {
    // Split the screen into header, stories, and footer sections
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(0),    // Stories list
            Constraint::Length(3), // Footer
        ])
        .split(f.area());

    // Render the header with enhanced styling
    let title = Paragraph::new("📰 Hacker News Top Stories")
        .style(
            Style::default()
                .fg(Color::White)
                .bg(Color::Blue)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
                .title_style(
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
        );
    f.render_widget(title, chunks[0]);

    if app.stories.is_empty() {
        let empty_msg = Paragraph::new("No stories available")
            .style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::White))
                    .title("Stories"),
            );
        f.render_widget(empty_msg, chunks[1]);
    } else {
        // Create list items for each story with improved visual design
        let items: Vec<ListItem> = app
            .stories
            .iter()
            .enumerate()
            .map(|(index, story)| {
                // Format the URL display
                let url_display = if let Some(url) = &story.url {
                    if !url.is_empty() {
                        let domain = url.split('/').nth(2).unwrap_or("unknown");
                        format!(" ({})", domain)
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                };

                // Format time
                let time_str = {
                    use std::time::{SystemTime, UNIX_EPOCH};
                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_secs();
                    let diff = now.saturating_sub(story.time);
                    if diff < 3600 {
                        format!("{}m", diff / 60)
                    } else if diff < 86400 {
                        format!("{}h", diff / 3600)
                    } else {
                        format!("{}d", diff / 86400)
                    }
                };

                // Create aligned content with proper spacing
                let content = vec![
                    // Empty line for spacing
                    Line::from(""),
                    // Title line with rank number
                    Line::from(vec![
                        Span::styled(
                            format!("{:2}. ", index + 1),
                            Style::default()
                                .fg(Color::DarkGray)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            &story.title,
                            Style::default()
                                .fg(Color::White)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            url_display,
                            Style::default()
                                .fg(Color::Blue)
                                .add_modifier(Modifier::ITALIC),
                        ),
                    ]),
                    // Stats line with consistent spacing and alignment
                    Line::from(vec![
                        Span::styled("    ", Style::default()), // Indent to align with title
                        Span::styled(
                            format!("▲ {:3}", story.score),
                            Style::default()
                                .fg(Color::Green)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
                        Span::styled(
                            format!("👤 {}", story.by),
                            Style::default().fg(Color::Magenta),
                        ),
                        Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
                        Span::styled(
                            format!("💬 {:2}", story.descendants.unwrap_or(0)),
                            Style::default().fg(Color::Cyan),
                        ),
                        Span::styled(" │ ", Style::default().fg(Color::DarkGray)),
                        Span::styled(
                            format!("🕒 {}", time_str),
                            Style::default().fg(Color::Yellow),
                        ),
                    ]),
                    // Empty line for spacing
                    Line::from(""),
                ];
                ListItem::new(content).style(Style::default())
            })
            .collect();

        // Render the stories list with enhanced styling
        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::White))
                    .title(format!(
                        "📋 Stories ({}/{})",
                        app.selected + 1,
                        app.stories.len()
                    ))
                    .title_style(
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD),
                    ),
            )
            .highlight_style(
                Style::default()
                    .bg(Color::Yellow)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("➤ ");

        let mut state = ListState::default();
        state.select(Some(app.selected));

        f.render_stateful_widget(list, chunks[1], &mut state);
    }

    // Render footer with instructions
    let footer_text = "↑↓ Navigate • Enter Open Link • R Refresh • Q Quit";
    let footer = Paragraph::new(footer_text)
        .style(
            Style::default()
                .fg(Color::White)
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Gray))
                .title("Controls")
                .title_style(
                    Style::default()
                        .fg(Color::Gray)
                        .add_modifier(Modifier::BOLD),
                ),
        );
    f.render_widget(footer, chunks[2]);
}

// ===== MAIN APPLICATION LOOP =====

/// Runs the main application loop, handling user input and rendering the UI
async fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> Result<()> {
    // Start loading stories in the background
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

    let tx_clone = tx.clone();
    tokio::spawn(async move {
        match fetch_stories_with_progress(|progress| {
            let _ = tx_clone.send(AppMessage::Progress(progress));
        })
        .await
        {
            Ok(stories) => {
                let _ = tx_clone.send(AppMessage::StoriesLoaded(stories));
            }
            Err(e) => {
                let _ = tx_clone.send(AppMessage::Error(e.to_string()));
            }
        }
    });

    loop {
        // Handle background messages
        while let Ok(msg) = rx.try_recv() {
            match msg {
                AppMessage::Progress(progress) => {
                    app.update_loading_progress(progress);
                }
                AppMessage::StoriesLoaded(stories) => {
                    app.set_stories(stories);
                }
                AppMessage::Error(error) => {
                    app.set_error(error);
                }
            }
        }

        // Render the current UI state
        terminal.draw(|f| ui(f, &mut app))?;

        // Poll for user input with a timeout
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                // Only handle key press events, ignore key release
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match app.state {
                    AppState::Loading => match key.code {
                        KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => {
                            return Ok(());
                        }
                        _ => {}
                    },
                    AppState::Error(_) => {
                        match key.code {
                            KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => {
                                return Ok(());
                            }
                            KeyCode::Char('r') | KeyCode::Char('R') => {
                                // Restart loading
                                app.state = AppState::Loading;
                                app.loading_progress = 0;
                                let tx_clone = tx.clone();
                                tokio::spawn(async move {
                                    match fetch_stories_with_progress(|progress| {
                                        let _ = tx_clone.send(AppMessage::Progress(progress));
                                    })
                                    .await
                                    {
                                        Ok(stories) => {
                                            let _ =
                                                tx_clone.send(AppMessage::StoriesLoaded(stories));
                                        }
                                        Err(e) => {
                                            let _ = tx_clone.send(AppMessage::Error(e.to_string()));
                                        }
                                    }
                                });
                            }
                            _ => {}
                        }
                    }
                    AppState::Stories => {
                        match key.code {
                            KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => {
                                return Ok(());
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                app.next();
                            }
                            KeyCode::Up | KeyCode::Char('k') => {
                                app.previous();
                            }
                            KeyCode::Enter => {
                                // Open URL in browser
                                if let Some(story) = app.selected_story() {
                                    if let Some(url) = &story.url {
                                        if !url.is_empty() {
                                            let _ = open::that(url);
                                        }
                                    }
                                }
                            }
                            KeyCode::Char('r') | KeyCode::Char('R') => {
                                // Refresh stories
                                app.state = AppState::Loading;
                                app.loading_progress = 0;
                                app.stories.clear();
                                let tx_clone = tx.clone();
                                tokio::spawn(async move {
                                    match fetch_stories_with_progress(|progress| {
                                        let _ = tx_clone.send(AppMessage::Progress(progress));
                                    })
                                    .await
                                    {
                                        Ok(stories) => {
                                            let _ =
                                                tx_clone.send(AppMessage::StoriesLoaded(stories));
                                        }
                                        Err(e) => {
                                            let _ = tx_clone.send(AppMessage::Error(e.to_string()));
                                        }
                                    }
                                });
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }
}

/// Messages for background communication
#[derive(Debug)]
enum AppMessage {
    Progress(u16),
    StoriesLoaded(Vec<Item>),
    Error(String),
}

/// Main entry point for the Hacker News terminal application
#[tokio::main]
async fn main() -> Result<()> {
    // ===== TERMINAL SETUP =====
    let result = std::panic::catch_unwind(|| enable_raw_mode());

    if result.is_err() {
        eprintln!("Failed to enable raw mode. Make sure you're running in a proper terminal.");
        return Ok(());
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // ===== CREATE APP =====
    let app = App::new();

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
        eprintln!("Application error: {:?}", err);
    }

    Ok(())
}
