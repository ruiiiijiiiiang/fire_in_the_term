// src/main.rs
use std::{
    error::Error,
    io,
    time::{Duration, Instant},
};

use crossterm::{
    event::{self, Event as CrosstermEvent, KeyCode, KeyEventKind},
    execute,
    terminal::{
        EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
        size as terminal_size,
    },
};
use ratatui::{
    backend::CrosstermBackend,
    prelude::*,
    widgets::{Block, Borders, Paragraph},
};

/// App struct holds the state of the fireplace simulation
struct App {
    fire_grid: Vec<Vec<u8>>, // Represents heat intensity at each cell (0-255)
    width: usize,
    height: usize,
    char_map: Vec<char>,   // Characters for mapping heat intensity
    color_map: Vec<Color>, // Colors for mapping heat intensity
}

impl App {
    /// Creates a new App instance with specified fireplace dimensions.
    ///
    /// # Arguments
    ///
    /// * `width` - The initial width of the fireplace grid in characters.
    /// * `height` - The initial height of the fireplace grid in characters.
    fn new(width: usize, height: usize) -> App {
        // Define a gradient of characters, ordered from coolest to hottest.
        // The first few are spaces or very light to minimize "smoke" appearance.
        let char_map = vec![' ', '.', ',', '\'', '"', '~', '^', 'o', 'O', '*', '0', 'M'];

        // Define a gradient of colors, ordered from coolest to hottest.
        // More vibrant flame colors, fewer dark transition steps.
        let color_map = vec![
            Color::Black,              // For very low/no heat (background)
            Color::Rgb(100, 0, 0),     // Deep red, subtle embers
            Color::Rgb(175, 0, 0),     // Red
            Color::Rgb(255, 75, 50),   // Orange-Red
            Color::Rgb(255, 150, 75),  // Dark Orange
            Color::Rgb(255, 175, 100), // Orange
            Color::Yellow,             // Yellow
            Color::Rgb(255, 255, 100), // Light Yellow
            Color::White,              // White, very hot core
            Color::Rgb(255, 255, 200), // Brighter white
            Color::Rgb(255, 250, 240), // Almost pure white for brightest parts
        ];

        App {
            fire_grid: vec![vec![0; width]; height], // Initialize grid with no heat
            width,
            height,
            char_map,
            color_map,
        }
    }

    /// Resizes the fire grid and updates the App's dimensions.
    /// The grid is reinitialized to all zeros to avoid rendering artifacts from old size.
    fn resize(&mut self, new_width: usize, new_height: usize) {
        if self.width != new_width || self.height != new_height {
            self.width = new_width;
            self.height = new_height;
            self.fire_grid = vec![vec![0; self.width]; self.height]; // Reinitialize grid
        }
    }

    /// Updates the fire grid for the next animation frame.
    /// This simulates heat decay, diffusion, and new heat injection.
    fn update_fire(&mut self) {
        // Create a buffer for the next state of the grid to avoid modifying
        // the current grid while calculating new values based on its current state.
        let mut next_grid = vec![vec![0; self.width]; self.height];

        // Step 1: Heat propagation (upwards diffusion) and decay
        // Iterate from the second-to-last row up to the first row (top)
        // This simulates heat rising from below.
        for y in (0..self.height - 1).rev() {
            // Start from y = height - 2 (second to last row)
            for x in 0..self.width {
                let current_heat = self.fire_grid[y][x];
                let below_heat = self.fire_grid[y + 1][x];

                // Base heat from the cell directly below, but heavily reduced to limit upward movement.
                // Combined with a portion of the current cell's heat to create a more "flickering in place" effect.
                let mut new_cell_heat = (below_heat / 2).saturating_add(current_heat / 3);

                // Add small contributions from side neighbors (diffusion)
                if x > 0 {
                    new_cell_heat = new_cell_heat.saturating_add(self.fire_grid[y][x - 1] / 8);
                }
                if x < self.width - 1 {
                    new_cell_heat = new_cell_heat.saturating_add(self.fire_grid[y][x + 1] / 8);
                }

                // Apply decay: Higher decay to keep the flame localized
                let decay_amount = rand::random_range(15..=18); // Increased decay range
                let decayed_heat = new_cell_heat.saturating_sub(decay_amount);

                // Add random fluctuation for flickering. More intense fluctuation.
                let fluctuation = rand::random_range(12..=15);
                let final_heat = if rand::random_bool(0.5) {
                    decayed_heat.saturating_add(fluctuation)
                } else {
                    decayed_heat.saturating_sub(fluctuation)
                };

                // Ensure heat doesn't exceed max or go below zero
                next_grid[y][x] = final_heat;
            }
        }

        // Step 2: Inject new heat at the bottom (logs/fire source)
        // This is where new flames are "born"
        let log_row = self.height - 1; // The very bottom row
        for x in 0..self.width {
            // Introduce new random heat. Make it more likely in the center to shape the flame.
            let distance_from_center = (x as f32 - self.width as f32 / 2.0).abs();
            let center_bias = 1.0 - (distance_from_center / (self.width as f32 / 2.0)); // 1.0 at center, 0.0 at edges

            if rand::random_bool(center_bias.powf(0.2) as f64) {
                // Use higher power for even sharper center concentration
                // Add significant heat if biased and random chance hits
                next_grid[log_row][x] = rand::random_range(200..=255); // Inject high heat directly
            } else {
                // Ensure some heat decays completely at the bottom if not reignited
                next_grid[log_row][x] =
                    next_grid[log_row][x].saturating_sub(rand::random_range(0..=5)); // Faster decay at bottom
            }
        }

        // Update the main grid with the calculated next state
        self.fire_grid = next_grid;
    }

