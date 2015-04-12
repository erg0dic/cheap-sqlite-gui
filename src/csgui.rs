use osext::{get_env_as};
use sqlite::{Sqlite, MAX_TABLE_NAME_LENGTH, exec_results};
use curses::{Window, Curses};

// Keys
const KEY_q : usize = 113;
const KEY_h : usize = 104;
const KEY_j : usize = 106;
const KEY_k : usize = 107;
const KEY_l : usize = 108;
const KEY_e : usize = 101;

// Dimensions
pub static DEFAULT_WIDTH : usize = 80;
pub static DEFAULT_HEIGHT : usize = 40;
pub static CELL_WIDTH : usize = 32;

/// Represents a Window with a connection to a database
#[derive(Clone)]
pub struct CSGWindow {
    sqlite : Sqlite,
	window : Window,
    text : String,
    selectable: bool
}

impl CSGWindow {
	pub fn new(sqlite : Sqlite,
			   selectable : bool,
			   nlines : usize, 
               ncols : usize, 
               begin_y : usize, 
               begin_x : usize) -> CSGWindow {
		CSGWindow {
			sqlite: sqlite,
			window: Window::new(nlines,
								ncols,
								begin_y,
								begin_x),
			text: "".to_string(),
			selectable: selectable
		}
	}
	/// Set window text
	pub fn set_text(&mut self, s : String) {
		self.text = s.clone();
		self.window.write(s.as_slice());
	}
	/// Emphasizes text
	pub fn highlight(s : String) -> String {
		["*".to_string(), s, "*".to_string()].concat()
	}
	/// Designates a window as selected
    pub fn select(&self) {
        self.window.write(CSGWindow::highlight(self.text.clone()).as_slice())
    }

    /// Undesignates a window as selected
    pub fn unselect(&self) {
        self.window.write(self.text.as_slice())
    }
}

/// Represents the type of view
#[derive(Clone)]
pub enum ScreenKind {
    TableList,
    TableDump
}

/// Represents a view to be displayed
#[derive(Clone)]
pub struct CSGScreen {
    sqlite : Sqlite,
    rows : usize,
    cols : usize,
    windows : Vec<Vec<Option<CSGWindow>>>,
    active_window : (usize, usize),
    kind : ScreenKind
}

impl CSGScreen {
    /// Initializes a 2D vector of windows to all None
    fn init_windows(rows : usize, cols : usize) -> Vec<Vec<Option<CSGWindow>>> {
        let mut windows : Vec<Vec<Option<CSGWindow>>> = Vec::new();
        for _ in (0..rows) {
            let mut row : Vec<Option<CSGWindow>> = Vec::new();
            for _ in (0..cols) {
                row.push(None);
            }
            windows.push(row);
        }
        windows
    }

    // Factory constructor for table dump screens
    fn new_table_dump(sqlite : Sqlite, width : usize, height : usize, table : String) -> CSGScreen {
        sqlite.dump_table(table);

        let results = exec_results.lock().unwrap();
        let mut windows : Vec<Vec<Option<CSGWindow>>> = CSGScreen::init_windows(height, width);
        let mut x : usize = 0;
        let mut y : usize = 0;

        // Column names
        for result in (*results).iter() {
            for name in result.col_names.iter() {
                let mut window = CSGWindow::new(sqlite.clone(), false, 1, CELL_WIDTH, y, x * CELL_WIDTH);
                window.set_text(name.clone());
                windows[x][y] = Some(window);
                x = x + 1;
            }
            x = 0;
            y = y + 1;
            break;
        }

        // Rows
        for result in (*results).iter() {
            for text in result.col_text.iter() {
                let mut window = CSGWindow::new(sqlite.clone(), true, 1, CELL_WIDTH, y, x * CELL_WIDTH);
                window.set_text(text.clone());
                if x == 0  && y == 1 {
                    window.select()
                }
                windows[x][y] = Some(window);
                x = x + 1;
            }
            x = 0;
            y = y + 1;
        }

        CSGScreen {
            sqlite : sqlite,
            rows : height,
            cols : width,
            windows : windows,
            active_window : (0, 1),
            kind : ScreenKind::TableDump
        }
    }

    // Factory constructor for table list screens
    fn new_table_list(sqlite : Sqlite, width : usize, height : usize) -> CSGScreen {
        sqlite.list_tables();

        let results = exec_results.lock().unwrap();
        let rows = height;
        let cols = width / CELL_WIDTH;
        let mut windows : Vec<Vec<Option<CSGWindow>>> = CSGScreen::init_windows(rows, cols);
        let mut y : usize = 0;

        for result in (*results).iter() {
            for text in result.col_text.iter() {
                let mut window = CSGWindow::new(sqlite.clone(), true, 1, CELL_WIDTH, y, 0);
                window.set_text(text.clone());
                if y == 0 {
                    window.select();
                }
                windows[0][y] = Some(window);
                y = y + 1;
            }
        }

        CSGScreen {
            sqlite : sqlite,
            rows : rows,
            cols : cols,
            windows : windows,
            active_window : (0, 0),
            kind : ScreenKind::TableList
        }
    }

    /// Get method for active window
    fn get_active_window(&self) -> Option<&CSGWindow> {
        self.windows[self.active_window.0][self.active_window.1].as_ref()
    }

