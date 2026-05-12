use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use rand::Rng;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph},
    Terminal,
};
use std::{
    io,
    time::{Duration, Instant},
};

#[derive(Clone)]
struct World {
    width: usize,
    height: usize,
    cells: Vec<bool>,
    generation: u64,
}

impl World {
    fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            cells: vec![false; width * height],
            generation: 0,
        }
    }

    fn idx(&self, x: usize, y: usize) -> usize {
        y * self.width + x
    }

    fn alive(&self, x: isize, y: isize) -> bool {
        if x < 0 || y < 0 {
            return false;
        }

        let x = x as usize;
        let y = y as usize;

        if x >= self.width || y >= self.height {
            return false;
        }

        self.cells[self.idx(x, y)]
    }

    fn randomize(&mut self) {
        let mut rng = rand::thread_rng();

        for cell in &mut self.cells {
            *cell = rng.gen_bool(0.28);
        }

        self.generation = 0;
    }

    fn clear(&mut self) {
        self.cells.fill(false);
        self.generation = 0;
    }

    fn seed_glider(&mut self) {
        self.clear();

        let cx = self.width / 2;
        let cy = self.height / 2;

        let points = [
            (cx + 1, cy),
            (cx + 2, cy + 1),
            (cx, cy + 2),
            (cx + 1, cy + 2),
            (cx + 2, cy + 2),
        ];

        for (x, y) in points {
            if x < self.width && y < self.height {
                let idx = self.idx(x, y);
                self.cells[idx] = true;
            }
        }
    }

    fn tick(&mut self) {
        let mut next = self.cells.clone();

        for y in 0..self.height {
            for x in 0..self.width {
                let mut neighbors = 0;

                for dy in -1..=1 {
                    for dx in -1..=1 {
                        if dx == 0 && dy == 0 {
                            continue;
                        }

                        if self.alive(x as isize + dx, y as isize + dy) {
                            neighbors += 1;
                        }
                    }
                }

                let idx = self.idx(x, y);
                let is_alive = self.cells[idx];

                next[idx] = matches!((is_alive, neighbors), (true, 2) | (true, 3) | (false, 3));
            }
        }

        self.cells = next;
        self.generation += 1;
    }

    fn living_count(&self) -> usize {
        self.cells.iter().filter(|cell| **cell).count()
    }

    fn resize(&mut self, width: usize, height: usize) {
        if width == self.width && height == self.height {
            return;
        }

        let mut new_world = World::new(width, height);

        let copy_w = self.width.min(width);
        let copy_h = self.height.min(height);

        for y in 0..copy_h {
            for x in 0..copy_w {
                let old_idx = self.idx(x, y);
                let new_idx = new_world.idx(x, y);
                new_world.cells[new_idx] = self.cells[old_idx];
            }
        }

        new_world.generation = self.generation;
        *self = new_world;
    }
}

struct App {
    world: World,
    paused: bool,
    tick_rate: Duration,
    last_tick: Instant,
}

impl App {
    fn new(width: usize, height: usize) -> Self {
        let mut world = World::new(width, height);
        world.randomize();

        Self {
            world,
            paused: false,
            tick_rate: Duration::from_millis(120),
            last_tick: Instant::now(),
        }
    }

    fn faster(&mut self) {
        let millis = self.tick_rate.as_millis() as u64;
        self.tick_rate = Duration::from_millis(millis.saturating_sub(20).max(20));
    }

    fn slower(&mut self) {
        let millis = self.tick_rate.as_millis() as u64;
        self.tick_rate = Duration::from_millis((millis + 20).min(1000));
    }
}

fn main() -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let size = terminal.size()?;
    let width = size.width.saturating_sub(2).max(10) as usize;
    let height = size.height.saturating_sub(6).max(10) as usize;

    let mut app = App::new(width, height);

    let result = run(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn run(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> Result<()> {
    loop {
        terminal.draw(|frame| {
            let area = frame.area();

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(5),
                    Constraint::Length(3),
                ])
                .split(area);

            let grid_width = chunks[0].width.saturating_sub(2).max(10) as usize;
            let grid_height = chunks[0].height.saturating_sub(2).max(10) as usize;

            app.world.resize(grid_width, grid_height);

            let mut lines = Vec::with_capacity(app.world.height);

            for y in 0..app.world.height {
                let mut row = String::with_capacity(app.world.width);

                for x in 0..app.world.width {
                    let idx = app.world.idx(x, y);
                    row.push(if app.world.cells[idx] { '●' } else { '·' });
                }

                lines.push(Line::from(row));
            }

            let title = format!(
                " Conway's Game of Life | Gen {} | Alive {} | {}ms | {} ",
                app.world.generation,
                app.world.living_count(),
                app.tick_rate.as_millis(),
                if app.paused { "PAUSED" } else { "RUNNING" }
            );

            let world = Paragraph::new(lines)
                .style(Style::default().fg(Color::Green))
                .block(Block::default().title(title).borders(Borders::ALL));

            frame.render_widget(world, chunks[0]);

            let help = Paragraph::new("Space pause/resume | n step | r randomize | g glider | c clear | + faster | - slower | q quit")
                .style(Style::default().fg(Color::Gray))
                .block(Block::default().title(" Controls ").borders(Borders::ALL));

            frame.render_widget(help, chunks[1]);
        })?;

        if event::poll(Duration::from_millis(10))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Char(' ') => app.paused = !app.paused,
                    KeyCode::Char('n') => app.world.tick(),
                    KeyCode::Char('r') => app.world.randomize(),
                    KeyCode::Char('g') => app.world.seed_glider(),
                    KeyCode::Char('c') => app.world.clear(),
                    KeyCode::Char('+') | KeyCode::Char('=') => app.faster(),
                    KeyCode::Char('-') | KeyCode::Char('_') => app.slower(),
                    _ => {}
                }
            }
        }

        if !app.paused && app.last_tick.elapsed() >= app.tick_rate {
            app.world.tick();
            app.last_tick = Instant::now();
        }
    }

    Ok(())
}