    /// Renders the current state of the `fire_grid` into a Ratatui `Text` object.
    /// Each heat value is converted to an ASCII character and assigned a color.
    fn render_fire(&self) -> Text {
        let mut lines = Vec::with_capacity(self.height);

        for y in 0..self.height {
            let mut spans = Vec::with_capacity(self.width);
            for x in 0..self.width {
                let heat = self.fire_grid[y][x];

                // Map heat (0-255) to an index in the `char_map`
                // Ensure index is within bounds of char_map.
                // char_map and color_map have the same length for direct mapping.
                let char_index = (heat as f32 / 255.0 * (self.char_map.len() - 1) as f32) as usize;
                let character = self.char_map[char_index];

                // Map heat (0-255) to an index in the `color_map`
                let color_index =
                    (heat as f32 / 255.0 * (self.color_map.len() - 1) as f32) as usize;
                let color = self.color_map[color_index];

                // Create a Span with the character and its determined color
                spans.push(Span::styled(
                    character.to_string(),
                    Style::default().fg(color),
                ));
            }
            // Each row of spans forms a Line
            lines.push(Line::from(spans));
        }
        // All lines together form the final Text object for the Paragraph widget
        Text::from(lines)
    }
}

/// The main application loop. Handles drawing, updating, and events.
fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> io::Result<()> {
    // Define the desired animation speed (tick rate).
    // A lower duration means more frames per second, resulting in a faster animation.
    let tick_rate = Duration::from_millis(60); // Faster tick rate for smoother animation (~16.6 FPS)
    let mut last_tick = Instant::now(); // Tracks time for consistent frame rate

    loop {
        // Draw the user interface.
        // The closure receives a `Frame` which allows rendering widgets onto the terminal.
        terminal.draw(|f| {
            let area = f.area(); // Get the current size of the terminal frame

            // Create a main block with borders and a title for the fireplace.
            let block = Block::default()
                .borders(Borders::ALL)
                .title(" Dynamic ASCII Fireplace (Press 'q' to quit) ")
                .title_alignment(Alignment::Center);

            // Render the main block to the entire frame area.
            f.render_widget(&block, area);

            // Get the inner area of the block where content (fireplace) will be drawn.
            let inner_area = block.inner(area);

            // Dynamically resize the app's internal grid to match the available drawing area.
            // This ensures the fireplace always fills the content area.
            app.resize(inner_area.width as usize, inner_area.height as usize);

            // Generate the ASCII art fire as a `Text` object from the app's state.
            let fire_text = app.render_fire();

            // Create a Paragraph widget to display the fire_text.
            // Align it to the center of its designated area.
            let paragraph = Paragraph::new(fire_text).alignment(Alignment::Center);

            // Render the paragraph (fireplace) directly into the inner_area.
            // Since `app.resize` has adjusted `app.width` and `app.height` to `inner_area`'s size,
            // this will fill the available content space.
            f.render_widget(paragraph, inner_area);
        })?;

        // Event Handling: Check for user input (specifically 'q' to quit) or terminal resize events.
        // This non-blocking poll with a timeout ensures the animation continues
        // even if no input is received.
        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if event::poll(timeout)? {
            // Read event if available
            match event::read()? {
                CrosstermEvent::Key(key) => {
                    // Only process key press events (not releases)
                    if key.kind == KeyEventKind::Press {
                        // If 'q' is pressed, exit the application loop
                        if key.code == KeyCode::Char('q') {
                            return Ok(());
                        }
                    }
                }
                CrosstermEvent::Resize(width, height) => {
                    eprintln!("Resizing to {}x{}", width, height);
                    // When terminal is resized, force a full redraw by calling terminal.autoresize()
                    // This will cause the next f.size() to pick up the new dimensions.
                    // The app.resize() call inside the draw loop handles adjusting the grid size.
                    terminal.autoresize()?;
                }
                _ => {} // Ignore other event types
            }
        }

        // Update Application State: If enough time has passed since the last tick,
        // update the fire simulation.
        if last_tick.elapsed() >= tick_rate {
            app.update_fire();
            last_tick = Instant::now(); // Reset the last tick time
        }
    }
}

/// The main entry point of the program.
fn main() -> Result<(), Box<dyn Error>> {
    // --- Terminal Setup ---
    // Enable raw mode: This allows direct control over terminal input/output,
    // which is necessary for TUI applications like Ratatui.
    enable_raw_mode()?;
    // Get a handle to the standard output.
    let mut stdout = io::stdout();
    // Enter the alternate screen buffer: This clears the terminal and
    // allows the TUI to draw without affecting the user's regular terminal history.
    execute!(stdout, EnterAlternateScreen)?;
    // Create a `CrosstermBackend` which Ratatui uses to interact with the terminal.
    let backend = CrosstermBackend::new(stdout);
    // Create a `Terminal` instance, the main rendering object for Ratatui.
    let mut terminal = Terminal::new(backend)?;

    // --- Application Initialization ---
    // Get the initial terminal size to set up the app.
    let (initial_width, initial_height) = terminal_size()?;
    let app = App::new(initial_width as usize, initial_height as usize);

    // --- Run the Application Loop ---
    // Call `run_app` to start the TUI. It will run until 'q' is pressed or an error occurs.
    let res = run_app(&mut terminal, app);

    // --- Terminal Restoration ---
    // Disable raw mode: Return the terminal to its normal operating state.
    disable_raw_mode()?;
    // Leave the alternate screen buffer: Restore the original terminal content.
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    // Show the cursor: Ensure the terminal cursor is visible again.
    terminal.show_cursor()?;

    // --- Error Handling ---
    // If the `run_app` function returned an error, print it to stderr.
    if let Err(err) = res {
        eprintln!("{err:?}");
    }

    Ok(()) // Indicate successful execution
}