    /// Set method for active window
    fn set_active_window(&mut self, prev : (i64, i64), next : (i64, i64)) {
        let (x, y) : (i64, i64) = match self.get_window(next.0, next.1) {
            Some(w) => {
                match w.selectable {
                    true => { 
                        w.select();
                        next
                    },
                    false => { prev }
                }
            },
            None => { prev }
        };
        if x != prev.0 || y != prev.1 {
            let prev_w = self.get_window(prev.0, prev.1).unwrap();
            prev_w.unselect();
        }
        self.active_window = (x as usize, y as usize);
    }

    /// Get method for a pointer to a window
    fn get_window(&mut self, x : i64, y : i64) -> Option<&mut CSGWindow> {
        if x < 0 || x >= self.rows as i64 || y < 0 || y >= self.cols as i64 {
            return None
        }
        self.windows[x as usize][y as usize].as_mut()
    }

    // Clears all window text in this screen
    fn clear_all(&self) {
        for i in (0..self.rows) {
            for j in (0..self.cols) {
                match self.windows[i][j] {
                    Some(ref w) => { w.window.clear(); },
                    None => { continue; }
                }
            }
        }
    }

    // Draws all window text in this screen
    fn write_all(&self) {
        for i in (0..self.rows) {
            for j in (0..self.cols) {
                match self.windows[i][j] {
                    Some(ref w) => { 
                    	w.window.write(w.text.as_slice()); 
                    	if (i, j) == self.active_window {
                    		w.select();
                    	}
                    },
                    None => { continue; }
                }
            }
        }
    }
}

/// Encapsulating structure for the user interface
pub struct CSG {
    sqlite : Sqlite,
    curses : Curses,
    width : usize,
    height : usize,
    screens : Vec<CSGScreen>,
    active_screen : usize
}

impl CSG {
	pub fn new(filename : &str) -> CSG {
        let sqlite = Sqlite::new(filename);
        let curses = Curses::new();
        let width = match get_env_as::<usize>("COLUMNS") {
            Some(n) => n,
            None => DEFAULT_WIDTH
        };
        let height = match get_env_as::<usize>("LINES") {
            Some(n) => n,
            None => DEFAULT_HEIGHT
        };
        let mut screens : Vec<CSGScreen> = Vec::new();
        let main_screen = CSGScreen::new_table_list(sqlite.clone(), width, height);
        screens.push(main_screen);

        CSG {
            sqlite : sqlite,
            curses : curses,
            width : width,
            height : height,
            screens : screens,
            active_screen : 0,
        }
	}


    /// Main loop, handles keystrokes & dispatches events
    pub fn run_forever(&mut self) {
        loop {
            let prev = self.get_active_window_coords();
            let (mut x, mut y) = prev;
            let c = self.read_current_window();

            match c {
                KEY_q => { 
                    self.screens[self.active_screen].clear_all();
                    self.screens.pop();
                    if self.screens.len() == 0 {
                        break;
                    }
                    self.active_screen = self.active_screen - 1;
                    self.screens[self.active_screen].write_all();
                },
                KEY_h => {
                    x = x - 1;
                    self.set_active_window(prev, (x, y));
                },
                KEY_j => {
                    y = y + 1;
                    self.set_active_window(prev, (x, y));
                },
                KEY_k => {
                    y = y - 1;
                    self.set_active_window(prev, (x, y));
                },
                KEY_l => {
                    x = x + 1;
                    self.set_active_window(prev, (x, y));
                },
                KEY_e => {
                    self.handle_edit();
                },
                _ => { continue; }
            }
        }
    }

    // Dispatches an edit depending on the kind of screen we are on
    fn handle_edit(&mut self) {
        match self.screens[self.active_screen].kind {
            ScreenKind::TableList => {
                self.screens[self.active_screen].clear_all();

                let active_window_text = self.get_active_window().unwrap().text.clone();
                let table_dump_screen = CSGScreen::new_table_dump(self.sqlite.clone(),
                                                                  self.width,
                                                                  self.height,
                                                                  active_window_text);
                self.add_screen(table_dump_screen);
            },
            ScreenKind::TableDump => {
                // Edit cells here
                return;
            }
        }
    }

    // Adds a new screen and sets it as active
    fn add_screen(&mut self, s : CSGScreen) {
        self.screens.push(s);
        self.active_screen = self.active_screen + 1;
    }

    /// Get method for coordinates of the active window
    fn get_active_window_coords(&self) -> (i64, i64) {
        let coords = self.screens[self.active_screen].active_window;
        (coords.0 as i64, coords.1 as i64)
    }

    /// Get method for a pointer to the active window
    fn get_active_window(&self) -> Option<&CSGWindow> {
        self.screens[self.active_screen].get_active_window()
    }

    /// Set method for the active window
    fn set_active_window(&mut self, prev : (i64, i64), next : (i64, i64)) {
        self.screens[self.active_screen].set_active_window(prev, next)
    }

    // Read characters within the context of the current window
    fn read_current_window(&self) -> usize {
        let current_window : &CSGWindow = self.get_active_window().unwrap();
        current_window.window.read_in()
    }
}