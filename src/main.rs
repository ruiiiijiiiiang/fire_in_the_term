use crossterm::{
    event::{self, Event as CrosstermEvent, KeyCode, KeyEventKind},
    execute,
    terminal::{
        EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
        size as terminal_size,
    },
};
use rand::{random_bool, random_range};
use ratatui::{
    backend::CrosstermBackend,
    prelude::*,
    widgets::{Block, Borders, Paragraph},
};
use std::{
    error::Error,
    io,
    time::{Duration, Instant},
};

struct App {
    fire_grid: Vec<Vec<u8>>,
    width: usize,
    height: usize,
    char_map: Vec<Vec<char>>,
    color_map: Vec<Color>,
}

impl App {
    fn new(width: usize, height: usize) -> App {
        let char_map = vec![
            vec![' '],
            vec!['.', '\'', '`', ','],
            vec!['~', '-', ';', ':'],
            vec!['"', ';', ':', '^'],
            vec!['!', '?', '=', '"'],
            vec!['(', ')', '|', '!'],
            vec!['[', ']', '\\', '/'],
            vec!['{', '}', 'I', 'V'],
            vec!['o', 'T', 'O', 'V'],
            vec!['H', 'A', '0', '*'],
            vec!['M', 'W', '%', 'X'],
            vec!['#', '$', '@', '&'],
        ];

        let color_map = vec![
            Color::Black,              // For very low/no heat (background)
            Color::Rgb(175, 0, 0),     // Deep red, subtle embers
            Color::Rgb(255, 0, 0),     // Red
            Color::Rgb(255, 150, 50),  // Orange-Red
            Color::Rgb(255, 175, 75),  // Dark Orange
            Color::Rgb(255, 200, 100), // Orange
            Color::Yellow,             // Yellow
            Color::Rgb(255, 255, 150), // Light Yellow
            Color::White,              // White, very hot core
            Color::Rgb(255, 255, 200), // Brighter white
            Color::Rgb(255, 255, 250), // Almost pure white for brightest parts
        ];

        App {
            fire_grid: vec![vec![0; width]; height],
            width,
            height,
            char_map,
            color_map,
        }
    }

    fn resize(&mut self, new_width: usize, new_height: usize) {
        if self.width != new_width || self.height != new_height {
            self.width = new_width;
            self.height = new_height;
            self.fire_grid = vec![vec![0; self.width]; self.height];
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
                let decay_amount = random_range(15..=18);
                let decayed_heat = new_cell_heat.saturating_sub(decay_amount);

                // Add random fluctuation for flickering. More intense fluctuation.
                let fluctuation = random_range(12..=15);
                next_grid[y][x] = if random_bool(0.5) {
                    decayed_heat.saturating_add(fluctuation)
                } else {
                    decayed_heat.saturating_sub(fluctuation)
                };
            }
        }

        // Step 2: Inject new heat at the bottom (logs/fire source)
        // This is where new flames are "born"
        let log_row = self.height - 1; // The very bottom row
        for x in 0..self.width {
            // Introduce new random heat. Make it more likely in the center to shape the flame.
            let distance_from_center = (x as f32 - self.width as f32 / 2.0).abs();
            let center_bias = 1.0 - (distance_from_center / (self.width as f32 / 2.0)); // 1.0 at center, 0.0 at edges

            if random_bool(center_bias.powf(0.2) as f64) {
                // Use higher power for even sharper center concentration
                // Add significant heat if biased and random chance hits
                next_grid[log_row][x] = random_range(200..=255);
            } else {
                // Ensure some heat decays completely at the bottom if not reignited
                next_grid[log_row][x] = next_grid[log_row][x].saturating_sub(random_range(5..=10));
            }
        }

        self.fire_grid = next_grid;
    }

    fn render_fire(&self) -> Text<'_> {
        let mut lines = Vec::with_capacity(self.height);

        for y in 0..self.height {
            let mut spans = Vec::with_capacity(self.width);
            for x in 0..self.width {
                let heat = self.fire_grid[y][x];

                let char_index = (heat as f32 / 255.0 * (self.char_map.len() - 1) as f32) as usize;
                let char_random_index = random_range(0..self.char_map[char_index].len());
                let character = self.char_map[char_index][char_random_index];

                let color_index =
                    (heat as f32 / 255.0 * (self.color_map.len() - 1) as f32) as usize;
                let color = self.color_map[color_index];

                spans.push(Span::styled(
                    character.to_string(),
                    Style::default().fg(color),
                ));
            }
            lines.push(Line::from(spans));
        }
        Text::from(lines)
    }
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> io::Result<()> {
    let tick_rate = Duration::from_millis(60); // ~16.6 FPS
    let mut last_tick = Instant::now();

    loop {
        terminal.draw(|f| {
            let area = f.area();
            let block = Block::default().borders(Borders::ALL);
            f.render_widget(&block, area);
            let inner_area = block.inner(area);
            app.resize(inner_area.width as usize, inner_area.height as usize);

            let fire_text = app.render_fire();
            let paragraph = Paragraph::new(fire_text).alignment(Alignment::Center);
            f.render_widget(paragraph, inner_area);
        })?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if event::poll(timeout)? {
            match event::read()? {
                CrosstermEvent::Key(key) => {
                    if key.kind == KeyEventKind::Press {
                        match key.code {
                            KeyCode::Char('q') => return Ok(()),
                            KeyCode::Char('c') if key.modifiers == event::KeyModifiers::CONTROL => {
                                return Ok(());
                            }
                            _ => {}
                        }
                    }
                }
                CrosstermEvent::Resize(width, height) => {
                    eprintln!("Resizing to {}x{}", width, height);
                    terminal.autoresize()?;
                }
                _ => {}
            }
        }

        if last_tick.elapsed() >= tick_rate {
            app.update_fire();
            last_tick = Instant::now();
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let (initial_width, initial_height) = terminal_size()?;
    let app = App::new(initial_width as usize, initial_height as usize);
    let res = run_app(&mut terminal, app);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("{err:?}");
    }
    Ok(())
}
