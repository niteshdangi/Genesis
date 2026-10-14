pub const WIDTH: usize = 128;
pub const HEIGHT: usize = 128;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[repr(u8)]
pub enum Cell {
    Empty = 0,
    A = 1,
    B = 2,
    C = 3,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct World {
    cells: Vec<u8>,
}

impl Default for World {
    fn default() -> Self {
        Self {
            cells: vec![0; WIDTH * HEIGHT],
        }
    }
}

impl World {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cells(&self) -> &[u8] {
        &self.cells
    }

    fn index(x: usize, y: usize) -> usize {
        assert!(x < WIDTH && y < HEIGHT, "cell coordinates out of bounds");
        y * WIDTH + x
    }

    pub fn get(&self, x: usize, y: usize) -> Cell {
        match self.cells[Self::index(x, y)] {
            0 => Cell::Empty,
            1 => Cell::A,
            2 => Cell::B,
            3 => Cell::C,
            _ => unreachable!("world contains an invalid cell state"),
        }
    }

    pub fn set(&mut self, x: usize, y: usize, state: Cell) {
        self.cells[Self::index(x, y)] = state as u8;
    }

    pub fn populations(&self) -> [usize; 4] {
        let mut populations = [0; 4];
        for &cell in &self.cells {
            populations[usize::from(cell)] += 1;
        }
        populations
    }

    pub fn neighbors(&self, x: usize, y: usize) -> [Cell; 8] {
        Self::neighbor_positions(x, y).map(|(nx, ny)| self.get(nx, ny))
    }

    pub fn neighbor_positions(x: usize, y: usize) -> [(usize, usize); 8] {
        Self::index(x, y);
        let xs = [(x + WIDTH - 1) % WIDTH, x, (x + 1) % WIDTH];
        let ys = [(y + HEIGHT - 1) % HEIGHT, y, (y + 1) % HEIGHT];
        [
            (xs[0], ys[0]),
            (xs[1], ys[0]),
            (xs[2], ys[0]),
            (xs[0], ys[1]),
            (xs[2], ys[1]),
            (xs[0], ys[2]),
            (xs[1], ys[2]),
            (xs[2], ys[2]),
        ]
    }
}
