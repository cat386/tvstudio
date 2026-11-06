//! Пятнашки (`puzzle.cpp`): `TPuzzleView` и `TPuzzleWindow`.
//! Стрелки двигают фишку в пустую клетку, щелчок по соседней фишке — тоже.

use tvision::*;

/// Палитра (`cpPuzzlePalette`).
const PUZZLE_PALETTE: &[u8] = b"\x06\x07";

const BOARD_START: [char; 16] = ['A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', ' '];
/// Шахматная раскраска фишек (`map`).
const MAP: [usize; 15] = [0, 1, 0, 1, 1, 0, 1, 0, 0, 1, 0, 1, 1, 0, 1];
const SOLUTION: &str = "ABCDEFGHIJKLMNO ";

/// Простой генератор псевдослучайных чисел (вместо `rand()`).
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545F4914F6CDD1D)
    }
}

pub struct PuzzleView {
    pub vs: ViewState,
    pub board: [[char; 4]; 4],
    pub moves: u32,
    pub solved: bool,
    rng: Rng,
}

impl PuzzleView {
    pub fn new(r: Rect) -> Self {
        let mut vs = ViewState::new(r);
        vs.options |= of::SELECTABLE;
        let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1) | 1;
        let mut board = [[' '; 4]; 4];
        for i in 0..4 {
            for j in 0..4 {
                board[i][j] = BOARD_START[i * 4 + j];
            }
        }
        let mut p = PuzzleView { vs, board, moves: 0, solved: false, rng: Rng(seed) };
        p.scramble();
        p
    }

    fn blank(&self) -> (usize, usize) {
        for i in 0..16 {
            if self.board[i / 4][i % 4] == ' ' {
                return (i % 4, i / 4);
            }
        }
        (3, 3)
    }

    /// Сдвиг фишки в пустую клетку (`moveKey`).
    pub fn move_key(&mut self, key: Key) {
        let (x, y) = self.blank();
        let src = match key {
            Key::Down if y > 0 => Some((x, y - 1)),
            Key::Up if y < 3 => Some((x, y + 1)),
            Key::Right if x > 0 => Some((x - 1, y)),
            Key::Left if x < 3 => Some((x + 1, y)),
            _ => None,
        };
        if let Some((sx, sy)) = src {
            self.board[y][x] = self.board[sy][sx];
            self.board[sy][sx] = ' ';
            if self.moves < 1000 {
                self.moves += 1;
            }
        }
    }

    /// Щелчок по фишке рядом с пустой клеткой (`moveTile`).
    fn move_tile(&mut self, p: Point) {
        let (bx, by) = self.blank();
        let i = (by * 4 + bx) as i32;
        let x = p.x / 3;
        let y = p.y;
        match y * 4 + x - i {
            -4 => self.move_key(Key::Down),
            -1 => self.move_key(Key::Right),
            1 => self.move_key(Key::Left),
            4 => self.move_key(Key::Up),
            _ => {}
        }
    }

    /// Перемешать (`scramble`).
    pub fn scramble(&mut self) {
        self.moves = 0;
        self.solved = false;
        loop {
            let k = match (self.rng.next() >> 4) % 4 {
                0 => Key::Up,
                1 => Key::Down,
                2 => Key::Right,
                _ => Key::Left,
            };
            self.move_key(k);
            self.moves += 1;
            if self.moves > 500 {
                break;
            }
        }
        self.moves = 0;
    }

    /// Проверка победы (`winCheck`).
    fn win_check(&mut self) {
        let s: String = self.board.iter().flatten().collect();
        if s == SOLUTION {
            self.solved = true;
        }
    }
}

impl View for PuzzleView {
    tvision::impl_view_basics!();

    /// Порт `TPuzzleView::draw`.
    fn draw(&self, dc: &mut DrawCtx) {
        let back = dc.color(1);
        let colors = [back, if self.solved { back } else { dc.color(2) }];
        for i in 0..4 {
            let mut b = DrawBuffer::new(18);
            b.move_char(0, ' ', back, 18);
            if i == 1 {
                b.move_str(13, "Move", back);
            }
            if i == 2 {
                b.move_str(14, &self.moves.to_string(), back);
            }
            for j in 0..4 {
                let c = self.board[i][j];
                let s = format!(" {} ", c);
                let attr = if c == ' ' { colors[0] } else { colors[MAP[(c as u8 - b'A') as usize]] };
                b.move_str(j * 3, &s, attr);
            }
            dc.write_line(0, i as i32, 18, 1, &b);
        }
    }

    fn palette(&self) -> &'static [u8] {
        PUZZLE_PALETTE
    }

    fn handle_event(&mut self, ev: &mut Event, ctx: &mut Ctx) {
        tvision::view::default_handle_event(&self.vs, ev, ctx);
        // После победы любая клавиша или щелчок начинает новую игру.
        if self.solved && (matches!(ev, Event::KeyDown(_)) || ev.is_mouse()) {
            self.scramble();
            ev.clear();
            return;
        }
        match ev {
            Event::MouseDown(m) => {
                let p = self.vs.make_local(m.pos);
                self.move_tile(p);
                ev.clear();
                self.win_check();
            }
            Event::KeyDown(k) => {
                self.move_key(k.key);
                ev.clear();
                self.win_check();
            }
            _ => {}
        }
    }
}

/// Окно пятнашек (`TPuzzleWindow`).
pub fn puzzle_window() -> Window {
    let mut w = Window::new(Rect::new(1, 1, 21, 7), "Puzzle", 0);
    w.flags &= !(wf::ZOOM | wf::GROW);
    w.group.vs.grow_mode = 0;
    let r = w.group.vs.extent().grown(-1, -1);
    w.add(PuzzleView::new(r));
    w
}
